/**
 * BadgeButtons.h - debounced reading of the badge's eight buttons.
 *
 * BadgeIOExpander can read the buttons directly, but raw reads bounce: a
 * single press produces a burst of on/off transitions for a few milliseconds.
 * This class filters that out and turns the result into press and release
 * events you can act on once each.
 *
 * Typical use:
 *
 *   badge.buttons.update();                  // call every loop()
 *   if (badge.buttons.wasPressed(BADGE_BTN1)) { ... }
 */
#pragma once

#include <Arduino.h>

#include "BadgeIOExpander.h"

/// How long the reading has to stay unchanged before it counts.
static const uint32_t BADGE_DEBOUNCE_MS = 20;

class BadgeButtons {
public:
  void begin(BadgeIOExpander &io);

  /// Reads the buttons and updates the press/release events. Call this once
  /// per loop() - it is cheap but it does perform one I2C read.
  void update();

  /// True for as long as the button is held down.
  bool isPressed(BadgeButton button) const;

  /// True for exactly one update() after the button went down.
  bool wasPressed(BadgeButton button) const;

  /// True for exactly one update() after the button came back up.
  bool wasReleased(BadgeButton button) const;

  /// True if any button is currently held.
  bool anyPressed() const { return _pressed != 0; }

  /// Bitmask of the currently held buttons, bit 0 = BADGE_BTN1.
  uint8_t pressedMask() const { return _pressed; }

  /// The unfiltered port 0 byte from the last update(). Useful when bringing
  /// up a new board: a bit reads 0 while its button is pressed.
  uint8_t rawPort() const { return _raw; }

private:
  BadgeIOExpander *_io = nullptr;

  uint8_t _raw = 0xFF;       ///< last raw read, active low
  uint8_t _candidate = 0xFF; ///< reading we are waiting to become stable
  uint8_t _stable = 0xFF;    ///< debounced reading, active low
  uint32_t _changedAt = 0;

  uint8_t _pressed = 0;      ///< active high, bit set = held
  uint8_t _pressedEdges = 0;
  uint8_t _releasedEdges = 0;
};
