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
use raylib_camp::color::{Color, palette};
use raylib_camp::input::{Button, Input};
use raylib_camp::rand::Prng;

use games::sevens::{Card, Game, PlayOutcome, Suit, SUITS, PLAYER_COUNT, MAX_HAND};

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

/// Sorts a slice of cards by suit, then by descending rank within each suit
/// (all suits grouped together, each fanned from its highest card down).
fn sort_cards(cards: &mut [Card]) {
    let comes_after = |a: Card, b: Card| -> bool {
        a.suit.index() > b.suit.index()
            || (a.suit.index() == b.suit.index() && a.rank < b.rank)
    };
    for index in 1..cards.len() {
        let mut cursor = index;
        while cursor > 0 && comes_after(cards[cursor - 1], cards[cursor]) {
            cards.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }
}

/// Appends a card's compact rank+suit label (e.g. `"8H"`) into a stack buffer.
fn card_label(card: Card, out: &mut [u8; 3]) -> &str {
    let rank = card.rank_label().as_bytes();
    let suit = card.suit.label().as_bytes();
    let mut count = 0;
    for &byte in rank {
        if count < 2 {
            out[count] = byte;
            count += 1;
        }
    }
    out[count] = suit[0];
    count += 1;
    core::str::from_utf8(&out[..count]).unwrap_or("?")
}

/// Draws a small card face with an optional highlight (the selected rack card).
fn draw_card(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, card: Card, selected: bool) {
    let background = if selected { palette::YELLOW } else { palette::WHITE };
    canvas.rect_filled(x, y, w, h, background);
    canvas.rect(x, y, w, h, palette::GREY);
    let face_color = if card.suit == Suit::Hearts || card.suit == Suit::Diamonds {
        palette::RED
    } else {
        palette::BLACK
    };
    let mut label = [0u8; 3];
    let text = card_label(card, &mut label);
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(text, x + (w - text_width) / 2, y + 1, 1, face_color);
}

/// Draws a decimal number centred around the given horizontal point.
fn draw_number(canvas: &mut Canvas, x: i32, y: i32, value: u32, color: Color) {
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
        let text_width = canvas.measure_text(text, 1);
        canvas.draw_text(text, x - text_width / 2, y, 1, color);
    }
}

/// Centred one-line helper label.
fn draw_centered(canvas: &mut Canvas, y: i32, text: &str, color: Color) {
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(text, 120 - text_width / 2, y, 1, color);
}

/// Draws the four suited piles in a row across the middle of the round panel.
fn draw_board(canvas: &mut Canvas, game: &Game) {
    const PILE_W: i32 = 46;
    const PILE_H: i32 = 34;
    let start_x = (240 - ((4 * PILE_W) + (3 * 8))) / 2;
    for (index, suit) in SUITS.iter().enumerate() {
        let x = start_x + index as i32 * (PILE_W + 8);
        match game.pile_state(index) {
            Some((low, high)) => {
                canvas.rect_filled(x, 96, PILE_W, PILE_H, palette::WHITE);
                canvas.rect(x, 96, PILE_W, PILE_H, palette::GREY);
                canvas.draw_text(suit.label(), x + 2, 97, 1, palette::BLACK);
                let mut range = [0u8; 7];
                let range_text = range_label(low, high, &mut range);
                let rw = canvas.measure_text(range_text, 1);
                canvas.draw_text(range_text, x + (PILE_W - rw) / 2, 114, 1, palette::BLACK);
            }
            None => {
                canvas.rect_filled(x, 96, PILE_W, PILE_H, palette::GREY);
                canvas.draw_text(suit.label(), x + 2, 97, 1, palette::WHITE);
            }
        }
    }
}

/// Renders the three opponent card-back stacks on the other sides of the table.
fn draw_opponents(canvas: &mut Canvas, game: &Game, current: usize) {
    let right = (current + 1) % PLAYER_COUNT;
    let top = (current + 2) % PLAYER_COUNT;
    let left = (current + 3) % PLAYER_COUNT;

    draw_opponent(canvas, 116, 6, game.hand(top).len());
    draw_opponent(canvas, 6, 120, game.hand(left).len());
    draw_opponent(canvas, 220, 120, game.hand(right).len());
}

fn draw_opponent(canvas: &mut Canvas, x: i32, y: i32, count: usize) {
    canvas.rect_filled(x, y, 11, 15, palette::BLUE);
    canvas.rect(x, y, 11, 15, palette::GREY);
    draw_number(canvas, x + 20, y + 2, count as u32, palette::WHITE);
}

