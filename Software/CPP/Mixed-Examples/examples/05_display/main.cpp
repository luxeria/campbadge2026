/**
 * 05 - Display
 *
 * Brings up the round GC9A01A panel and draws on it: colour fills, circles,
 * centred text and a dot orbiting the middle of the screen.
 *
 * Two badge-specific details worth knowing:
 *
 *  - The panel's reset line is on the I/O expander, not on a GPIO, so the
 *    expander has to be initialised first. BadgeDisplay::begin() takes the
 *    expander as an argument for exactly that reason.
 *  - The screen is round. It is addressed as a 240x240 square, but the corners
 *    are hidden behind the bezel, so keep what matters within
 *    BADGE_TFT_RADIUS of (BADGE_TFT_CX, BADGE_TFT_CY).
 *
 * If the screen stays black: the TFT_CS and TFT_DC labels on the 5 pin header
 * are swapped on the PCB, so check your wiring against BadgePins.h first.
 *
 *   pio run -e ex_display -t upload -t monitor
 */
#include <Arduino.h>
#include <math.h>

#include <BadgeDisplay.h>
#include <BadgeIOExpander.h>

BadgeIOExpander io;
BadgeDisplay display;

void showColours() {
  static const uint16_t COLOURS[] = {GC9A01A_RED,  GC9A01A_GREEN,
                                     GC9A01A_BLUE, GC9A01A_YELLOW,
                                     GC9A01A_CYAN, GC9A01A_MAGENTA};
  static const char *NAMES[] = {"RED", "GREEN", "BLUE", "YELLOW", "CYAN",
                                "MAGENTA"};

  for (uint8_t i = 0; i < 6; i++) {
    display.fillScreen(COLOURS[i]);
    display.centerText(NAMES[i], BADGE_TFT_CX, BADGE_TFT_CY, GC9A01A_BLACK, 3);
    delay(500);
  }
}

void showCircles() {
  display.fillScreen(GC9A01A_BLACK);

  // Concentric rings, drawn from the outside in. The outermost one sits right
  // on the edge of the visible area.
  for (int16_t r = BADGE_TFT_RADIUS - 2; r > 20; r -= 12) {
    display.drawCircle(BADGE_TFT_CX, BADGE_TFT_CY, r, GC9A01A_DARKCYAN);
    delay(40);
  }

  display.centerText("LuxCamp", BADGE_TFT_CX, BADGE_TFT_CY - 20,
                     GC9A01A_WHITE, 3);
  display.centerText("Badge 2026", BADGE_TFT_CX, BADGE_TFT_CY + 20,
                     GC9A01A_ORANGE, 2);
  delay(2000);
}

void showOrbit(uint32_t durationMs) {
  display.fillScreen(GC9A01A_BLACK);
  display.drawCircle(BADGE_TFT_CX, BADGE_TFT_CY, BADGE_TFT_RADIUS - 2,
                     GC9A01A_DARKGREY);
  display.centerText("orbit", BADGE_TFT_CX, BADGE_TFT_CY, GC9A01A_WHITE, 2);

  const int16_t orbitRadius = BADGE_TFT_RADIUS - 20;
  const uint32_t until = millis() + durationMs;

  int16_t previousX = -1, previousY = -1;
  float angle = 0.0f;

  while (millis() < until) {
    const int16_t x = BADGE_TFT_CX + (int16_t)(cosf(angle) * orbitRadius);
    const int16_t y = BADGE_TFT_CY + (int16_t)(sinf(angle) * orbitRadius);

    // Erase where the dot was, then draw it in its new place. Redrawing only
    // what changed keeps this smooth; repainting the whole 240x240 screen
    // every frame over SPI would not be.
    if (previousX >= 0) {
      display.fillCircle(previousX, previousY, 6, GC9A01A_BLACK);
    }
    display.fillCircle(x, y, 6, GC9A01A_GREENYELLOW);

    previousX = x;
    previousY = y;
    angle += 0.08f;
    delay(16);
  }
}

void setup() {
  Serial.begin(115200);
  delay(500);

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - display demo");

  if (!io.begin()) {
    Serial.println("I/O expander not found - it drives the display's reset "
                   "line, so the panel cannot be initialised");
    return;
  }
  Serial.printf("I/O expander at 0x%02X\n", io.address());

  if (!display.begin(io)) {
    Serial.println("display init failed");
    return;
  }
  Serial.println("display ready");

  display.centerText("Hello Badge", BADGE_TFT_CX, BADGE_TFT_CY, GC9A01A_WHITE,
                     2);
  delay(1500);
}

void loop() {
  if (!display.ready()) {
    delay(1000);
    return;
  }

  showColours();
  showCircles();
  showOrbit(6000);
}
