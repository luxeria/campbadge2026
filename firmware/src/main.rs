//! Firmware entry point for the LuxCamp badge (M5Stack Atom S3 Lite).
//!
//! Display bring-up: initialises the I2C expander (which drives the display
//! reset), configures SPI to the GC9A01A round panel, initialises the panel,
//! and pushes a solid-colour slide from the `raylib_camp` framebuffer. Each
//! step is logged over native-USB serial so failures are easy to localise.

#![no_std]
#![no_main]

use core::fmt::Write;

use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::main;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_hal::usb_serial_jtag::UsbSerialJtag;

use raylib_camp::canvas::{Canvas, display};
use raylib_camp::color::palette;

// Embeds an ESP-IDF application descriptor so `espflash` can flash the binary.
esp_bootloader_esp_idf::esp_app_desc!();

/// TCA9535/TCA9539 registers.
const REG_CONFIG_0: u8 = 0x06;
const REG_CONFIG_1: u8 = 0x07;
const REG_OUTPUT_1: u8 = 0x03;

/// TCA9539 configuration: port 0 (buttons) all inputs, port 1 low nibble
/// (display reset + LEDs) outputs.
const PORT_0_INPUTS: u8 = 0xff;
const PORT_1_OUTPUT_MASK: u8 = 0xe0;

/// GC9A01A display reset lives on expander pin P1.0.
const DISPLAY_RESET_BIT: u8 = 0x01;

/// The round panel size in pixels.
const PANEL_SIZE: usize = 240;

/// Framebuffer backing storage (RGB565), big enough for the whole panel.
static mut FRAMEBUFFER: [u16; display::PIXEL_COUNT] = [0; display::PIXEL_COUNT];

/// Sends a single command byte to the panel.
fn send_command(spi: &mut Spi<'_, esp_hal::Blocking>, dc: &mut Output, cs: &mut Output, command: u8) {
    cs.set_low();
    dc.set_low();
    let _ = spi.write(&[command]);
    cs.set_high();
}

/// Sends a block of data bytes to the panel.
fn send_data(
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    data: &[u8],
) {
    cs.set_low();
    dc.set_high();
    let _ = spi.write(data);
    cs.set_high();
}

/// Runs the minimal GC9A01A initialisation sequence.
fn init_display(
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    delay: &mut Delay,
) {
    send_command(spi, dc, cs, 0x36);
    send_data(spi, dc, cs, &[0x00]); // MADCTL, memory orientation
    send_command(spi, dc, cs, 0x3a);
    send_data(spi, dc, cs, &[0x05]); // RGB565 pixel format
    send_command(spi, dc, cs, 0x11); // sleep out
    delay.delay_millis(120);
    send_command(spi, dc, cs, 0x29); // display on

    // Address the full 240x240 window from (0,0) to (239,239).
    send_command(spi, dc, cs, 0x2a);
    send_data(spi, dc, cs, &[0x00, 0x00, 0x00, 0xef]);
    send_command(spi, dc, cs, 0x2b);
    send_data(spi, dc, cs, &[0x00, 0x00, 0x00, 0xef]);
}

/// Pushes the whole framebuffer to the display, byte-swapping each RGB565
/// value because the panel expects big-endian pixel bytes.
fn flush_screen(
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    framebuffer: &[u16],
) {
    send_command(spi, dc, cs, 0x2c); // memory write
    cs.set_low();
    dc.set_high();
    let mut row_bytes = [0u8; PANEL_SIZE * 2];
    for row in 0..PANEL_SIZE {
        for column in 0..PANEL_SIZE {
            let pixel = framebuffer[row * PANEL_SIZE + column];
            row_bytes[column * 2] = (pixel >> 8) as u8;
            row_bytes[column * 2 + 1] = (pixel & 0xff) as u8;
        }
        let _ = spi.write(&row_bytes);
    }
    cs.set_high();
}

/// Probes the I2C bus for the badge's I/O expander, which is addressed in the
/// TCA9539 range 0x74..=0x77 depending on its address pins.
fn probe_expander(serial: &mut UsbSerialJtag<'_, esp_hal::Blocking>, i2c: &mut I2c<'_, esp_hal::Blocking>) -> u8 {
    let mut dummy = [0u8; 1];
    for address in 0x74..=0x77 {
        if i2c.read(address, &mut dummy).is_ok() {
            writeln!(serial, "expander found at 0x{address:02x}").ok();
            return address;
        }
    }
    0x00
}

/// Application entry point.
#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut serial = UsbSerialJtag::new(peripherals.USB_DEVICE);
    let mut delay = Delay::new();
    writeln!(serial, "badge: bring-up starting").ok();

    // --- I2C expander (buttons, LEDs, display reset) ---
    let mut i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("configure I2C")
    .with_sda(peripherals.GPIO38)
    .with_scl(peripherals.GPIO39);

    let expander = probe_expander(&mut serial, &mut i2c);
    if expander == 0 {
        writeln!(serial, "ERROR: expander not found on I2C").ok();
        loop {}
    }

    // Port 0 = button inputs, port 1 bits 0..4 = outputs (reset + LEDs).
    i2c.write(expander, &[REG_CONFIG_0, PORT_0_INPUTS])
        .expect("configure expander port 0");
    i2c.write(expander, &[REG_CONFIG_1, PORT_1_OUTPUT_MASK])
        .expect("configure expander port 1");
    writeln!(serial, "expander configured").ok();

    // Bring the display out of reset via P1.0.
    i2c.write(expander, &[REG_OUTPUT_1, 0x00])
        .expect("assert display reset");
    delay.delay_millis(50);
    i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
        .expect("release display reset");
    delay.delay_millis(20);
    writeln!(serial, "display reset released").ok();

    // --- SPI to the panel ---
    let mut spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(20))
            .with_mode(Mode::_0),
    )
    .expect("configure SPI")
    .with_sck(peripherals.GPIO5)
    .with_mosi(peripherals.GPIO6);

    let mut dc = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let mut cs = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());

    init_display(&mut spi, &mut dc, &mut cs, &mut delay);
    writeln!(serial, "display initialised").ok();

    // --- Draw a solid colour slide from the engine framebuffer ---
    // Borrow the static once via a raw pointer so the canvas lives for the
    // whole loop without repeatedly forming a mutable reference to the static.
    let framebuffer = unsafe { &mut *(&raw mut FRAMEBUFFER) };
    let mut canvas = Canvas::new(framebuffer);
    let slide = [palette::RED, palette::GREEN, palette::BLUE];
    let mut frame: usize = 0;
    loop {
        canvas.clear(slide[frame % slide.len()]);
        flush_screen(&mut spi, &mut dc, &mut cs, canvas.as_slice());
        writeln!(serial, "showing colour {}", frame % slide.len()).ok();
        frame += 1;
        delay.delay_millis(1000);
    }
}
