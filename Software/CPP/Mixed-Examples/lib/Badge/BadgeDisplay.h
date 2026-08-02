/**
 * BadgeDisplay.h - driver for the badge's round GC9A01A display.
 *
 * BadgeDisplay *is an* Adafruit_GC9A01A, so every Adafruit_GFX call you have
 * seen elsewhere works unchanged:
 *
 *   badge.display.fillScreen(GC9A01A_BLACK);
 *   badge.display.drawCircle(BADGE_TFT_CX, BADGE_TFT_CY, 100, GC9A01A_CYAN);
 *   badge.display.centerText("Hello", BADGE_TFT_CX, BADGE_TFT_CY);
 *
 * Two things are specific to this badge:
 *
 *  - The panel's reset line hangs off the I/O expander rather than off a GPIO,
 *    so begin() needs an initialised BadgeIOExpander to reset the panel.
 *  - The display shares no pins with the default SPI bus, so we set up our own
 *    SPI instance with the badge's pins before handing it to Adafruit_GFX.
 */
#pragma once

#include <Adafruit_GC9A01A.h>
#include <Arduino.h>
#include <SPI.h>

#include "BadgeIOExpander.h"
#include "BadgePins.h"

/// SPI bus used for the display, set up with the badge pins in begin().
extern SPIClass badgeTftSPI;

class BadgeDisplay : public Adafruit_GC9A01A {
public:
  BadgeDisplay();

  /**
   * Resets the panel through the I/O expander and initialises the display.
   *
   * @param io  an expander that has already been begun
   * @param spiFrequency SPI clock in Hz
   * @return false if the expander is not available (the panel cannot be reset
   *         without it)
   */
  bool begin(BadgeIOExpander &io, uint32_t spiFrequency = BADGE_TFT_SPI_HZ);

  /// Draws text centred on (cx, cy) rather than with its top-left corner
  /// there, which is what you almost always want on a round screen.
  void centerText(const char *text, int16_t cx, int16_t cy);

  /// Same, but also picks the colour and text size first.
  void centerText(const char *text, int16_t cx, int16_t cy, uint16_t color,
                  uint8_t size = 2);

  /// True once the display has been initialised.
  bool ready() const { return _ready; }

private:
  bool _ready = false;
};
