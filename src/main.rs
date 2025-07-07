#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Ticker, Timer};
use embassy_usb::class::hid::{HidReader, HidReaderWriter, HidWriter, State};
use embassy_usb::msos::windows_version;
use embassy_usb::{Builder, Config, UsbDevice};
use key_mapping::char_to_hid_u8;
use static_cell::StaticCell;
use usb::KodeboardUsbDeviceHandler;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use {defmt_rtt as _, panic_probe as _};

mod debouncer;
mod decoder;
mod key_mapping;
mod usb;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type EventChannelType = (char, bool);
type EventChannel = Channel<ThreadModeRawMutex, EventChannelType, 32>;
type EventSender = Sender<'static, ThreadModeRawMutex, EventChannelType, 32>;
type EventReceiver = Receiver<'static, ThreadModeRawMutex, EventChannelType, 32>;
static EVENT_CHANNEL: EventChannel = Channel::new();

// Descriptors for the USB. Static so we can share the USB handles around tasks
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

// The state for the USB
static STATE: StaticCell<State> = StaticCell::new();

// The USB device handler
static USB_DEV_HANDLER: StaticCell<KodeboardUsbDeviceHandler> = StaticCell::new();

type ButtonType = Mutex<ThreadModeRawMutex, Option<Input<'static>>>;
static MORSE_BUTTON: ButtonType = Mutex::new(None);
static SPACE_BUTTON: ButtonType = Mutex::new(None);
static SHIFT_BUTTON: ButtonType = Mutex::new(None);

macro_rules! setup_button {
    ($pin: expr, $target: expr) => {
        // Set up the button for listening to morse code inputs
        let mut btn = Input::new($pin, Pull::Up);
        btn.set_schmitt(true);
        {
            *($target.lock().await) = Some(btn);
        }
    };
}

macro_rules! read_button {
    ($btn: expr) => {{
        let btn_unlocked = $btn.lock().await;

        if let Some(btn_ref) = btn_unlocked.as_ref() {
            Some(btn_ref.is_high())
        } else {
            None
        }
    }};
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Connected a Morse Kodeboard!");
    info!("Configuring...");

    let p = embassy_rp::init(Default::default());

    // Set up USB
    let driver = Driver::new(p.USB, Irqs);
    let device_handler = USB_DEV_HANDLER.init(usb::KodeboardUsbDeviceHandler::default());

    // TODO: this is a test code from pid.codes, change before release
    let mut config = Config::new(0x16c0, 0x27dd);
    config.manufacturer = Some("Wilsk");
    config.product = Some("Morse Kodeboard");
    config.serial_number = Some("000001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    let mut builder = Builder::new(
        driver,
        config,
        &mut CONFIG_DESC.init([0; 256])[..],
        &mut BOS_DESC.init([0; 256])[..],
        &mut MSOS_DESC.init([0; 256])[..],
        &mut CONTROL_BUF.init([0; 64])[..],
    );
    builder.handler(device_handler);
    builder.msos_descriptor(windows_version::WIN10, 2);

    // Create the HID inteface
    let hid_config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 60,
        max_packet_size: 64,
    };
    let hid = HidReaderWriter::<_, 1, 8>::new(&mut builder, STATE.init(State::new()), hid_config);
    let usb = builder.build();

    // Set up the button for listening to morse code inputs
    setup_button!(p.PIN_14, SPACE_BUTTON);
    setup_button!(p.PIN_15, SHIFT_BUTTON);
    setup_button!(p.PIN_16, MORSE_BUTTON);

    info!("Configuration complete");

    // Now start spinning up the tasks
    info!("Spawning USB handling task");
    unwrap!(spawner.spawn(usb_loop(usb)));

    info!("Spawning usb HID transmission task");
    let (reader, writer) = hid.split();
    unwrap!(spawner.spawn(usb_hid_loop(EVENT_CHANNEL.receiver(), writer)));

    info!("Spawning USB request handler task");
    unwrap!(spawner.spawn(usb_request_handler(reader)));

    info!("Spawning morse code button observer task");
    unwrap!(spawner.spawn(generate_morse_code_characters(
        &MORSE_BUTTON,
        &SHIFT_BUTTON,
        EVENT_CHANNEL.sender()
    )));
}

/// The underlying USB send/receive loop on the [UsbDevice]
#[embassy_executor::task]
async fn usb_loop(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

/// Listens for events from the morse code parser and sends them on as key
/// presses on the HID keyboard interface
#[embassy_executor::task]
async fn usb_hid_loop(
    event_receiver: EventReceiver,
    mut writer: HidWriter<'static, Driver<'static, USB>, 8>,
) {
    info!("Starting event loop");
    // throttle the loop a little bit
    let mut ticker = Ticker::every(Duration::from_millis(20));
    loop {
        match event_receiver.try_receive() {
            Ok((char, shift_held)) => {
                let Some(code) = char_to_hid_u8(char) else {
                    continue;
                };

                info!("Sending {} Key ({}u8)", char, code);
                let report = KeyboardReport {
                    keycodes: [code, 0, 0, 0, 0, 0],
                    leds: 0,
                    modifier: if shift_held { 0x02 } else { 0 },
                    reserved: 0,
                };
                // Send the report.
                match writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };

                // delay 10ms before we release the key
                Timer::after(Duration::from_millis(10)).await;

                info!("Releasing {} Key", char);
                let report = KeyboardReport {
                    keycodes: [0, 0, 0, 0, 0, 0],
                    leds: 0,
                    modifier: 0,
                    reserved: 0,
                };
                // Send the report.
                match writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };
            }
            Err(_err) => {
                // nop - we just move on
            }
        }

        ticker.next().await;
    }
}

/// Handles USB requests received on the [`HidReader`]
#[embassy_executor::task]
async fn usb_request_handler(reader: HidReader<'static, Driver<'static, USB>, 1>) {
    let mut request_handler = usb::KodeboardUsbRequestHandler::default();
    reader.run(false, &mut request_handler).await;
}

/// Listens to the supplied button and passes button actions (press/release) to
/// a morse code decoder. As characters are received by the encoder it sends them
/// through the [`EventSender`] channel for transmission via USB HID.
#[embassy_executor::task]
async fn generate_morse_code_characters(
    morse_btn: &'static ButtonType,
    shift_button: &'static ButtonType,
    sender: EventSender,
) {
    info!("Configuring morse decoder");
    let mut morse_decoder = decoder::Decoder::new(60);
    let mut ticker = Ticker::every(Duration::from_millis(1));

    let mut btn_debouncer = if let Some(btn_ref) = morse_btn.lock().await.as_ref() {
        debouncer::DebouncedInput::new(btn_ref.is_high())
    } else {
        crate::panic!("Unable to access button")
    };

    info!("Starting morse listen loop");
    loop {
        // debounce the input
        let result = {
            if let Some(btn) = read_button!(morse_btn) {
                btn_debouncer.debounce(btn)
            } else {
                btn_debouncer.current()
            }
        };

        // update the morse decoder
        let change_time = Instant::now();
        if let Some(char) = morse_decoder.push(result, change_time) {
            let shift_held = if let Some(shift_held) = read_button!(shift_button) {
                shift_held
            } else {
                false
            };
            sender.send((char, shift_held)).await;
        }

        // only check inputs periodically
        ticker.next().await;
    }
}
