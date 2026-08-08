//! Firmware entry point for the LuxCamp badge (M5Stack Atom S3 Lite).
//!
//! Brings up the I2C expander (display reset + buttons + LEDs), initialises the
//! round GC9A01A panel over SPI, and runs an interactive demo driven by the
//! `raylib_camp` engine. Debug logs are emitted over native-USB serial in a
//! strictly non-blocking way, so the game runs the same whether or not a serial
//! monitor is attached.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::main;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_hal::usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagTx};

use raylib_camp::canvas::{Canvas, display};
use raylib_camp::color::palette;
use raylib_camp::input::{Button, Input};

// Embeds an ESP-IDF application descriptor so `espflash` can flash the binary.
esp_bootloader_esp_idf::esp_app_desc!();

/// TCA9535/TCA9539 registers.
const REG_INPUT_0: u8 = 0x00;
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

/// A tiny `core::fmt::Write` target writing into a fixed stack buffer.
struct ByteWriter<'a> {
    buffer: &'a mut [u8],
    length: usize,
}

impl core::fmt::Write for ByteWriter<'_> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let bytes = text.as_bytes();
        let remaining = &mut self.buffer[self.length..];
        let count = bytes.len().min(remaining.len());
        remaining[..count].copy_from_slice(&bytes[..count]);
        self.length += count;
        Ok(())
    }
}

/// Emits a log line without ever blocking the caller.
///
/// The native-USB serial FIFO only drains when a host reads it, so a blocking
/// write stalls the game whenever no monitor is attached. Bytes are pushed one
/// at a time and the write stops as soon as the FIFO is full, dropping whatever
/// does not fit; a non-blocking flush is attempted so partial lines still leave
/// the peripheral when a host is present.
fn log_line(tx: &mut UsbSerialJtagTx<'_, esp_hal::Blocking>, line: &str) {
    for byte in line.bytes() {
        if tx.write_byte_nb(byte).is_err() {
            break;
        }
    }
    let _ = tx.flush_tx_nb();
}

/// Writes a formatted message to the serial output non-blockingly.
///
/// Formats into a fixed stack buffer so the `no_std` firmware needs no heap.
fn log_fmt(tx: &mut UsbSerialJtagTx<'_, esp_hal::Blocking>, message: core::fmt::Arguments<'_>) {
    let (storage, length) = {
        let mut storage = [0u8; 64];
        let length = {
            let mut writer = ByteWriter {
                buffer: &mut storage,
                length: 0,
            };
            let _ = core::fmt::write(&mut writer, message);
            writer.length
        };
        (storage, length)
    };
    log_line(tx, core::str::from_utf8(&storage[..length]).unwrap_or(""));
}

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

/// GC9A01A register tuning table, transcribed from Adafruit's initialisation.
/// Each pair is a command byte followed by its parameter bytes.
const INIT_SEQUENCE: &[(&[u8], &[u8])] = &[
    (&[0xef], &[]),
    (&[0xeb], &[0x14]),
    (&[0xfe], &[]),
    (&[0xef], &[]),
    (&[0xeb], &[0x14]),
    (&[0x84], &[0x40]),
    (&[0x85], &[0xff]),
    (&[0x86], &[0xff]),
    (&[0x87], &[0xff]),
    (&[0x88], &[0x0a]),
    (&[0x89], &[0x21]),
    (&[0x8a], &[0x00]),
    (&[0x8b], &[0x80]),
    (&[0x8c], &[0x01]),
    (&[0x8d], &[0x01]),
    (&[0x8e], &[0xff]),
    (&[0x8f], &[0xff]),
    (&[0xb6], &[0x00, 0x00]),
    (&[0x36], &[0x48]), // MADCTL: MX | BGR
    (&[0x3a], &[0x05]), // RGB565 pixel format
    (&[0x90], &[0x08, 0x08, 0x08, 0x08]),
    (&[0xbd], &[0x06]),
    (&[0xbc], &[0x00]),
    (&[0xff], &[0x60, 0x01, 0x04]),
    (&[0xc3], &[0x13]),
    (&[0xc4], &[0x13]),
    (&[0xc9], &[0x22]),
    (&[0xbe], &[0x11]),
    (&[0xe1], &[0x10, 0x0e]),
    (&[0xdf], &[0x21, 0x0c, 0x02]),
    (&[0xf0], &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2a]),
    (&[0xf1], &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6f]),
    (&[0xf2], &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2a]),
    (&[0xf3], &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6f]),
    (&[0xed], &[0x1b, 0x0b]),
    (&[0xae], &[0x77]),
    (&[0xcd], &[0x63]),
    (&[0xe8], &[0x34]),
    (&[0x62], &[0x18, 0x0d, 0x71, 0xed, 0x70, 0x70, 0x18, 0x0f, 0x71, 0xef, 0x70, 0x70]),
    (&[0x63], &[0x18, 0x11, 0x71, 0xf1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xf3, 0x70, 0x70]),
    (&[0x64], &[0x28, 0x29, 0xf1, 0x01, 0xf1, 0x00, 0x07]),
    (&[0x66], &[0x3c, 0x00, 0xcd, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00]),
    (&[0x67], &[0x00, 0x3c, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98]),
    (&[0x74], &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4e, 0x00]),
    (&[0x98], &[0x3e, 0x07]),
    (&[0x35], &[]), // tearing effect line on
    (&[0x21], &[]), // display inversion on
];

