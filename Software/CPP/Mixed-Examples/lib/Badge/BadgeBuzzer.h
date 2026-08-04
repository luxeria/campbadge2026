/**
 * BadgeBuzzer.h - driver for the badge's passive buzzer.
 *
 * The SMD8530 is a *passive* buzzer: it contains no oscillator, so it only
 * makes a sound while you feed it a square wave. The ESP32 has no working
 * tone() implementation for this, so we use one LEDC (PWM) channel instead.
 *
 * Everything here is non-blocking. playTone() and playMelody() return
 * immediately and update() takes care of stopping notes on time, so the
 * display and buttons stay responsive while sound is playing:
 *
 *   badge.buzzer.playTone(NOTE_A4, 200);
 *   ...
 *   badge.buzzer.update();   // call every loop()
 *
 * If a melody does not sound the way you expected, build with
 * `build_flags = -DBADGE_BUZZER_DEBUG` and every note is printed to the serial
 * monitor with a millis() timestamp, so you can see what actually played and
 * for how long.
 */
#pragma once

#include <Arduino.h>

#include "BadgePins.h"

/// One entry of a melody. A frequency of 0 is a rest.
struct BadgeNote {
  uint16_t frequency;
  uint16_t durationMs;
};

// Note frequencies in Hz, equal temperament with A4 = 440 Hz, rounded to the
// nearest whole hertz. The buzzer is small and tinny, so notes below roughly
// 200 Hz are more felt than heard.
//
// Sharps only - a flat is the same pitch as the sharp below it, so E-flat is
// NOTE_DS4, B-flat is NOTE_AS4, and so on.
static const uint16_t NOTE_REST = 0;

static const uint16_t NOTE_C4 = 262;
static const uint16_t NOTE_CS4 = 277;
static const uint16_t NOTE_D4 = 294;
static const uint16_t NOTE_DS4 = 311;
static const uint16_t NOTE_E4 = 330;
static const uint16_t NOTE_F4 = 349;
static const uint16_t NOTE_FS4 = 370;
static const uint16_t NOTE_G4 = 392;
static const uint16_t NOTE_GS4 = 415;
static const uint16_t NOTE_A4 = 440;
static const uint16_t NOTE_AS4 = 466;
static const uint16_t NOTE_B4 = 494;

static const uint16_t NOTE_C5 = 523;
static const uint16_t NOTE_CS5 = 554;
static const uint16_t NOTE_D5 = 587;
static const uint16_t NOTE_DS5 = 622;
static const uint16_t NOTE_E5 = 659;
static const uint16_t NOTE_F5 = 698;
static const uint16_t NOTE_FS5 = 740;
static const uint16_t NOTE_G5 = 784;
static const uint16_t NOTE_GS5 = 831;
static const uint16_t NOTE_A5 = 880;
static const uint16_t NOTE_AS5 = 932;
static const uint16_t NOTE_B5 = 988;

static const uint16_t NOTE_C6 = 1047;
static const uint16_t NOTE_CS6 = 1109;
static const uint16_t NOTE_D6 = 1175;
static const uint16_t NOTE_DS6 = 1245;
static const uint16_t NOTE_E6 = 1319;
static const uint16_t NOTE_F6 = 1397;
static const uint16_t NOTE_FS6 = 1480;
static const uint16_t NOTE_G6 = 1568;
static const uint16_t NOTE_GS6 = 1661;
static const uint16_t NOTE_A6 = 1760;
static const uint16_t NOTE_AS6 = 1865;
static const uint16_t NOTE_B6 = 1976;

class BadgeBuzzer {
public:
  /**
   * Claims an LEDC channel and attaches it to the buzzer pin.
   *
   * @param pin          buzzer pin, defaults to the badge wiring
   * @param ledcChannel  LEDC channel to use, 0-15. Change it if something else
   *                     in your sketch already uses channel 0.
   */
  void begin(int8_t pin = BADGE_PIN_BUZZER, uint8_t ledcChannel = 0);

  /**
   * Starts a tone.
   *
   * @param frequency Hz, or NOTE_REST for silence
   * @param durationMs how long to play; 0 means "until stopTone() is called"
   */
  void playTone(uint16_t frequency, uint32_t durationMs = 0);

  /// Stops any tone or melody immediately and leaves the pin low.
  void stopTone();

  /// A short click, handy as button feedback.
  void beep(uint16_t frequency = NOTE_C6, uint32_t durationMs = 30);

  /**
   * Plays a sequence of notes in the background.
   *
   * The array is not copied, so it has to stay alive until the melody
   * finishes - a `static const` or global array is the easy way to do that.
   */
  void playMelody(const BadgeNote *notes, uint16_t count, bool loop = false);

  void stopMelody();

  /// True while a tone or melody is sounding.
  bool isPlaying() const { return _playing; }

  /// Advances the tone timer and the melody. Call this every loop().
  void update();

private:
  int8_t _pin = BADGE_PIN_BUZZER;
  uint8_t _channel = 0;
  bool _ready = false;
  bool _playing = false;

  uint32_t _toneUntil = 0; ///< millis() deadline, 0 = play indefinitely

  const BadgeNote *_melody = nullptr;
  uint16_t _melodyCount = 0;
  uint16_t _melodyIndex = 0;
  uint32_t _noteUntil = 0;
  bool _melodyLoop = false;
  bool _inGap = false;

  void startNote();
  void output(uint16_t frequency);
};
