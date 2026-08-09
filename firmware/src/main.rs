//! Firmware entry point for the LuxCamp badge (M5Stack Atom S3 Lite).
//!
//! Runs the two-player SLSO8-themed dice game from the `games` crate in a
//! rotating hot-seat mode: the badge is passed between players each turn. Dice
//! tumble down and settle, the active die is highlighted, and the player picks
//! scoring dice, scores them, rolls the leftovers, or banks the turn.

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

use raylib_camp::canvas::{display, Canvas};
use raylib_camp::color::Color;
use raylib_camp::input::{Button, Input};
use raylib_camp::rand::Prng;

use games::dice::{slso8, Game, DICE_COUNT};

// Embeds an ESP-IDF application descriptor so `espflash` can flash the binary.
esp_bootloader_esp_idf::esp_app_desc!();

/// TCA9535/TCA9539 registers.
const REG_INPUT_0: u8 = 0x00;
const REG_CONFIG_0: u8 = 0x06;
const REG_CONFIG_1: u8 = 0x07;
const REG_OUTPUT_1: u8 = 0x03;

/// TCA9539 configuration: port 0 (buttons) all inputs; port 1 keeps only the
/// display reset (P1.0) as an output so the four LED pins stay inputs (off)
/// from the very first write - they never light, so there is no boot flash.
const PORT_0_INPUTS: u8 = 0xff;
const LED_PINS_INPUT: u8 = 0xfe;

/// GC9A01A display reset lives on expander pin P1.0.
const DISPLAY_RESET_BIT: u8 = 0x01;

/// Framebuffer backing storage (RGB565), big enough for the whole panel.
static mut FRAMEBUFFER: [u16; display::PIXEL_COUNT] = [0; display::PIXEL_COUNT];

/// Side length of a settled die in pixels.
const DIE: i32 = 38;
/// Horizontal spacing between dice slots.
const SLOT_SPACING: i32 = 46;
/// Frames a farkle turn-over screen waits before handing itself over (~2 s at
/// ~30 fps), so the player never has to press B8 to "bank 0" and continue.
const FARKLE_HOLD_FRAMES: u32 = 60;
/// How many pixels the selected die lifts above the row (animated smoothly).
const DIE_LIFT: i32 = 14;

/// Emits a log line without ever blocking the caller.
fn log_line(tx: &mut UsbSerialJtagTx<'_, esp_hal::Blocking>, line: &str) {
    for byte in line.bytes() {
        if tx.write_byte_nb(byte).is_err() {
            break;
        }
    }
    let _ = tx.flush_tx_nb();
}

/// Maps a zero-based button index to its [`Button`] variant.
fn button_at(index: usize) -> Button {
    match index {
        1 => Button::Btn2,
        2 => Button::Btn3,
        3 => Button::Btn4,
        4 => Button::Btn5,
        5 => Button::Btn6,
        6 => Button::Btn7,
        7 => Button::Btn8,
        _ => Button::Btn1,
    }
}

/// Logs a freshly pressed button by name and hex mask, e.g. `btn1 0x01`.
fn log_button_press(tx: &mut UsbSerialJtagTx<'_, esp_hal::Blocking>, index: usize) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buffer = *b"btn0 0x00\n";
    buffer[3] = b'0' + (index as u8 + 1);
    let bit = 1u8 << index;
    buffer[7] = HEX[(bit >> 4) as usize];
    buffer[8] = HEX[(bit & 0x0f) as usize];
    if let Ok(text) = core::str::from_utf8(&buffer[..10]) {
        log_line(tx, text);
    }
}

/// Sends a single command byte to the panel.
fn send_command(
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    command: u8,
) {
    cs.set_low();
    dc.set_low();
    let _ = spi.write(&[command]);
    cs.set_high();
}

/// Sends a block of data bytes to the panel.
fn send_data(spi: &mut Spi<'_, esp_hal::Blocking>, dc: &mut Output, cs: &mut Output, data: &[u8]) {
    cs.set_low();
    dc.set_high();
    let _ = spi.write(data);
    cs.set_high();
}