/// Runs the full GC9A01A initialisation sequence.
fn init_display(
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    delay: &mut Delay,
) {
    for &(command, data) in INIT_SEQUENCE {
        send_command(spi, dc, cs, command[0]);
        if !data.is_empty() {
            send_data(spi, dc, cs, data);
        }
    }

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
fn probe_expander(
    tx: &mut UsbSerialJtagTx<'_, esp_hal::Blocking>,
    i2c: &mut I2c<'_, esp_hal::Blocking>,
) -> u8 {
    let mut dummy = [0u8; 1];
    for address in 0x74..=0x77 {
        if i2c.read(address, &mut dummy).is_ok() {
            log_fmt(tx, format_args!("expander found at 0x{address:02x}\n"));
            return address;
        }
    }
    0x00
}

/// Reads one byte from an I2C register on the expander.
fn read_expander_register(
    i2c: &mut I2c<'_, esp_hal::Blocking>,
    address: u8,
    register: u8,
) -> u8 {
    let mut value = [0u8; 1];
    let _ = i2c.write_read(address, &[register], &mut value);
    value[0]
}

/// Draws a static demonstration scene using the engine's drawing primitives.
fn draw_demo_scene(canvas: &mut Canvas) {
    canvas.rect(30, 40, 70, 50, palette::RED);
    canvas.rect_filled(120, 40, 70, 50, palette::GREEN);
    canvas.circle(70, 150, 20, palette::YELLOW);
    canvas.circle_filled(150, 150, 20, palette::CYAN);
    canvas.triangle_filled((100, 200), (160, 200), (130, 165), palette::MAGENTA);
    canvas.line(20, 20, 220, 30, palette::ORANGE);
    canvas.draw_text("CAMP 2026", 70, 4, 2, palette::WHITE);
}

/// Application entry point.
#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let usb = UsbSerialJtag::new(peripherals.USB_DEVICE);
    let (_rx, mut tx) = usb.split();
    let mut delay = Delay::new();
    log_line(&mut tx, "badge: starting\n");

    // --- I2C expander (buttons, LEDs, display reset) ---
    let mut i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("configure I2C")
    .with_sda(peripherals.GPIO38)
    .with_scl(peripherals.GPIO39);

    let expander = probe_expander(&mut tx, &mut i2c);
    if expander == 0 {
        log_line(&mut tx, "ERROR: expander not found on I2C\n");
        loop {}
    }

    // Port 0 = button inputs, port 1 bits 0..4 = outputs (reset + LEDs).
    i2c.write(expander, &[REG_CONFIG_0, PORT_0_INPUTS])
        .expect("configure expander port 0");
    i2c.write(expander, &[REG_CONFIG_1, PORT_1_OUTPUT_MASK])
        .expect("configure expander port 1");
    log_line(&mut tx, "expander configured\n");

    // Bring the display out of reset via P1.0, matching the reference
    // driver's timing (idle high, pulse low, release high, then settle).
    i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
        .expect("set display reset idle");
    delay.delay_millis(10);
    i2c.write(expander, &[REG_OUTPUT_1, 0x00])
        .expect("assert display reset");
    delay.delay_millis(10);
    i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
        .expect("release display reset");
    delay.delay_millis(120);
    log_line(&mut tx, "display reset released\n");

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
    log_line(&mut tx, "display initialised\n");

    // --- Render a demonstration scene from the engine framebuffer ---
    let framebuffer = unsafe { &mut *(&raw mut FRAMEBUFFER) };
    let mut canvas = Canvas::new(framebuffer);
    log_line(&mut tx, "rendering demo\n");

    // Bouncing disc, labelled with its position and velocity.
    const CENTRE: (f32, f32) = (120.0, 120.0);
    const BOUNDARY: f32 = 92.0;
    let mut ball_radius: i32 = 8;
    let mut position: (f32, f32) = (120.0, 60.0);
    let mut velocity: (f32, f32) = (2.5, 2.0);
    let mut ball_color = palette::WHITE;
    let mut input = Input::new();
    let mut now_ms: u32 = 0;

    loop {
        now_ms = now_ms.wrapping_add(33);
        canvas.clear(palette::BLACK);
        draw_demo_scene(&mut canvas);

        // Buttons on expander port 0 read low while pressed, so the set of
        // pressed bits is the inverse of the raw port byte.
        let port0 = read_expander_register(&mut i2c, expander, REG_INPUT_0);
        let active_mask = !port0;
        input.update(active_mask, now_ms);

        // Tap a colour button to select a persistent ball colour.
        if input.just_pressed(Button::Btn1) {
            ball_color = palette::RED;
            log_line(&mut tx, "colour red\n");
        } else if input.just_pressed(Button::Btn2) {
            ball_color = palette::GREEN;
            log_line(&mut tx, "colour green\n");
        } else if input.just_pressed(Button::Btn3) {
            ball_color = palette::BLUE;
            log_line(&mut tx, "colour blue\n");
        } else if input.just_pressed(Button::Btn4) {
            ball_color = palette::YELLOW;
            log_line(&mut tx, "colour yellow\n");
        }

        // Holding the control buttons applies their effect continuously.
        if input.pressed(Button::Btn5) {
            velocity.0 *= 1.15;
            velocity.1 *= 1.15;
        } else if input.pressed(Button::Btn6) {
            velocity.0 *= 0.85;
            velocity.1 *= 0.85;
        }
        if input.pressed(Button::Btn7) {
            ball_radius = ball_radius.saturating_add(3).min(40);
        } else if input.pressed(Button::Btn8) {
            ball_radius = ball_radius.saturating_sub(3).max(2);
        }

        // Keep the ball from crawling or escaping; scale speed back in bounds.
        let speed = raylib_camp::math::sqrt(velocity.0 * velocity.0 + velocity.1 * velocity.1);
        if speed > 20.0 {
            let scale = 20.0 / speed;
            velocity.0 *= scale;
            velocity.1 *= scale;
        } else if speed < 0.3 {
            let scale = 0.3 / speed;
            velocity.0 *= scale;
            velocity.1 *= scale;
        }

        let relative_x = position.0 - CENTRE.0;
        let relative_y = position.1 - CENTRE.1;
        let distance = raylib_camp::math::sqrt(relative_x * relative_x + relative_y * relative_y);
        if distance > 0.0 && distance + ball_radius as f32 > BOUNDARY {
            // Reflect velocity about the boundary normal and push the disc back in.
            let normal_x = relative_x / distance;
            let normal_y = relative_y / distance;
            let dot = velocity.0 * normal_x + velocity.1 * normal_y;
            velocity.0 -= 2.0 * dot * normal_x;
            velocity.1 -= 2.0 * dot * normal_y;
            let overhang = distance + ball_radius as f32 - BOUNDARY;
            position.0 -= normal_x * overhang;
            position.1 -= normal_y * overhang;
        }
        position.0 += velocity.0;
        position.1 += velocity.1;
        canvas.circle_filled(
            position.0 as i32,
            position.1 as i32,
            ball_radius,
            ball_color,
        );

        flush_screen(&mut spi, &mut dc, &mut cs, canvas.as_slice());
        delay.delay_millis(33);
    }
}
