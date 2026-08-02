/**
 * BadgePins.h - pin assignments and hardware constants for the LuxCamp Badge 2026.
 *
 * Everything that describes *how the badge is wired* lives in this file, so
 * there is exactly one place to look (and one place to change if you use a
 * different microcontroller than the M5Stack AtomS3 Lite).
 */
#pragma once

#include <stdint.h>

// ---------------------------------------------------------------------------
// Microcontroller pins
// ---------------------------------------------------------------------------
//
// The badge is a bring-your-own-microcontroller board, so the pin numbers
// depend on what you plugged into it. Two boards are supported out of the box;
// the board macro comes from `board = ...` in platformio.ini.
//
// The M5Stack Atom Lite (ESP32) and AtomS3 Lite (ESP32-S3) have the same
// footprint, so the same badge header positions end up on different GPIO
// numbers. The mapping between them is:
//
//     header position   AtomS3 Lite   Atom Lite
//     ---------------   -----------   ---------
//     TFT_SCL           G5            G22
//     TFT_SDA           G6            G19
//     TFT_DC            G7            G23
//     TFT_CS            G8            G33
//     I2C_SDA           G38           G25
//     I2C_SCL           G39           G21
//     BZR               G2            G26
//
// Errata: on the 5 pin header the silkscreen labels for TFT_CS and TFT_DC are
// swapped. The values below follow the schematic (and the labels on the 7 pin
// header), which are the correct ones. If your display stays black, swapping
// these two is the first thing to try.

#if defined(ARDUINO_M5Stack_ATOMS3)

// M5Stack AtomS3 Lite (ESP32-S3)
static const int8_t BADGE_PIN_BUZZER = 2;   ///< Passive buzzer, driven with LEDC
static const int8_t BADGE_PIN_TFT_SCL = 5;  ///< Display SPI clock
static const int8_t BADGE_PIN_TFT_SDA = 6;  ///< Display SPI data (MOSI)
static const int8_t BADGE_PIN_TFT_DC = 7;   ///< Display data/command select
static const int8_t BADGE_PIN_TFT_CS = 8;   ///< Display chip select
static const int8_t BADGE_PIN_I2C_SDA = 38; ///< I2C data, goes to the I/O expander
static const int8_t BADGE_PIN_I2C_SCL = 39; ///< I2C clock, goes to the I/O expander

#elif defined(ARDUINO_M5Stack_ATOM)

// M5Stack Atom Lite (ESP32-PICO-D4)
static const int8_t BADGE_PIN_BUZZER = 26;
static const int8_t BADGE_PIN_TFT_SCL = 22;
static const int8_t BADGE_PIN_TFT_SDA = 19;
static const int8_t BADGE_PIN_TFT_DC = 23;
static const int8_t BADGE_PIN_TFT_CS = 33;
static const int8_t BADGE_PIN_I2C_SDA = 25;
static const int8_t BADGE_PIN_I2C_SCL = 21;

#else
#error "Unknown board. Add its pin numbers to BadgePins.h, or build with board = m5stack-atom / m5stack-atoms3."
#endif

/// The display has no MISO line and no separate reset pin on the MCU side;
/// reset is driven by the I/O expander instead (see BADGE_IO_BIT_TFT_RST).
static const int8_t BADGE_PIN_TFT_MISO = -1;

// ---------------------------------------------------------------------------
// Display (GC9A01A, 1.28 inch, 240x240, round)
// ---------------------------------------------------------------------------

static const int16_t BADGE_TFT_WIDTH = 240;
static const int16_t BADGE_TFT_HEIGHT = 240;

/// Centre of the round screen. Anything you draw should stay within
/// BADGE_TFT_RADIUS of this point or it will be hidden behind the bezel.
static const int16_t BADGE_TFT_CX = 120;
static const int16_t BADGE_TFT_CY = 120;
static const int16_t BADGE_TFT_RADIUS = 120;

/// SPI clock for the panel. 40 MHz is comfortable; drop to 27 MHz if you use
/// long jumper wires and see corrupted pixels.
static const uint32_t BADGE_TFT_SPI_HZ = 40000000UL;

// ---------------------------------------------------------------------------
// I/O expander (UMW-TCA9539PWR, TCA9535-compatible register layout)
// ---------------------------------------------------------------------------

/// Register map. Each register exists once per 8 bit port (port 0 and port 1).
static const uint8_t BADGE_IO_REG_INPUT_0 = 0x00;
static const uint8_t BADGE_IO_REG_INPUT_1 = 0x01;
static const uint8_t BADGE_IO_REG_OUTPUT_0 = 0x02;
static const uint8_t BADGE_IO_REG_OUTPUT_1 = 0x03;
static const uint8_t BADGE_IO_REG_POLARITY_0 = 0x04;
static const uint8_t BADGE_IO_REG_POLARITY_1 = 0x05;
/// Configuration register: a set bit means "input", a cleared bit means "output".
static const uint8_t BADGE_IO_REG_CONFIG_0 = 0x06;
static const uint8_t BADGE_IO_REG_CONFIG_1 = 0x07;

/// Candidate I2C addresses probed by BadgeIOExpander::begin().
/// 0x74..0x77 are the TCA9539 range, 0x20..0x27 the TCA9535 range.
///
/// The 2026 badges measured so far answer at 0x77. The probe is kept anyway so
/// a differently strapped board still works; pass the address explicitly to
/// begin() if you would rather skip it.
static const uint8_t BADGE_IO_ADDRESSES[] = {0x74, 0x75, 0x76, 0x77, 0x20, 0x21,
                                             0x22, 0x23, 0x24, 0x25, 0x26, 0x27};
static const uint8_t BADGE_IO_ADDRESS_COUNT =
    sizeof(BADGE_IO_ADDRESSES) / sizeof(BADGE_IO_ADDRESSES[0]);

/// Port 1 bit that holds the display in reset while pulled low.
static const uint8_t BADGE_IO_BIT_TFT_RST = 0;

/// The eight buttons, wired to P0.0 - P0.7. They read low while pressed.
enum BadgeButton : uint8_t {
  BADGE_BTN1 = 0,
  BADGE_BTN2 = 1,
  BADGE_BTN3 = 2,
  BADGE_BTN4 = 3,
  BADGE_BTN5 = 4,
  BADGE_BTN6 = 5,
  BADGE_BTN7 = 6,
  BADGE_BTN8 = 7,
};
static const uint8_t BADGE_BUTTON_COUNT = 8;

/// The four LEDs, wired to P1.1 - P1.4.
///
/// Errata: some badges were assembled with two blue LEDs, so BADGE_LED_RED may
/// glow blue on your board.
enum BadgeLed : uint8_t {
  BADGE_LED_GREEN = 0,
  BADGE_LED_BLUE = 1,
  BADGE_LED_RED = 2,
  BADGE_LED_YELLOW = 3,
};
static const uint8_t BADGE_LED_COUNT = 4;

/// Port 1 bit for a given LED: green is P1.1, so the offset is one.
static inline uint8_t badgeLedBit(BadgeLed led) {
  return (uint8_t)(led + 1);
}
