/**
 * 00 - Pin probe
 *
 * Works out which pins the badge's I2C bus is wired to, by trying every
 * combination of the board's exposed pins and seeing which one gets an answer
 * from the I/O expander.
 *
 * You only need this if you are bringing up a board that is not already listed
 * in BadgePins.h, or if you wired the badge to something other than the
 * documented header positions. Once it prints a hit, put those numbers into
 * BadgePins.h and use the other examples.
 *
 * One subtlety worth knowing if you write something similar yourself:
 * Wire.end() does not undo the ESP32's GPIO matrix routing, so a pin used as
 * SCL in an earlier attempt keeps on clocking the bus. Without detaching the
 * pins by hand between attempts, every later combination looks like a hit.
 *
 *   pio run -e atom_pin_probe -t upload -t monitor
 */
#include <Arduino.h>
#include <Wire.h>
#include <esp32-hal-matrix.h>

#include <BadgePins.h>

#if defined(ARDUINO_M5Stack_ATOMS3)
// Pins broken out on an M5Stack AtomS3 Lite.
static const uint8_t PINS[] = {1, 2, 5, 6, 7, 8, 38, 39};
#else
// Pins broken out on an M5Stack Atom Lite.
static const uint8_t PINS[] = {19, 21, 22, 23, 25, 26, 32, 33};
#endif
static const uint8_t PIN_COUNT = sizeof(PINS) / sizeof(PINS[0]);

void releaseAllPins() {
  for (uint8_t i = 0; i < PIN_COUNT; i++) {
    pinMatrixOutDetach(PINS[i], false, false);
    pinMode(PINS[i], INPUT);
  }
}

void setup() {
  Serial.begin(115200);
  delay(1500);

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - I2C pin probe");
  Serial.printf("Trying %u pins in every SDA/SCL combination...\n", PIN_COUNT);

  uint8_t hits = 0;
  for (uint8_t a = 0; a < PIN_COUNT; a++) {
    for (uint8_t b = 0; b < PIN_COUNT; b++) {
      if (a == b) {
        continue;
      }
      const uint8_t sda = PINS[a];
      const uint8_t scl = PINS[b];

      releaseAllPins();
      delay(5);

      Wire.begin(sda, scl, 100000);
      for (uint8_t i = 0; i < BADGE_IO_ADDRESS_COUNT; i++) {
        Wire.beginTransmission(BADGE_IO_ADDRESSES[i]);
        if (Wire.endTransmission() == 0) {
          Serial.printf("  HIT  SDA = GPIO%u, SCL = GPIO%u, address = 0x%02X\n",
                        sda, scl, BADGE_IO_ADDRESSES[i]);
          hits++;
        }
      }
      Wire.end();
    }
  }

  releaseAllPins();

  if (hits == 0) {
    Serial.println("  nothing found - check the wiring and the badge's power");
  }
  Serial.printf("Done, %u hit(s).\n", hits);
}

void loop() { delay(10000); }
