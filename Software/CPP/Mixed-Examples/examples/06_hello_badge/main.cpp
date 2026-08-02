/**
 * 06 - Hello Badge
 *
 * Everything on the badge working together, and the default environment:
 *
 *   pio run -t upload -t monitor
 *
 * What it does:
 *   BTN1 - BTN4   switch between four screens, light the matching LED while
 *                 held, and click the buzzer
 *   BTN5          plays a short melody
 *   BTN6 - BTN8   show up on the "buttons" screen like the others
 *
 * The loop never blocks. badge.update() debounces the buttons and advances
 * the buzzer, and each screen only repaints the pixels that actually changed,
 * so the melody keeps playing smoothly while you press things.
 */
#include <Arduino.h>
#include <math.h>

#include <Badge.h>

// --- Screens ---------------------------------------------------------------

enum Screen : uint8_t {
  SCREEN_WELCOME = 0,
  SCREEN_BUTTONS,
  SCREEN_LEDS,
  SCREEN_UPTIME,
  SCREEN_COUNT,
};

static const char *SCREEN_NAMES[] = {"Welcome", "Buttons", "LEDs", "Uptime"};

Screen currentScreen = SCREEN_WELCOME;
bool needsRepaint = true;
bool gagActive = false; ///< the tune has taken the screen over

// --- Melody ----------------------------------------------------------------

// The tune lives in its own header. A melody is just an array of
// {frequency, duration} pairs called MELODY, and playMelody() takes any array
// you hand it. NOTE_REST is silence. See BadgeBuzzer.h for the note names.
#include "melody.h"
static const uint16_t MELODY_LENGTH = sizeof(MELODY) / sizeof(MELODY[0]);

// --- Helpers ---------------------------------------------------------------

static const uint16_t LED_COLOURS[] = {GC9A01A_GREEN, GC9A01A_BLUE,
                                       GC9A01A_RED, GC9A01A_YELLOW};
static const char *LED_NAMES[] = {"GREEN", "BLUE", "RED", "YELLOW"};

/// Position of button `index` on a ring around the middle of the screen.
void buttonDotPosition(uint8_t index, int16_t &x, int16_t &y) {
  // Start at the top and go clockwise.
  const float angle = -HALF_PI + (TWO_PI * index) / BADGE_BUTTON_COUNT;
  const int16_t ringRadius = BADGE_TFT_RADIUS - 35;
  x = BADGE_TFT_CX + (int16_t)(cosf(angle) * ringRadius);
  y = BADGE_TFT_CY + (int16_t)(sinf(angle) * ringRadius);
}

void drawHeader() {
  badge.display.fillScreen(GC9A01A_BLACK);
  badge.display.drawCircle(BADGE_TFT_CX, BADGE_TFT_CY, BADGE_TFT_RADIUS - 2,
                           GC9A01A_DARKGREY);
  badge.display.centerText(SCREEN_NAMES[currentScreen], BADGE_TFT_CX, 40,
                           GC9A01A_DARKCYAN, 2);
}

// --- Screen painting -------------------------------------------------------

void paintWelcome() {
  drawHeader();
  badge.display.centerText("LuxCamp", BADGE_TFT_CX, BADGE_TFT_CY - 22,
                           GC9A01A_WHITE, 3);
  badge.display.centerText("Badge 2026", BADGE_TFT_CX, BADGE_TFT_CY + 12,
                           GC9A01A_ORANGE, 2);
  badge.display.centerText("BTN1-4 change screen", BADGE_TFT_CX,
                           BADGE_TFT_CY + 55, GC9A01A_LIGHTGREY, 1);
  badge.display.centerText("BTN5 plays a tune", BADGE_TFT_CX,
                           BADGE_TFT_CY + 70, GC9A01A_LIGHTGREY, 1);
}

void paintButtons() {
  drawHeader();
  badge.display.centerText("hold a button", BADGE_TFT_CX, BADGE_TFT_CY,
                           GC9A01A_LIGHTGREY, 1);

  for (uint8_t i = 0; i < BADGE_BUTTON_COUNT; i++) {
    int16_t x, y;
    buttonDotPosition(i, x, y);
    badge.display.drawCircle(x, y, 14, GC9A01A_DARKGREY);

    char label[2] = {(char)('1' + i), '\0'};
    badge.display.centerText(label, x, y, GC9A01A_LIGHTGREY, 1);
  }
}

void paintLeds() {
  drawHeader();
  for (uint8_t i = 0; i < BADGE_LED_COUNT; i++) {
    const int16_t y = BADGE_TFT_CY - 30 + i * 26;
    badge.display.centerText(LED_NAMES[i], BADGE_TFT_CX + 10, y,
                             LED_COLOURS[i], 2);
  }
  badge.display.centerText("BTN1-4 light them", BADGE_TFT_CX,
                           BADGE_TFT_CY + 78, GC9A01A_LIGHTGREY, 1);
}

void paintUptime() {
  drawHeader();
  badge.display.centerText("running for", BADGE_TFT_CX, BADGE_TFT_CY - 30,
                           GC9A01A_LIGHTGREY, 1);
}

