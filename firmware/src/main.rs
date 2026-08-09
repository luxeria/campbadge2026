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

use raylib_camp::canvas::{Canvas, display};
use raylib_camp::color::Color;
use raylib_camp::input::{Button, Input};
use raylib_camp::rand::Prng;

use games::dice::{Game, DICE_COUNT, slso8};

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
fn read_expander_register(
    i2c: &mut I2c<'_, esp_hal::Blocking>,
    address: u8,
    register: u8,
) -> u8 {
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
fn draw_pn(canvas: &mut Canvas, x: i32, y: i32, player: usize, color: Color) {
    let mut buffer = *b"P0";
    buffer[1] = b'0' + player as u8;
    if let Ok(text) = core::str::from_utf8(&buffer) {
        let text_width = canvas.measure_text(text, 1);
        canvas.draw_text(text, x - text_width / 2, y, 1, color);
    }
}

/// Horizontal centre of the `i`-th die in a row of `count`.
fn die_slot(count: usize, index: usize) -> i32 {
    let span = (count as i32 - 1) * SLOT_SPACING;
    120 - span / 2 + index as i32 * SLOT_SPACING
}

/// Draws a single die face with its pip layout.
fn draw_die(canvas: &mut Canvas, cx: i32, cy: i32, size: i32, value: u8) {
    let half = size / 2;
    canvas.rect_filled(cx - half, cy - half, size, size, slso8::CREAM);
    canvas.rect(cx - half, cy - half, size, size, slso8::BURNT);

    let step = size / 4;
    let pip = (size / 7).max(2);
    let dots: &[(i32, i32)] = match value {
        1 => &[(0, 0)],
        2 => &[(-step, -step), (step, step)],
        3 => &[(-step, -step), (0, 0), (step, step)],
        4 => &[(-step, -step), (step, -step), (-step, step), (step, step)],
        5 => &[(-step, -step), (step, -step), (0, 0), (-step, step), (step, step)],
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
        canvas.rect_filled(cx + dx - pip / 2, cy + dy - pip / 2, pip, pip, slso8::NAVY);
    }
}

/// Draws an empty die slot (used before a roll).
fn draw_slot(canvas: &mut Canvas, cx: i32, cy: i32, size: i32) {
    let half = size / 2;
    canvas.rect(cx - half, cy - half, size, size, slso8::MAUVE);
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
    const SETTLE_Y: i32 = 112;
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
            draw_die(canvas, px[index], py[index], size, shown);
        }

        flush_screen(spi, dc, cs, canvas.as_slice());
        delay.delay_millis(33);
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
                        &mut canvas, &mut spi, &mut dc, &mut cs, &mut delay,
                        &values[..thrown], &mut rng,
                    );
                    selector = 0;
                    marked = [false; DICE_COUNT];
                    if !scorable {
                        last_farkle = true;
                        last_banker = turn_player;
                    }
                    phase = if scorable { Phase::Select } else { Phase::TurnOver };
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
                    match game.score_selected(&chosen[..count]) {
                        Ok(_) => {
                            selector = 0;
                            marked = [false; DICE_COUNT];
                            phase = if game.dice_count() == 0 {
                                Phase::AwaitRoll // hot dice
                            } else {
                                Phase::Select
                            };
                        }
                        Err(_) => bad_frames = 9, // invalid selection flash
                    }
                }
                if input.just_pressed(Button::Btn6) {
                    let turn_player = game.current_player();
                    let thrown = game.dice_count();
                    let scorable = game.throw(&mut rng);
                    let mut values = [0u8; DICE_COUNT];
                    // Slice to the pre-throw count: on a farkle the game resets
                    // its full pool but keeps the just-rolled dice in place.
                    values[..thrown].copy_from_slice(&game.dice()[..thrown]);
                    animate_roll(
                        &mut canvas, &mut spi, &mut dc, &mut cs, &mut delay,
                        &values[..thrown], &mut rng,
                    );
                    selector = 0;
                    marked = [false; DICE_COUNT];
                    if !scorable {
                        last_farkle = true;
                        last_banker = turn_player;
                    }
                    phase = if scorable { Phase::Select } else { Phase::TurnOver };
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
                if input.just_pressed(Button::Btn8) {
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
        canvas.draw_text("P0", 8, 4, 1, if current_player == 0 { active } else { idle });
        draw_number(&mut canvas, 34, 4, game.score(0), slso8::CREAM, 1);
        canvas.draw_text("P1", 188, 4, 1, if current_player == 1 { active } else { idle });
        draw_number(&mut canvas, 214, 4, game.score(1), slso8::CREAM, 1);
        let active_x = if current_player == 0 { 8 } else { 188 };
        canvas.rect(active_x, 14, 24, 2, slso8::ORANGE);

        // The turn's accounting sits above the dice row, out of the way.
        draw_centered(&mut canvas, 18, "TURN", slso8::PEACH);
        draw_number(&mut canvas, 120, 26, game.turn_score(), slso8::CREAM, 1);

        let dice = game.dice();
        let count = dice.len();
        match phase {
            Phase::Select => {
                // All dice share one centre so the active die grows in place
                // instead of shifting upward when it gets larger.
                const CENTER_Y: i32 = 106;
                for index in 0..count {
                    let x = die_slot(count, index);
                    let selected = index == selector;
                    let marked_die = marked[index];
                    let size = if selected {
                        DIE + 10
                    } else if marked_die {
                        DIE + 4
                    } else {
                        DIE - 8
                    };
                    draw_die(&mut canvas, x, CENTER_Y, size, dice[index]);
                    if marked_die {
                        draw_centered(
                            &mut canvas,
                            CENTER_Y + size / 2 + 6,
                            "*",
                            slso8::ORANGE,
                        );
                    }
                }
            }
            _ => {
                for index in 0..game.dice_count() {
                    draw_slot(&mut canvas, die_slot(game.dice_count(), index), 104, DIE);
                }
            }
        }

        if bad_frames > 0 {
            draw_centered(&mut canvas, 178, "!! INVALID !!", slso8::BURNT);
        } else {
            match phase {
                Phase::AwaitRoll => {
                    draw_centered(&mut canvas, 186, "ROLL (B6)", slso8::CREAM);
                }
                Phase::Select => {
                    draw_centered(&mut canvas, 172, "MARK B2 . SCORE B4", slso8::PEACH);
                    draw_centered(&mut canvas, 184, "ROLL B6 . BANK B8", slso8::PEACH);
                }
                Phase::TurnOver => {
                    if last_farkle {
                        draw_centered(&mut canvas, 136, "FARKLE!", slso8::BURNT);
                    } else {
                        draw_number(&mut canvas, 120, 128, last_banked, slso8::CREAM, 2);
                    }
                    draw_pn(&mut canvas, 120, 150, last_banker, slso8::ORANGE);
                    draw_number(&mut canvas, 120, 160, game.score(last_banker), slso8::CREAM, 1);
                    draw_centered(&mut canvas, 184, "NEXT - B8", slso8::CREAM);
                }
            }
        }

        flush_screen(&mut spi, &mut dc, &mut cs, canvas.as_slice());
        delay.delay_millis(33);
    }
}