/// GC9A01A register tuning table, transcribed from Adafruit's initialisation.
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
    (
        &[0x62],
        &[
            0x18, 0x0d, 0x71, 0xed, 0x70, 0x70, 0x18, 0x0f, 0x71, 0xef, 0x70, 0x70,
        ],
    ),
    (
        &[0x63],
        &[
            0x18, 0x11, 0x71, 0xf1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xf3, 0x70, 0x70,
        ],
    ),
    (&[0x64], &[0x28, 0x29, 0xf1, 0x01, 0xf1, 0x00, 0x07]),
    (
        &[0x66],
        &[0x3c, 0x00, 0xcd, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00],
    ),
    (
        &[0x67],
        &[0x00, 0x3c, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98],
    ),
    (&[0x74], &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4e, 0x00]),
    (&[0x98], &[0x3e, 0x07]),
    (&[0x35], &[]),
    (&[0x21], &[]),
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
    send_command(spi, dc, cs, 0x2a);
    send_data(spi, dc, cs, &[0x00, 0x00, 0x00, 0xef]);
    send_command(spi, dc, cs, 0x2b);
    send_data(spi, dc, cs, &[0x00, 0x00, 0x00, 0xef]);
}

/// Pushes the whole framebuffer to the display, byte-swapping to big-endian.
fn flush_screen(
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    framebuffer: &[u16],
) {
    send_command(spi, dc, cs, 0x2c); // memory write
    cs.set_low();
    dc.set_high();
    let mut row_bytes = [0u8; display::WIDTH * 2];
    for row in 0..display::HEIGHT {
        for column in 0..display::WIDTH {
            let pixel = framebuffer[row * display::WIDTH + column];
            row_bytes[column * 2] = (pixel >> 8) as u8;
            row_bytes[column * 2 + 1] = (pixel & 0xff) as u8;
        }
        let _ = spi.write(&row_bytes);
    }
    cs.set_high();
}

/// Probes the I2C bus for the badge's I/O expander in the TCA9539 range.
fn probe_expander(i2c: &mut I2c<'_, esp_hal::Blocking>) -> u8 {
    let mut dummy = [0u8; 1];
    for address in 0x74..=0x77 {
        if i2c.read(address, &mut dummy).is_ok() {
            return address;
        }
    }
    0
}

/// Reads one byte from an I2C register on the expander.
fn read_expander_register(i2c: &mut I2c<'_, esp_hal::Blocking>, address: u8, register: u8) -> u8 {
    let mut value = [0u8; 1];
    let _ = i2c.write_read(address, &[register], &mut value);
    value[0]
}

