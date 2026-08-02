#include "BadgeDisplay.h"

// HSPI rather than FSPI: on the ESP32-S3 the SPI signals are routed through
// the GPIO matrix anyway, and HSPI has no default MISO pin it would try to
// claim - which suits us, because the badge's display has no MISO line.
SPIClass badgeTftSPI(HSPI);

BadgeDisplay::BadgeDisplay()
    : Adafruit_GC9A01A(&badgeTftSPI, BADGE_PIN_TFT_DC, BADGE_PIN_TFT_CS,
                       /* rst */ -1) {
  // rst is -1 on purpose: the reset line is on the I/O expander, so the
  // Adafruit driver cannot toggle it and begin() does it for us instead.
}

bool BadgeDisplay::begin(BadgeIOExpander &io, uint32_t spiFrequency) {
  if (!io.detected()) {
    return false;
  }

  // Set up the SPI bus with the badge's pins *before* Adafruit_GFX gets to it.
  // Adafruit_SPITFT::initSPI() calls badgeTftSPI.begin() with no arguments,
  // and the ESP32 implementation returns immediately if the bus is already
  // running - so whichever pins we install here are the ones that stick.
  badgeTftSPI.begin(BADGE_PIN_TFT_SCL, BADGE_PIN_TFT_MISO, BADGE_PIN_TFT_SDA,
                    BADGE_PIN_TFT_CS);

  io.resetDisplay();

  Adafruit_GC9A01A::begin(spiFrequency);
  setRotation(0);
  fillScreen(GC9A01A_BLACK);

  _ready = true;
  return true;
}

void BadgeDisplay::centerText(const char *text, int16_t cx, int16_t cy) {
  int16_t x1, y1;
  uint16_t w, h;
  getTextBounds(text, 0, 0, &x1, &y1, &w, &h);
  setCursor(cx - (int16_t)(w / 2) - x1, cy - (int16_t)(h / 2) - y1);
  print(text);
}

void BadgeDisplay::centerText(const char *text, int16_t cx, int16_t cy,
                              uint16_t color, uint8_t size) {
  setTextSize(size);
  setTextColor(color);
  centerText(text, cx, cy);
}
