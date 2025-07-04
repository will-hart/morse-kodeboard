#![no_std]
#![no_main]

use debouncer::DebounceResult;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Ticker, Timer};
use embassy_usb::class::hid::State;
use usbd_hid::descriptor::KeyboardReport;
use {defmt_rtt as _, panic_probe as _};

mod debouncer;
mod usb;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type EventChannel = Channel<ThreadModeRawMutex, u8, 32>;
type EventSender = Sender<'static, ThreadModeRawMutex, u8, 32>;
static EVENT_CHANNEL: EventChannel = Channel::new();

type ButtonType = Mutex<ThreadModeRawMutex, Option<Input<'static>>>;
static MORSE_BUTTON: ButtonType = Mutex::new(None);

#[embassy_executor::task]
async fn generate_morse_code_characters(morse_btn: &'static ButtonType, sender: EventSender) {
    let mut morse_decoder = morse_codec::decoder::Decoder::<16>::new()
        .with_reference_short_ms(100)
        .build();
    let mut btn_debouncer = debouncer::DebouncedInput::<12>::new();
    let mut ticker = Ticker::every(Duration::from_millis(2));
    let mut last_change: Instant = Instant::now();

    loop {
        // debounce the input
        let result = {
            let btn_unlocked = morse_btn.lock().await;

            if let Some(btn_ref) = btn_unlocked.as_ref() {
                btn_debouncer.debounce(btn_ref.is_high())
            } else {
                DebounceResult::default()
            }
        };

        // update the morse decoder
        if result.is_changed {
            // register the event with the morse decoder
            let this_change = Instant::now();
            let delta = last_change - this_change;
            last_change = this_change;

            morse_decoder.signal_event(delta.as_millis() as u16, result.is_on);

            // check if we have a message to send on the keyboard
            let len = morse_decoder.message.len();
            if len > 0 {
                let msg = morse_decoder.message.as_charray();
                for &ch in msg.iter().take(len) {
                    sender.send(ch).await;
                }

                morse_decoder.message.clear();
            }
        }

        // only check inputs periodically
        ticker.next().await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let morse_btn = Input::new(p.PIN_9, Pull::Up);
    {
        *(MORSE_BUTTON.lock().await) = Some(morse_btn);
    }

    // Create the driver, from the HAL and required buffers and handlers
    let driver = Driver::new(p.USB, Irqs);
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut device_handler = usb::MyDeviceHandler::new();
    let mut state = State::new();
    let (mut usb, hid) = usb::build_usb(
        driver,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
        &mut device_handler,
        &mut state,
    );
    let usb_fut = usb.run();
    let (reader, mut writer) = hid.split();

    let event_receiver = EVENT_CHANNEL.receiver();

    // set up a listening / transmitting loop for the USB interface
    let in_fut = async {
        loop {
            match event_receiver.try_receive() {
                Ok(char) => {
                    info!("Sending Key");
                    // Create a report with the A key pressed. (no shift modifier)
                    let report = KeyboardReport {
                        keycodes: [char, 0, 0, 0, 0, 0],
                        leds: 0,
                        modifier: 0,
                        reserved: 0,
                    };
                    // Send the report.
                    match writer.write_serialize(&report).await {
                        Ok(()) => {}
                        Err(e) => warn!("Failed to send report: {:?}", e),
                    };

                    // delay 10ms before we release the key
                    Timer::after(Duration::from_millis(10)).await;

                    info!("Releasing Key");
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

            info!("LOW DETECTED");
        }
    };

    unwrap!(spawner.spawn(generate_morse_code_characters(
        &MORSE_BUTTON,
        EVENT_CHANNEL.sender()
    )));

    let mut request_handler = usb::MyRequestHandler {};
    let out_fut = async {
        reader.run(false, &mut request_handler).await;
    };

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, join(in_fut, out_fut)).await;
}