/// Draws a decimal number centred around the given horizontal point, at a scale.
fn draw_number(canvas: &mut Canvas, x: i32, y: i32, value: u32, color: Color, scale: i32) {
    let mut buffer = [0u8; 8];
    let mut n = 0;
    if value == 0 {
        buffer[n] = b'0';
        n += 1;
    }
    let mut v = value;
    while v > 0 && n < buffer.len() {
        buffer[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    buffer[..n].reverse();
    if let Ok(text) = core::str::from_utf8(&buffer[..n]) {
        let text_width = canvas.measure_text(text, scale);
        canvas.draw_text(text, x - text_width / 2, y, scale, color);
    }
}

/// Centred one-line helper label.
fn draw_centered(canvas: &mut Canvas, y: i32, text: &str, color: Color) {
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(text, 120 - text_width / 2, y, 1, color);
}

/// Draws a short `P<n>` player label centred on the given point.
/// Draws a centred `A  <score>` style ledger line for one player.
fn draw_ledger(
    canvas: &mut Canvas,
    cx: i32,
    y: i32,
    player: usize,
    score: u32,
    scale: i32,
    color: Color,
) {
    let mut buffer = [0u8; 12];
    buffer[0] = if player % 2 == 0 { b'A' } else { b'B' };
    buffer[1] = b' ';
    buffer[2] = b' ';
    let mut digits = [0u8; 9];
    let mut n = 0;
    if score == 0 {
        digits[n] = b'0';
        n += 1;
    }
    let mut v = score;
    while v > 0 && n < digits.len() {
        digits[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    digits[..n].reverse();
    buffer[3..3 + n].copy_from_slice(&digits[..n]);
    if let Ok(text) = core::str::from_utf8(&buffer[..3 + n]) {
        let text_width = canvas.measure_text(text, scale);
        canvas.draw_text(text, cx - text_width / 2, y, scale, color);
    }
}

/// Horizontal centre of the `i`-th die in a row of `count`.
fn die_slot(count: usize, index: usize) -> i32 {
    let span = (count as i32 - 1) * SLOT_SPACING;
    120 - span / 2 + index as i32 * SLOT_SPACING
}

/// Draws a single die face with its pip layout.
fn draw_die(canvas: &mut Canvas, cx: i32, cy: i32, size: i32, value: u8, outline: Color) {
    let half = size / 2;
    let x = cx - half;
    let y = cy - half;
    let corner = (size / 6).clamp(2, 14);

    // Slightly rounded body with a thin outline in the given colour.
    rounded_rect_filled(
        canvas,
        x - 1,
        y - 1,
        size + 2,
        size + 2,
        corner + 1,
        outline,
    );
    rounded_rect_filled(canvas, x, y, size, size, corner, slso8::CREAM);

    // Pips sit a third of the way across each axis; the wide step keeps the
    // dots on a 3-column face (2/4/6) well separated even at the largest size.
    let step = size / 3;
    // Radius stays comfortably below `step` so dots never touch, while staying
    // as round and large as possible.
    let pip = (size / 9).max(3);
    let dots: &[(i32, i32)] = match value {
        1 => &[(0, 0)],
        2 => &[(-step, -step), (step, step)],
        3 => &[(-step, -step), (0, 0), (step, step)],
        4 => &[(-step, -step), (step, -step), (-step, step), (step, step)],
        5 => &[
            (-step, -step),
            (step, -step),
            (0, 0),
            (-step, step),
            (step, step),
        ],
        _ => &[
            (-step, -step),
            (step, -step),
            (-step, 0),
            (step, 0),
            (-step, step),
            (step, step),
        ],
    };
    for &(dx, dy) in dots {
        canvas.circle_filled(cx + dx, cy + dy, pip, slso8::NAVY);
    }
}

/// Returns the index of the leftmost rolled 1 or 5, if any, so the selector
/// can start on a scorable die after a throw.
fn first_scorable(dice: &[u8]) -> Option<usize> {
    dice.iter().position(|&value| value == 1 || value == 5)
}

/// Fills a rectangle with rounded corners of the given radius.
fn rounded_rect_filled(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
    color: Color,
) {
    let radius = radius.clamp(0, width / 2).min(height / 2);
    canvas.rect_filled(x + radius, y, width - 2 * radius, height, color);
    canvas.rect_filled(x, y + radius, radius, height - 2 * radius, color);
    canvas.rect_filled(
        x + width - radius,
        y + radius,
        radius,
        height - 2 * radius,
        color,
    );
    canvas.circle_filled(x + radius, y + radius, radius, color);
    canvas.circle_filled(x + width - radius, y + radius, radius, color);
    canvas.circle_filled(x + radius, y + height - radius, radius, color);
    canvas.circle_filled(x + width - radius, y + height - radius, radius, color);
}

/// Draws an empty die slot (used before a roll).
/// Draws an empty, rounded die slot with a question-mark prompt (shown before
/// a roll, and after the turn has ended but before matching dice land again).
fn draw_slot(canvas: &mut Canvas, cx: i32, cy: i32, size: i32) {
    let half = size / 2;
    let x = cx - half;
    let y = cy - half;
    let corner = (size / 6).clamp(2, 14);
    // Hollow rounded outline in mauve, cut from the navy background.
    rounded_rect_filled(canvas, x, y, size, size, corner, slso8::MAUVE);
    rounded_rect_filled(
        canvas,
        x + 1,
        y + 1,
        size - 2,
        size - 2,
        (corner - 1).max(1),
        slso8::NAVY,
    );
    // Vertically centred question-mark prompt (glyph is 10 px tall).
    let text = "?";
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(
        text,
        cx - text_width / 2,
        cy - half + (size - 10) / 2,
        1,
        slso8::MAUVE,
    );
}

/// Running phase of the local game loop.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Slots empty; waiting for the player to roll.
    AwaitRoll,
    /// Dice landed; the player selects, scores, rolls or banks.
    Select,
    /// The turn ended (banked or farkle); waiting to continue.
    TurnOver,
}

/// Animates `final_values` tumbling chaotically before settling into their slots.
fn animate_roll(
    canvas: &mut Canvas,
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    delay: &mut Delay,
    final_values: &[u8],
    rng: &mut Prng,
) {
    const FRAMES: i32 = 13;
    const SETTLE_Y: i32 = 120;
    let count = final_values.len().min(DICE_COUNT);

    // Scatter the dice across the upper play area.
    let mut px = [0i32; DICE_COUNT];
    let mut py = [0i32; DICE_COUNT];
    for index in 0..count {
        px[index] = 40 + rng.next_range(170) as i32;
        py[index] = 25 + rng.next_range(100) as i32;
    }

    for frame in 0..FRAMES {
        let progress = frame as f32 / (FRAMES - 1) as f32;
        canvas.clear(slso8::NAVY);

        // Bounce around for a while, then ease into the landing slots.
        for index in 0..count {
            if progress < 0.72 {
                let dx = (rng.next_range(48) as i32) - 24;
                let dy = (rng.next_range(40) as i32) - 20;
                px[index] = (px[index] + dx).clamp(24, 214);
                py[index] = (py[index] + dy).clamp(16, 150);
            } else {
                let slot_x = die_slot(count, index);
                px[index] = px[index] + ((slot_x - px[index]) * 3) / 5;
                py[index] = py[index] + ((SETTLE_Y - py[index]) * 3) / 5;
            }

            let shown = if progress < 0.9 {
                1 + rng.next_range(6) as u8
            } else {
                final_values[index]
            };
            let size = if progress < 0.5 { DIE + 8 } else { DIE };
            draw_die(canvas, px[index], py[index], size, shown, slso8::BURNT);
        }

        flush_screen(spi, dc, cs, canvas.as_slice());
        delay.delay_millis(33);
    }
}

/// Blinks a farkled throw on screen a few times - a beat to register the miss -
/// before handing over to the turn-over (FARKLE) screen.
fn blink_farkle(
    canvas: &mut Canvas,
    spi: &mut Spi<'_, esp_hal::Blocking>,
    dc: &mut Output,
    cs: &mut Output,
    delay: &mut Delay,
    dice: &[u8],
) {
    const BLINKS: u32 = 3;
    const VISIBLE_MS: u32 = 220;
    const HIDDEN_MS: u32 = 220;
    let count = dice.len().min(DICE_COUNT);
    for _ in 0..BLINKS {
        canvas.clear(slso8::NAVY);
        for index in 0..count {
            draw_die(
                canvas,
                die_slot(count, index),
                120,
                DIE,
                dice[index],
                slso8::BURNT,
            );
        }
        flush_screen(spi, dc, cs, canvas.as_slice());
        delay.delay_millis(VISIBLE_MS);

        canvas.clear(slso8::NAVY);
        flush_screen(spi, dc, cs, canvas.as_slice());
        delay.delay_millis(HIDDEN_MS);
    }
}

/// Application entry point.
#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let usb = UsbSerialJtag::new(peripherals.USB_DEVICE);
    let (_rx, mut tx) = usb.split();
    let mut delay = Delay::new();
    log_line(&mut tx, "badge: dice starting\n");

    let mut i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("configure I2C")
    .with_sda(peripherals.GPIO38)
    .with_scl(peripherals.GPIO39);

    let expander = probe_expander(&mut i2c);
    if expander == 0 {
        log_line(&mut tx, "ERROR: expander not found on I2C\n");
        loop {}
    }

    i2c.write(expander, &[REG_CONFIG_0, PORT_0_INPUTS])
        .expect("configure expander port 0");
    i2c.write(expander, &[REG_CONFIG_1, LED_PINS_INPUT])
        .expect("configure expander port 1");

    i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
        .expect("set display reset idle");
    delay.delay_millis(10);
    i2c.write(expander, &[REG_OUTPUT_1, 0x00])
        .expect("assert display reset");
    delay.delay_millis(10);
    i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
        .expect("release display reset");
    delay.delay_millis(120);

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

    // --- Dice game (SLSO8, rotating hot-seat) ---
    let framebuffer = unsafe { &mut *(&raw mut FRAMEBUFFER) };
    let mut canvas = Canvas::new(framebuffer);

    let mut rng = Prng::new(0xd1ce);
    let mut game = Game::new();
    let mut input = Input::new();
    let mut now_ms: u32 = 0;
    let mut selector: usize = 0;
    let mut marked = [false; DICE_COUNT];
    let mut phase = Phase::AwaitRoll;
    let mut bad_frames: u32 = 0;
    let mut last_banker: usize = 0;
    let mut last_banked: u32 = 0;
    let mut last_farkle: bool = false;
    // Frames spent on the turn-over screen; drives the automatic hand-off after
    // a farkle so the player never has to "bank 0" to continue.
    let mut turnover_frames: u32 = 0;
    // Current vertical lift of each die, eased toward its target each frame
    // so moving the cursor looks smooth and fast.
    let mut lift = [0i32; DICE_COUNT];

    loop {
        now_ms = now_ms.wrapping_add(33);
        let port0 = read_expander_register(&mut i2c, expander, REG_INPUT_0);
        let active_mask = !port0;
        input.update(active_mask, now_ms);

        // Diagnostic: log every freshly pressed button by name and mask.
        if input.any_just_pressed() {
            for index in 0..8 {
                if input.just_pressed(button_at(index)) {
                    log_button_press(&mut tx, index);
                }
            }
        }

        let in_play = game.dice_count();

        // --- Input handling per phase ---
        match phase {
            Phase::AwaitRoll => {
                if input.just_pressed(Button::Btn6) {
                    let turn_player = game.current_player();
                    let thrown = game.dice_count();
                    let scorable = game.throw(&mut rng);
                    let mut values = [0u8; DICE_COUNT];
                    // Slice to the pre-throw count: on a farkle the game resets
                    // its full pool but keeps the just-rolled dice in place.
                    values[..thrown].copy_from_slice(&game.dice()[..thrown]);
                    animate_roll(
                        &mut canvas,
                        &mut spi,
                        &mut dc,
                        &mut cs,
                        &mut delay,
                        &values[..thrown],
                        &mut rng,
                    );
                    // Start the cursor on the leftmost 1 or 5 if any rolled.
                    // Nothing is pre-marked; Score falls back to the cursor die.
                    marked = [false; DICE_COUNT];
                    selector = first_scorable(game.dice()).unwrap_or(0);
                    if scorable {
                        phase = Phase::Select;
                    } else {
                        last_farkle = true;
                        last_banker = turn_player;
                        // Blink the farkled throw a few times before the FARKLE
                        // screen, so the player registers the miss.
                        let rolled = game.dice();
                        blink_farkle(
                            &mut canvas,
                            &mut spi,
                            &mut dc,
                            &mut cs,
                            &mut delay,
                            &rolled[..thrown],
                        );
                        phase = Phase::TurnOver;
                    }
                }
                // Hot dice: after scoring all five, the player may also bank the
                // turn instead of rolling again. Only possible when the turn
                // already has points (`turn_score > 0`).
                if game.turn_score() > 0 && input.just_pressed(Button::Btn8) {
                    last_banker = game.current_player();
                    last_banked = game.turn_score();
                    last_farkle = false;
                    game.bank();
                    phase = Phase::TurnOver;
                }
            }
            Phase::Select if in_play > 0 => {
                if input.just_pressed(Button::Btn1) {
                    selector = (selector + in_play - 1) % in_play;
                }
                if input.just_pressed(Button::Btn3) {
                    selector = (selector + 1) % in_play;
                }
                if input.just_pressed(Button::Btn2) {
                    marked[selector] = !marked[selector];
                }
                if input.just_pressed(Button::Btn4) {
                    let mut chosen = [0usize; DICE_COUNT];
                    let mut count = 0;
                    for index in 0..in_play {
                        if marked[index] && count < DICE_COUNT {
                            chosen[count] = index;
                            count += 1;
                        }
                    }
                    // Nothing marked: fall back to the die under the cursor so
                    // a quick "Score" still works on the selected die.
                    if count == 0 && selector < in_play {
                        chosen[count] = selector;
                        count += 1;
                    }
                    match game.score_selected(&chosen[..count]) {
                        Ok(_) => {
                            selector = 0;
                            marked = [false; DICE_COUNT];
                            // Hot dice: if every die in play was scored, the
                            // game has already reset to a fresh five, so hand
                            // back to the rolling phase for a roll-again/bank
                            // choice rather than staying in Select (where the
                            // re-roll guard would block a full pool).
                            phase = if count == in_play {
                                Phase::AwaitRoll
                            } else {
                                Phase::Select
                            };
                        }
                        Err(_) => bad_frames = 9, // invalid selection flash
                    }
                }
                // Re-roll the leftover dice still in play (Btn6).
                if in_play < DICE_COUNT && input.just_pressed(Button::Btn6) {
                    let turn_player = game.current_player();
                    let thrown = game.dice_count();
                    let scorable = game.throw(&mut rng);
                    let mut values = [0u8; DICE_COUNT];
                    // Slice to the pre-throw count: on a farkle the game resets
                    // its full pool but keeps the just-rolled dice in place.
                    values[..thrown].copy_from_slice(&game.dice()[..thrown]);
                    animate_roll(
                        &mut canvas,
                        &mut spi,
                        &mut dc,
                        &mut cs,
                        &mut delay,
                        &values[..thrown],
                        &mut rng,
                    );
                    // Start the cursor on the leftmost 1 or 5 if any rolled.
                    // Nothing is pre-marked; Score falls back to the cursor die.
                    marked = [false; DICE_COUNT];
                    selector = first_scorable(game.dice()).unwrap_or(0);
                    if scorable {
                        phase = Phase::Select;
                    } else {
                        last_farkle = true;
                        last_banker = turn_player;
                        let rolled = game.dice();
                        blink_farkle(
                            &mut canvas,
                            &mut spi,
                            &mut dc,
                            &mut cs,
                            &mut delay,
                            &rolled[..thrown],
                        );
                        phase = Phase::TurnOver;
                    }
                }
                if input.just_pressed(Button::Btn8) {
                    last_banker = game.current_player();
                    last_banked = game.turn_score();
                    last_farkle = false;
                    game.bank();
                    phase = Phase::TurnOver;
                }
            }
            _ => {
                // A farkle hands over automatically after a short beat; a bank
                // keeps waiting for B8 so the banked amount can be read.
                turnover_frames += 1;
                if input.just_pressed(Button::Btn8)
                    || (last_farkle && turnover_frames >= FARKLE_HOLD_FRAMES)
                {
                    turnover_frames = 0;
                    selector = 0;
                    marked = [false; DICE_COUNT];
                    phase = Phase::AwaitRoll;
                }
            }
        }

        if bad_frames > 0 {
            bad_frames -= 1;
        }

        // --- Render ---
        canvas.clear(slso8::NAVY);

        // Both players' banked totals, active one lit orange + underlined.
        let current_player = game.current_player();
        let active = slso8::ORANGE;
        let idle = slso8::MAUVE;
        canvas.draw_text(
            "A",
            8,
            4,
            1,
            if current_player == 0 { active } else { idle },
        );
        draw_number(&mut canvas, 34, 4, game.score(0), slso8::CREAM, 1);
        canvas.draw_text(
            "B",
            188,
            4,
            1,
            if current_player == 1 { active } else { idle },
        );
        draw_number(&mut canvas, 214, 4, game.score(1), slso8::CREAM, 1);
        let active_x = if current_player == 0 { 8 } else { 188 };
        canvas.rect(active_x, 14, 24, 2, slso8::ORANGE);

        // The turn's accounting sits above the dice row, out of the way.
        draw_centered(&mut canvas, 18, "TURN", slso8::PEACH);
        draw_number(&mut canvas, 120, 32, game.turn_score(), slso8::CREAM, 2);

        // Just below the turn score, name the player currently at the table.
        let playing: &str = if current_player == 0 {
            "PLAYING  A"
        } else {
            "PLAYING  B"
        };
        draw_centered(&mut canvas, 54, playing, slso8::PEACH);

        let dice = game.dice();
        let count = dice.len();
        match phase {
            Phase::Select => {
                // All dice share one row centre. 120 is the middle of the
                // 240-tall display.
                const CENTER_Y: i32 = 120;
                // Ease each die's lift toward its target (marked = up) so
                // marking/unmarking glides smoothly but quickly.
                for index in 0..count {
                    let target = if marked[index] { DIE_LIFT } else { 0 };
                    let mut l = lift[index];
                    let diff = target - l;
                    if diff != 0 {
                        l += diff * 3 / 5;
                        if (target - l).abs() < 2 {
                            l = target;
                        }
                        lift[index] = l;
                    }
                }
                for index in 0..count {
                    let x = die_slot(count, index);
                    let selected = index == selector;
                    let marked_die = marked[index];
                    let y = CENTER_Y - lift[index];
                    // Marked dice shrink and sit above; the cursor die is
                    // enlarged. Marking takes precedence so the change shows
                    // immediately.
                    let size = if marked_die {
                        DIE
                    } else if selected {
                        DIE + 12
                    } else {
                        DIE - 8
                    };
                    // Dies marked for scoring get a bright orange border.
                    let outline = if marked_die {
                        slso8::ORANGE
                    } else {
                        slso8::BURNT
                    };
                    draw_die(&mut canvas, x, y, size, dice[index], outline);
                }
            }
            Phase::AwaitRoll => {
                for index in 0..game.dice_count() {
                    draw_slot(&mut canvas, die_slot(game.dice_count(), index), 120, DIE);
                }
            }
            _ => {}
        }

        if bad_frames > 0 {
            draw_centered(&mut canvas, 178, "!! INVALID !!", slso8::BURNT);
        } else {
            match phase {
                Phase::AwaitRoll => {
                    if game.turn_score() > 0 {
                        // Hot dice: roll all five again or bank.
                        draw_centered(&mut canvas, 186, "ROLL B6 . BANK B8", slso8::PEACH);
                    } else {
                        draw_centered(&mut canvas, 186, "ROLL (B6)", slso8::CREAM);
                    }
                }
                Phase::Select => {
                    draw_centered(&mut canvas, 172, "MARK B2 . SCORE B4", slso8::PEACH);
                    draw_centered(&mut canvas, 184, "ROLL B6 . BANK B8", slso8::PEACH);
                }
                Phase::TurnOver => {
                    if last_farkle {
                        draw_centered(&mut canvas, 115, "FARKLE!", slso8::BURNT);
                    } else {
                        // "BANKED" label + amount, block vertically centred on
                        // the middle (120) of the 240-tall display; big font.
                        draw_centered(&mut canvas, 97, "BANKED", slso8::PEACH);
                        draw_number(&mut canvas, 120, 113, last_banked, slso8::CREAM, 3);
                    }
                    // Both players' banked totals, the one who just went lit.
                    for player in 0..2 {
                        let color = if player == last_banker { active } else { idle };
                        let line_y = 154 + player as i32 * 24;
                        draw_ledger(
                            &mut canvas,
                            120,
                            line_y,
                            player,
                            game.score(player),
                            2,
                            color,
                        );
                    }
                    draw_centered(&mut canvas, 212, "NEXT - B8", slso8::CREAM);
                }
            }
        }

        flush_screen(&mut spi, &mut dc, &mut cs, canvas.as_slice());
        delay.delay_millis(33);
    }
}
