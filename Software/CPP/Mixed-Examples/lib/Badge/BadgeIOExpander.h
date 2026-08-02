/**
 * BadgeIOExpander.h - driver for the badge's UMW-TCA9539PWR I/O expander.
 *
 * The expander sits on the I2C bus and carries almost all of the badge's
 * digital I/O:
 *
 *   port 0  P0.0 - P0.7   the eight buttons (read low while pressed)
 *   port 1  P1.0          display reset
 *           P1.1 - P1.4   the four LEDs
 *           P1.5 - P1.7   free, broken out on the GPIO header
 *
 * The one thing worth knowing before reading the code: an LED is turned *off*
 * by configuring its pin as an input, not by driving it high. See ledOff().
 */
#pragma once

#include <Arduino.h>
#include <Wire.h>

#include "BadgePins.h"

class BadgeIOExpander {
public:
  /**
   * Initialises the I2C bus, finds the expander and puts every pin into a
   * known state: buttons as inputs, LEDs off, display held out of reset.
   *
   * @param wire    I2C bus to use. Wire is already set up with the badge pins.
   * @param address I2C address, or 0 to probe BADGE_IO_ADDRESSES automatically.
   * @return true if the expander answered.
   */
  bool begin(TwoWire &wire = Wire, uint8_t address = 0);

  /// I2C address the expander was found at, or 0 if begin() failed.
  uint8_t address() const { return _address; }
  bool detected() const { return _address != 0; }

  // -- Buttons --------------------------------------------------------------

  /// Reads all eight buttons at once and returns the raw port 0 byte.
  /// A bit reads 0 while its button is pressed. Returns 0xFF (nothing pressed)
  /// if the I2C read fails, so a loose cable looks like "no input" rather than
  /// like every button being held down.
  uint8_t readButtons();

  /// Convenience wrapper around readButtons() for a single button.
  bool isPressed(BadgeButton button);

  // -- LEDs -----------------------------------------------------------------

  /**
   * Turns an LED on by driving its pin low.
   *
   * The output latch is cleared *before* the pin is switched to output, so the
   * pin never briefly drives high on the way there.
   */
  void ledOn(BadgeLed led);

  /**
   * Turns an LED off by configuring its pin as an input.
   *
   * This looks odd but is required by the hardware: the LEDs are fed from 5V
   * while the expander can only pull up to VCC. On a 3.3V board like the
   * AtomS3 Lite, driving the pin high leaves enough voltage across the LED to
   * keep it dimly lit. Letting the pin float switches it off properly.
   */
  void ledOff(BadgeLed led);

  void setLed(BadgeLed led, bool on);
  void setAllLeds(bool on);

  // -- Display reset --------------------------------------------------------

  /**
   * Pulses P1.0 to hardware-reset the display, then waits for the panel to
   * come back. Call this before initialising the display driver.
   *
   * Unlike the LEDs this pin is driven both ways: the panel runs off VCC, so
   * pulling it up to VCC is a perfectly good logic high.
   */
  void resetDisplay();

  // -- Low level ------------------------------------------------------------

  bool writeRegister(uint8_t reg, uint8_t value);
  bool readRegister(uint8_t reg, uint8_t &value);

  /// Number of I2C transactions that failed since begin(). Handy for spotting
  /// a flaky connection.
  uint32_t errorCount() const { return _errors; }

private:
  TwoWire *_wire = &Wire;
  uint8_t _address = 0;
  uint32_t _errors = 0;

  /// Shadow copies of the two write-only-in-practice registers of port 1.
  /// Bit set in _config1 means "input", cleared means "output".
  uint8_t _config1 = 0xFF;
  uint8_t _output1 = 0xFF;

  void setPort1PinOutput(uint8_t bit, bool high);
  void setPort1PinInput(uint8_t bit);
};
