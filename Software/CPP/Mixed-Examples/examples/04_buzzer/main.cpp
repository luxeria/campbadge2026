/**
 * 04 - Buzzer
 *
 * Plays a beep, a frequency sweep and a short melody on the badge's passive
 * buzzer. This is the one example that needs no I2C at all - the buzzer is
 * wired straight to a GPIO.
 *
 * "Passive" means the buzzer has no oscillator of its own and only makes a
 * sound while you feed it a square wave. On the ESP32 that is a job for the
 * LEDC (PWM) peripheral; see BadgeBuzzer.cpp.
 *
 * Note that nothing here blocks: playMelody() returns immediately and the
 * melody advances inside buzzer.update(), which is why loop() can keep doing
 * other things (here: printing) while the tune plays.
 *
 *   pio run -e ex_buzzer -t upload -t monitor
 */
#include <Arduino.h>

#include <BadgeBuzzer.h>

BadgeBuzzer buzzer;

// The array has to outlive the call to playMelody(), because the buzzer only
// keeps a pointer to it. `static const` at file scope is the simple way.
static const BadgeNote MELODY[] = {
    {NOTE_C5, 150}, {NOTE_E5, 150},  {NOTE_G5, 150}, {NOTE_C6, 300},
    {NOTE_REST, 80}, {NOTE_G5, 150}, {NOTE_C6, 450},
};
static const uint16_t MELODY_LENGTH = sizeof(MELODY) / sizeof(MELODY[0]);

void setup() {
  Serial.begin(115200);
  delay(500);

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - buzzer demo");
  Serial.printf("Buzzer on GPIO%d\n", BADGE_PIN_BUZZER);

  buzzer.begin();
}

void loop() {
  Serial.println("beep");
  buzzer.beep();
  // beep() is non-blocking, so we have to keep calling update() for the
  // buzzer to stop on time.
  uint32_t until = millis() + 500;
  while (millis() < until) {
    buzzer.update();
  }

  Serial.println("sweep");
  for (uint16_t frequency = 200; frequency <= 2000; frequency += 20) {
    buzzer.playTone(frequency);
    delay(8);
  }
  buzzer.stopTone();
  delay(300);

  Serial.println("melody");
  buzzer.playMelody(MELODY, MELODY_LENGTH);
  while (buzzer.isPlaying()) {
    buzzer.update();
    // There is plenty of time left over here for anything else your sketch
    // needs to do.
  }

  delay(1500);
}
