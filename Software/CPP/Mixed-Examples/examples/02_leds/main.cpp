/**
 * 02 - LEDs
 *
 * Cycles through the badge's four LEDs, then runs a chase, then blinks all of
 * them together.
 *
 * The interesting part is what "off" means on this badge. The LEDs are fed
 * from 5V but the I/O expander can only pull its pins up to VCC (3.3V on an
 * AtomS3 Lite), so driving a pin high leaves the LED dimly lit. Configuring
 * the pin as an *input* instead lets it float and switches the LED off
 * properly. BadgeIOExpander::ledOff() does that for you - watch for it in
 * BadgeIOExpander.cpp.
 *
 * Two things that are not bugs: the POWER LED cannot be switched off at all
 * (desolder it if it bothers you), and on some badges LED_RED is actually a
 * second blue LED.
 *
 *   pio run -e ex_leds -t upload -t monitor
 */
#include <Arduino.h>

#include <BadgeIOExpander.h>

BadgeIOExpander io;

static const BadgeLed LEDS[] = {BADGE_LED_GREEN, BADGE_LED_BLUE, BADGE_LED_RED,
                                BADGE_LED_YELLOW};
static const char *LED_NAMES[] = {"green", "blue", "red", "yellow"};

void setup() {
  Serial.begin(115200);
  delay(500);

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - LED demo");

  if (!io.begin()) {
    Serial.println("I/O expander not found - run the ex_i2c_scan example");
    return;
  }
  Serial.printf("I/O expander at 0x%02X\n", io.address());
}

void loop() {
  if (!io.detected()) {
    delay(1000);
    return;
  }

  // One at a time.
  for (uint8_t i = 0; i < BADGE_LED_COUNT; i++) {
    Serial.printf("%s on\n", LED_NAMES[i]);
    io.ledOn(LEDS[i]);
    delay(400);
    io.ledOff(LEDS[i]);
  }

  // Chase, twice around.
  Serial.println("chase");
  for (uint8_t round = 0; round < 2; round++) {
    for (uint8_t i = 0; i < BADGE_LED_COUNT; i++) {
      io.ledOn(LEDS[i]);
      delay(80);
      io.ledOff(LEDS[i]);
    }
  }

  // All together. If any LED still glows faintly here, the pin is being
  // driven high somewhere instead of being switched to an input.
  Serial.println("all on / all off");
  for (uint8_t i = 0; i < 3; i++) {
    io.setAllLeds(true);
    delay(250);
    io.setAllLeds(false);
    delay(250);
  }

  delay(800);
}
