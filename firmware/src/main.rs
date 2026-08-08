//! Firmware entry point for the LuxCamp badge (M5Stack Atom S3 Lite).
//!
//! Minimal build that validates the esp-hal toolchain, flashing and visibility
//! over the native USB serial port. It initialises the chip, emits a heartbeat
//! over USB-SERIAL-JTAG, and loops forever. The display and input drivers are
//! layered in from here.

#![no_std]
#![no_main]

use core::fmt::Write;

use esp_backtrace as _;
use esp_hal::main;
use esp_hal::time::Instant;
use esp_hal::usb_serial_jtag::UsbSerialJtag;

// Embeds an ESP-IDF application descriptor so `espflash` can flash the binary.
esp_bootloader_esp_idf::esp_app_desc!();

/// Application entry point, called after the `esp-hal` runtime brings up the
/// CPU. Initialises the chip, the native-USB serial port and runs forever.
#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut serial = UsbSerialJtag::new(peripherals.USB_DEVICE);
    let mut counter: u32 = 0;

    loop {
        writeln!(serial, "badge alive {counter}").expect("write heartbeat");
        counter += 1;

        let now = Instant::now();
        while now.elapsed().as_millis() < 1000 {}
    }
}