/// Takes over the screen while the tune plays. Not one of the four screens -
/// it paints itself once and the loop puts the previous screen back when the
/// melody ends.
void paintGag() {
  badge.display.fillScreen(GC9A01A_MAGENTA);
  badge.display.centerText("GOTCHA", BADGE_TFT_CX, BADGE_TFT_CY - 18,
                           GC9A01A_WHITE, 4);
  badge.display.centerText("you have been badged", BADGE_TFT_CX,
                           BADGE_TFT_CY + 25, GC9A01A_BLACK, 1);
}

void repaint() {
  switch (currentScreen) {
  case SCREEN_WELCOME:
    paintWelcome();
    break;
  case SCREEN_BUTTONS:
    paintButtons();
    break;
  case SCREEN_LEDS:
    paintLeds();
    break;
  case SCREEN_UPTIME:
    paintUptime();
    break;
  default:
    break;
  }
}

// --- Live parts of each screen --------------------------------------------

/// Fills or clears the eight dots as buttons go down and up. Only the dots
/// that changed are touched, which is what keeps this cheap enough to run
/// every loop.
void refreshButtonDots() {
  static uint8_t lastMask = 0;
  const uint8_t mask = badge.buttons.pressedMask();
  if (mask == lastMask) {
    return;
  }

  for (uint8_t i = 0; i < BADGE_BUTTON_COUNT; i++) {
    const bool wasSet = (lastMask & (1 << i)) != 0;
    const bool isSet = (mask & (1 << i)) != 0;
    if (wasSet == isSet) {
      continue;
    }

    int16_t x, y;
    buttonDotPosition(i, x, y);
    badge.display.fillCircle(x, y, 13,
                             isSet ? GC9A01A_GREENYELLOW : GC9A01A_BLACK);
    badge.display.drawCircle(x, y, 14, GC9A01A_DARKGREY);

    char label[2] = {(char)('1' + i), '\0'};
    badge.display.centerText(label, x, y,
                             isSet ? GC9A01A_BLACK : GC9A01A_LIGHTGREY, 1);
  }

  lastMask = mask;
}

void refreshUptime() {
  static uint32_t lastSeconds = 0xFFFFFFFF;
  const uint32_t seconds = millis() / 1000;
  if (seconds == lastSeconds) {
    return;
  }
  lastSeconds = seconds;

  char text[16];
  snprintf(text, sizeof(text), "%lu:%02lu", (unsigned long)(seconds / 60),
           (unsigned long)(seconds % 60));

  // Paint over the old value before drawing the new one.
  badge.display.fillRect(BADGE_TFT_CX - 80, BADGE_TFT_CY - 10, 160, 34,
                         GC9A01A_BLACK);
  badge.display.centerText(text, BADGE_TFT_CX, BADGE_TFT_CY + 7,
                           GC9A01A_WHITE, 3);
}

// --- Arduino ---------------------------------------------------------------

void setup() {
  Serial.begin(115200);
  delay(500);

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - hello badge");

  if (!badge.begin()) {
    Serial.println("I/O expander not found - run the ex_i2c_scan example");
    // Not fatal: the buzzer is on a plain GPIO and still works.
    badge.buzzer.playTone(NOTE_C4, 400);
    return;
  }

  Serial.printf("I/O expander at 0x%02X, display %s\n", badge.io.address(),
                badge.display.ready() ? "ready" : "FAILED");

  badge.buzzer.beep();
}

void loop() {
  badge.update();

  if (!badge.io.detected()) {
    badge.buzzer.update();
    delay(100);
    return;
  }

  // BTN1 - BTN4: pick a screen, light the matching LED, click.
  for (uint8_t i = 0; i < BADGE_LED_COUNT; i++) {
    const BadgeButton button = (BadgeButton)i;

    if (badge.buttons.wasPressed(button)) {
      badge.io.ledOn((BadgeLed)i);
      badge.buzzer.beep();

      currentScreen = (Screen)i;
      needsRepaint = true;
      Serial.printf("BTN%u -> %s\n", i + 1, SCREEN_NAMES[i]);
    }

    if (badge.buttons.wasReleased(button)) {
      badge.io.ledOff((BadgeLed)i);
    }
  }

  // BTN5: play the tune.
  if (badge.buttons.wasPressed(BADGE_BTN5)) {
    Serial.println("BTN5 -> melody");
    badge.buzzer.playMelody(MELODY, MELODY_LENGTH);
    if (badge.display.ready()) {
      paintGag();
      gagActive = true;
    }
  }

  if (!badge.display.ready()) {
    return;
  }

  // The gag holds the screen until the melody finishes, or until you press one
  // of the screen buttons to cut it short.
  if (gagActive) {
    if (badge.buzzer.isPlaying() && !needsRepaint) {
      return;
    }
    gagActive = false;
    needsRepaint = true;
  }

  if (needsRepaint) {
    needsRepaint = false;
    repaint();
  }

  switch (currentScreen) {
  case SCREEN_BUTTONS:
    refreshButtonDots();
    break;
  case SCREEN_UPTIME:
    refreshUptime();
    break;
  default:
    break;
  }
}
