//! Shared board bring-up: display, I/O expander, buttons and the engine
//! framebuffer.
//!
//! One firmware build runs exactly one game, selected at compile time by the
//! active cargo feature. This module initialises the shared hardware once and
//! hands a [`Board`] to that game's entry point, so every game main is free of
//! the setup boilerplate that used to be copy-pasted between them.

use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_hal::usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagTx};

use raylib_camp::canvas::{display, Canvas};
use raylib_camp::input::Input;

#[cfg(feature = "dice")]
use crate::expander;

/// TCA9535/TCA9539 register addresses.
const REG_INPUT_0: u8 = 0x00;
const REG_CONFIG_0: u8 = 0x06;
const REG_CONFIG_1: u8 = 0x07;
const REG_OUTPUT_1: u8 = 0x03;

/// Port 0 (buttons) all inputs; port 1 keeps only the display reset (P1.0) as
/// an output so the four LED pins stay inputs (off) from the first write.
const PORT_0_INPUTS: u8 = 0xff;
const LED_PINS_INPUT: u8 = 0xfe;

/// GC9A01A display reset lives on expander pin P1.0.
const DISPLAY_RESET_BIT: u8 = 0x01;

/// Framebuffer backing storage (RGB565), big enough for the whole panel.
static mut FRAMEBUFFER: [u16; display::PIXEL_COUNT] = [0; display::PIXEL_COUNT];

/// The fully-initialised badge, ready for one game to take over.
///
/// Holds every peripheral the games need and the engine framebuffer behind a
/// [`Canvas`]. Game mains borrow it mutably for the rest of the program.
pub struct Board {
    /// Engine draw surface over the panel framebuffer.
    pub canvas: Canvas<'static>,
    /// Button state machine for all eight buttons.
    pub input: Input,
    /// Millisecond clock, advanced by each call to [`Board::frame`].
    pub now_ms: u32,
    /// Serial logging sink (non-blocking).
    #[cfg(feature = "dice")]
    pub tx: UsbSerialJtagTx<'static, esp_hal::Blocking>,
    /// Blocking delay provider.
    pub delay: Delay,
    /// I2C bus to the expander.
    pub i2c: I2c<'static, esp_hal::Blocking>,
    /// Address the expander answered on the I2C bus at.
    pub expander: u8,
    /// PWM peripheral shared by the buzzer melodies.
    #[cfg(feature = "dice")]
    pub ledc: esp_hal::ledc::Ledc<'static>,
    spi: Spi<'static, esp_hal::Blocking>,
    dc: Output<'static>,
    cs: Output<'static>,
    /// Shadow of the expander port 1 direction register.
    #[cfg(feature = "dice")]
    config1: u8,
    /// Shadow of the expander port 1 output register.
    #[cfg(feature = "dice")]
    output1: u8,
}

/// Emits a log line without ever blocking the caller.
pub fn log_line(tx: &mut UsbSerialJtagTx<'_, esp_hal::Blocking>, line: &str) {
    for byte in line.bytes() {
        if tx.write_byte_nb(byte).is_err() {
            break;
        }
    }
    let _ = tx.flush_tx_nb();
}

impl Board {
    /// Brings up every peripheral the badge firmware needs.
    pub fn new() -> Self {
        let peripherals = esp_hal::init(esp_hal::Config::default());
        let usb = UsbSerialJtag::new(peripherals.USB_DEVICE);
        let (_rx, mut tx) = usb.split();
        let mut delay = Delay::new();

        #[cfg(feature = "dice")]
        let ledc = {
            let mut ledc = esp_hal::ledc::Ledc::new(peripherals.LEDC);
            ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);
            ledc
        };

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
        #[cfg(feature = "dice")]
        let config1 = LED_PINS_INPUT;

        i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
            .expect("set display reset idle");
        delay.delay_millis(10);
        i2c.write(expander, &[REG_OUTPUT_1, 0x00])
            .expect("assert display reset");
        delay.delay_millis(10);
        i2c.write(expander, &[REG_OUTPUT_1, DISPLAY_RESET_BIT])
            .expect("release display reset");
        delay.delay_millis(120);
        #[cfg(feature = "dice")]
        let output1 = DISPLAY_RESET_BIT;

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

        let framebuffer: &'static mut [u16] = unsafe { &mut *(&raw mut FRAMEBUFFER) };
        let canvas = Canvas::new(framebuffer);

        log_line(&mut tx, "badge: display initialised\n");
        Self {
            canvas,
            input: Input::new(),
            now_ms: 0,
            #[cfg(feature = "dice")]
            tx,
            delay,
            i2c,
            expander,
            #[cfg(feature = "dice")]
            ledc,
            spi,
            dc,
            cs,
            #[cfg(feature = "dice")]
            config1,
            #[cfg(feature = "dice")]
            output1,
        }
    }

    /// Advances the frame clock and polls the eight buttons through the
    /// expander, feeding the results into the engine's input state machine.
    pub fn frame(&mut self) {
        self.now_ms = self.now_ms.wrapping_add(33);
        let mut value = [0u8; 1];
        let _ = self
            .i2c
            .write_read(self.expander, &[REG_INPUT_0], &mut value);
        self.input.update(!value[0], self.now_ms);
    }

    /// Pushes the full framebuffer out to the display.
    pub fn flush(&mut self) {
        flush_screen(
            &mut self.spi,
            &mut self.dc,
            &mut self.cs,
            self.canvas.as_slice(),
        );
    }

    /// Turns a seat/power LED on or off through the expander.
    #[cfg(feature = "dice")]
    pub fn set_led(&mut self, led: expander::Led, on: bool) {
        expander::set_led(
            &mut self.i2c,
            self.expander,
            &mut self.config1,
            &mut self.output1,
            led,
            on,
        );
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

/// Probes the I2C bus for the badge's I/O expander, which is addressed in the
/// TCA9539 range depending on its address pins.
fn probe_expander(i2c: &mut I2c<'_, esp_hal::Blocking>) -> u8 {
    let mut dummy = [0u8; 1];
    for address in 0x74..=0x77 {
        if i2c.read(address, &mut dummy).is_ok() {
            return address;
        }
    }
    0
}
