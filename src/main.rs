#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::Duration;
use embassy_usb::class::hid::State;
use usbd_hid::descriptor::KeyboardReport;
use {defmt_rtt as _, panic_probe as _};

mod debouncer;
mod usb;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut morse_btn =
        debouncer::Debouncer::new(Input::new(p.PIN_9, Pull::Up), Duration::from_millis(20));

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

    // set up a listening / transmitting loop for the USB interface
    let in_fut = async {
        loop {
            info!("Waiting for HIGH on pin 16");
            morse_btn.debounce().await;
            info!("HIGH DETECTED");
            // Create a report with the A key pressed. (no shift modifier)
            let report = KeyboardReport {
                keycodes: [4, 0, 0, 0, 0, 0],
                leds: 0,
                modifier: 0,
                reserved: 0,
            };
            // Send the report.
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            };
            morse_btn.debounce().await;
            info!("LOW DETECTED");
            let report = KeyboardReport {
                keycodes: [0, 0, 0, 0, 0, 0],
                leds: 0,
                modifier: 0,
                reserved: 0,
            };
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            };
        }
    };

    let mut request_handler = usb::MyRequestHandler {};
    let out_fut = async {
        reader.run(false, &mut request_handler).await;
    };

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, join(in_fut, out_fut)).await;
}
