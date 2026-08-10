//! Driver for the TCA9539 I/O expander's four controllable LEDs.
//!
//! The LEDs live on port 1 pins P1.1..P1.4 and are driven purely over I2C.
//! Because the expander's config and output registers cannot be reliably read
//! back (the output register returns pin level rather than the latch, and a
//! floating input reads garbage), every write here pushes the full byte from
//! a software shadow kept by the caller. Turning an LED on latches it low
//! *before* switching the pin to an output so it never glitches high, and
//! turning it off floats the pin by switching it to an input (the LEDs are fed
//! from 5V while the expander can only pull up to VCC).

use esp_hal::i2c::master::I2c;

/// TCA9539 register holding the port 1 output latch.
const REG_OUTPUT_1: u8 = 0x03;
/// TCA9539 register holding the port 1 direction (set bit = input).
const REG_CONFIG_1: u8 = 0x07;

/// A controllable LED on the badge, identified by its port 1 pin.
///
/// All four physical LEDs are enumerated; the two currently unused by the game
/// logic are retained so later effects (farkle, winner) can drive them.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Led {
    /// Green LED on expander pin P1.1.
    Green,
    /// Blue LED on expander pin P1.2.
    Blue,
    /// Red LED on expander pin P1.3.
    Red,
    /// Yellow LED on expander pin P1.4.
    Yellow,
}

impl Led {
    /// Returns the port 1 bit mask for this LED.
    fn bit(self) -> u8 {
        match self {
            Led::Green => 0x02,
            Led::Blue => 0x04,
            Led::Red => 0x08,
            Led::Yellow => 0x10,
        }
    }
}

/// Turns a single [`Led`] on or off through the I/O expander.
///
/// `config1` and `output1` shadow the current register contents and are
/// updated in place so the caller keeps them across calls. See the module
/// documentation for why the shadow registers exist.
pub fn set_led(
    i2c: &mut I2c<'_, esp_hal::Blocking>,
    address: u8,
    config1: &mut u8,
    output1: &mut u8,
    led: Led,
    on: bool,
) {
    let mask = led.bit();
    if on {
        if *output1 & mask == 0 && *config1 & mask == 0 {
            return;
        }
        // Latch low first, then switch direction, so the pin does not briefly
        // drive high while becoming an output.
        *output1 &= !mask;
        i2c.write(address, &[REG_OUTPUT_1, *output1])
            .expect("latch LED output low");
        if *config1 & mask != 0 {
            *config1 &= !mask;
            i2c.write(address, &[REG_CONFIG_1, *config1])
                .expect("switch LED pin to output");
        }
    } else if *config1 & mask == 0 {
        // Float the pin to switch the LED off; the latch is left untouched.
        *config1 |= mask;
        i2c.write(address, &[REG_CONFIG_1, *config1])
            .expect("switch LED pin to input");
    }
}
