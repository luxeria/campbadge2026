/**
 * 03 - Buttons
 *
 * Reads the eight buttons through the I/O expander and prints what it sees.
 *
 * All eight buttons live in a single register, so one I2C read gets you all
 * of them at once. BadgeButtons debounces that byte and turns it into press
 * and release events, which is what you normally want - reacting to
 * isPressed() directly would fire hundreds of times while a button is held.
 *
 * The raw register value is printed too. A bit reads 0 while its button is
 * pressed, so with nothing held down you should see 0xFF.
 *
 *   pio run -e ex_buttons -t upload -t monitor
 */
#include <Arduino.h>

#include <BadgeButtons.h>
#include <BadgeIOExpander.h>

BadgeIOExpander io;
BadgeButtons buttons;

uint8_t lastRaw = 0xFF;

void setup() {
  Serial.begin(115200);
  delay(500);

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - button demo");

  if (!io.begin()) {
    Serial.println("I/O expander not found - run the ex_i2c_scan example");
    return;
  }
  Serial.printf("I/O expander at 0x%02X\n", io.address());

  buttons.begin(io);
  Serial.println("Press the buttons. Nothing pressed reads as 0xFF.");
}

void loop() {
  if (!io.detected()) {
    delay(1000);
    return;
  }

  buttons.update();

  // Show the raw register whenever it changes, so you can check the wiring
  // and the polarity with your own eyes.
  if (buttons.rawPort() != lastRaw) {
    lastRaw = buttons.rawPort();
    Serial.printf("raw port 0 = 0x%02X\n", lastRaw);
  }

  for (uint8_t i = 0; i < BADGE_BUTTON_COUNT; i++) {
    BadgeButton button = (BadgeButton)i;
    if (buttons.wasPressed(button)) {
      Serial.printf("BTN%u pressed\n", i + 1);
    }
    if (buttons.wasReleased(button)) {
      Serial.printf("BTN%u released\n", i + 1);
    }
  }

  delay(5);
}
