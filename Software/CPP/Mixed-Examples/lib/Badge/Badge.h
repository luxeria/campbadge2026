/**
 * Badge.h - everything on the LuxCamp Badge 2026 in one object.
 *
 * Include this single header and use the global `badge` instance:
 *
 *   #include <Badge.h>
 *
 *   void setup() {
 *     Serial.begin(115200);
 *     badge.begin();
 *     badge.display.centerText("Hello", BADGE_TFT_CX, BADGE_TFT_CY,
 *                              GC9A01A_WHITE);
 *   }
 *
 *   void loop() {
 *     badge.update();
 *     if (badge.buttons.wasPressed(BADGE_BTN1)) {
 *       badge.io.ledOn(BADGE_LED_GREEN);
 *       badge.buzzer.beep();
 *     }
 *   }
 *
 * If you only need one component you can use the individual drivers directly
 * (BadgeIOExpander, BadgeButtons, BadgeBuzzer, BadgeDisplay) - the examples
 * under examples/ do exactly that.
 */
#pragma once

#include <Arduino.h>

#include "BadgeButtons.h"
#include "BadgeBuzzer.h"
#include "BadgeDisplay.h"
#include "BadgeIOExpander.h"
#include "BadgePins.h"

class Badge {
public:
  BadgeIOExpander io;
  BadgeButtons buttons;
  BadgeBuzzer buzzer;
  BadgeDisplay display;

  /**
   * Brings up the badge in the order the hardware requires: I2C and the I/O
   * expander first (it owns the display's reset line), then the buttons, the
   * buzzer and finally the display.
   *
   * @param withDisplay set to false to skip display init, which saves about a
   *        fifth of a second on boot if your sketch does not draw anything
   * @return false if the I/O expander did not answer on the I2C bus. The
   *         buzzer still works in that case - it is wired straight to a GPIO.
   */
  bool begin(bool withDisplay = true);

  /// Services the button debouncing and the buzzer timing. Call this at the
  /// top of every loop(); nothing here blocks.
  void update();
};

/// The one badge instance. Defined in Badge.cpp.
extern Badge badge;