/// Draws the current player's hand rack at the bottom plus the status line.
fn draw_hand_rack(canvas: &mut Canvas, cards: &[Card], cursor: usize, game: &Game) {
    let count = cards.len();
    if count > 0 {
        let card_w = (228 / count as i32).clamp(8, 22);
        let start_x = (240 - count as i32 * card_w) / 2;
        const BASE_Y: i32 = 212;
        for (index, card) in cards.iter().enumerate() {
            // The selected card is raised so it reads as the active choice.
            let y = if index == cursor { BASE_Y - 6 } else { BASE_Y };
            draw_card(canvas, start_x + index as i32 * card_w, y, card_w - 1, 26, *card, index == cursor);
        }
    }

    if game.round_over() {
        if game.game_over() {
            draw_centered(canvas, 150, "GAME OVER", palette::RED);
        } else {
            draw_centered(canvas, 150, "ROUND OVER - B8", palette::RED);
        }
    } else if game.current_must_draw() {
        draw_centered(canvas, 196, "No move - press B6", palette::ORANGE);
    }
}

/// Builds a `low-high` range label such as `"6-K"`.
fn range_label(low: u8, high: u8, out: &mut [u8; 7]) -> &str {
    let mut count = 0;
    for &byte in rank_label(low).as_bytes() {
        out[count] = byte;
        count += 1;
    }
    out[count] = b'-';
    count += 1;
    for &byte in rank_label(high).as_bytes() {
        out[count] = byte;
        count += 1;
    }
    core::str::from_utf8(&out[..count]).unwrap_or("")
}

/// Short label for a rank value (Ace..King).
fn rank_label(rank: u8) -> &'static str {
    match rank {
        1 => "A",
        11 => "J",
        12 => "Q",
        13 => "K",
        value => {
            const DIGITS: [&str; 9] = ["2", "3", "4", "5", "6", "7", "8", "9", "10"];
            DIGITS[(value - 2) as usize]
        }
    }
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

    // --- Sevens game, rotating hot-seat mode ---
    let framebuffer = unsafe { &mut *(&raw mut FRAMEBUFFER) };
    let mut canvas = Canvas::new(framebuffer);
    log_line(&mut tx, "sevens starting\n");

    let mut rng = Prng::new(0x53e7);
    let mut game = Game::new(&mut rng);
    let mut input = Input::new();
    let mut now_ms: u32 = 0;
    let mut hand_cursor: usize = 0;

    loop {
        now_ms = now_ms.wrapping_add(33);

        let port0 = read_expander_register(&mut i2c, expander, REG_INPUT_0);
        let active_mask = !port0;
        input.update(active_mask, now_ms);

        let current = game.current_player();
        // Ordered copy of the current player's hand for the bottom rack.
        let mut hand_cards = [Card::new(Suit::Spades, 1).unwrap(); MAX_HAND];
        let mut hand_len = 0;
        for card in game.hand(current).iter() {
            hand_cards[hand_len] = card;
            hand_len += 1;
        }
        sort_cards(&mut hand_cards[..hand_len]);
        if hand_cursor >= hand_len {
            hand_cursor = hand_len.saturating_sub(1);
        }

        if !game.round_over() && !game.game_over() {
            // Move the cursor across the hand rack (left/right).
            if input.just_pressed(Button::Btn1) {
                hand_cursor = (hand_cursor + hand_len - 1) % hand_len.max(1);
            }
            if input.just_pressed(Button::Btn3) {
                hand_cursor = (hand_cursor + 1) % hand_len.max(1);
            }

            // Play the selected card (Btn5 or Btn7).
            if input.just_pressed(Button::Btn5) || input.just_pressed(Button::Btn7) {
                let selection = hand_cards
                    .get(hand_cursor.min(hand_len.saturating_sub(1)))
                    .copied();
                if let Some(card) = selection {
                    match game.play(card) {
                        Ok(PlayOutcome::Played) => {
                            log_line(&mut tx, "played\n");
                            hand_cursor = 0;
                        }
                        Ok(PlayOutcome::RoundFinished) => log_line(&mut tx, "round finished\n"),
                        Err(_) => log_line(&mut tx, "illegal\n"),
                    }
                }
            }

            // Pass / draw (Btn6): draw one card from each opponent and skip the
            // turn, whether stuck or choosing to hold back.
            if input.just_pressed(Button::Btn6) {
                game.draw_from_others(&mut rng);
                hand_cursor = 0;
            }
        } else if input.just_pressed(Button::Btn8) {
            // Next round, or a brand-new game after reaching 250 points.
            if game.game_over() {
                game = Game::new(&mut rng);
            } else {
                game.next_round(&mut rng);
            }
            hand_cursor = 0;
        }

        // --- Render the rotating table, current player always at the bottom. ---
        canvas.clear(palette::BLACK);
        draw_board(&mut canvas, &game);
        draw_opponents(&mut canvas, &game, current);
        draw_hand_rack(&mut canvas, &hand_cards[..hand_len], hand_cursor, &game);

        flush_screen(&mut spi, &mut dc, &mut cs, canvas.as_slice());
        delay.delay_millis(33);
    }
}
