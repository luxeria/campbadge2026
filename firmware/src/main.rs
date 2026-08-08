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

use games::snake::{CELL, Direction, GRID, ORIGIN_X, ORIGIN_Y, Snake, StepResult, pastel};

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

/// Draws a small score numeral near the bottom of the round panel.
fn draw_score(canvas: &mut Canvas, score: usize) {
    let mut buffer = [0u8; 8];
    let mut value = score;
    let mut length = 0;
    if value == 0 {
        buffer[0] = b'0';
        length = 1;
    }
    while value > 0 && length < buffer.len() {
        buffer[length] = b'0' + (value % 10) as u8;
        value /= 10;
        length += 1;
    }
    buffer[..length].reverse();
    let text = core::str::from_utf8(&buffer[..length]).unwrap_or("0");
    let width = canvas.measure_text(text, 2);
    canvas.draw_text(text, 120 - width / 2, 210, 2, palette::WHITE);
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

    // --- Snake game from the `games` crate ---
    let framebuffer = unsafe { &mut *(&raw mut FRAMEBUFFER) };
    let mut canvas = Canvas::new(framebuffer);
    log_line(&mut tx, "snake starting\n");

    let mut input = Input::new();
    let mut now_ms: u32 = 0;
    let mut tick_accum: u32 = 0;
    const BASE_TICK_MS: u32 = 180;
    const MIN_TICK_MS: u32 = 45;
    let mut snake = Snake::new(0x5eed);
    let mut game_over = false;

    loop {
        now_ms = now_ms.wrapping_add(33);

        // Buttons on expander port 0 read low while pressed; invert the byte.
        let port0 = read_expander_register(&mut i2c, expander, REG_INPUT_0);
        let active_mask = !port0;
        input.update(active_mask, now_ms);

        // Physical layout on this badge: up = Btn2, right = Btn3.
        if input.just_pressed(Button::Btn1) {
            snake.set_direction(Direction::Left);
        } else if input.just_pressed(Button::Btn2) {
            snake.set_direction(Direction::Up);
        } else if input.just_pressed(Button::Btn3) {
            snake.set_direction(Direction::Right);
        } else if input.just_pressed(Button::Btn4) {
            snake.set_direction(Direction::Down);
        }
        if game_over && input.just_pressed(Button::Btn8) {
            snake = Snake::new(now_ms);
            game_over = false;
            tick_accum = 0; // drop the time piled up on the game-over screen
        }

        // The snake quickens as it grows, never below the floor; holding
        // Btn5 multiplies the speed fourfold while pressed.
        let mut tick_ms = BASE_TICK_MS
            .saturating_sub(snake.score() as u32 * 16)
            .max(MIN_TICK_MS);
        if !game_over && input.pressed(Button::Btn5) {
            tick_ms /= 4;
        }
        tick_accum += 33;
        if !game_over && tick_accum >= tick_ms {
            tick_accum -= tick_ms;
            if snake.update() == StepResult::Died {
                game_over = true;
            }
        }

        canvas.clear(palette::BLACK);
        // Leave the surround black and fill the playfield in a pastel brown-grey.
        canvas.rect_filled(ORIGIN_X, ORIGIN_Y, GRID * CELL, GRID * CELL, pastel::BACKGROUND);
        canvas.rect(ORIGIN_X, ORIGIN_Y, GRID * CELL, GRID * CELL, palette::BLACK);
        if game_over {
            canvas.draw_text("GAME OVER", 76, 100, 2, pastel::BODY);
            draw_score(&mut canvas, snake.score());
        } else {
            snake.draw(&mut canvas);
            draw_score(&mut canvas, snake.score());
        }

        flush_screen(&mut spi, &mut dc, &mut cs, canvas.as_slice());
        delay.delay_millis(33);
    }
}
