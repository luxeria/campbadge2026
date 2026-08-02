/**
 * 01 - I2C scan
 *
 * The first thing to run on a badge you have just wired up. It scans the I2C
 * bus and prints every device that answers, then tells you which address the
 * I/O expander was found at.
 *
 * Expected output: exactly one device, somewhere in 0x74-0x77 (TCA9539) or
 * 0x20-0x27 (TCA9535). If the scan comes up empty, check SDA/SCL and the
 * badge's power before trying any of the other examples - the buttons, the
 * LEDs and the display's reset line all hang off this one chip.
 *
 *   pio run -e ex_i2c_scan -t upload -t monitor
 */
#include <Arduino.h>
#include <Wire.h>

#include <BadgeIOExpander.h>
#include <BadgePins.h>

BadgeIOExpander io;

void scanBus() {
  Serial.println();
  Serial.println("Scanning I2C bus...");

  uint8_t found = 0;
  for (uint8_t address = 1; address < 127; address++) {
    Wire.beginTransmission(address);
    if (Wire.endTransmission() == 0) {
      Serial.printf("  device at 0x%02X\n", address);
      found++;
    }
  }

  if (found == 0) {
    Serial.println("  nothing found - check SDA/SCL wiring and badge power");
  } else {
    Serial.printf("  %u device(s) found\n", found);
  }
}

void setup() {
  Serial.begin(115200);
  delay(500); // give the USB serial port a moment to come up

  Serial.println();
  Serial.println("LuxCamp Badge 2026 - I2C scan");
  Serial.printf("SDA = GPIO%d, SCL = GPIO%d\n", BADGE_PIN_I2C_SDA,
                BADGE_PIN_I2C_SCL);

  Wire.begin(BADGE_PIN_I2C_SDA, BADGE_PIN_I2C_SCL);
  Wire.setClock(400000);

  scanBus();

  // The driver does the same probe internally and remembers the address it
  // found, so you never have to hard-code it in your own sketches.
  Serial.println();
  if (io.begin()) {
    Serial.printf("I/O expander detected at 0x%02X\n", io.address());

    uint8_t buttons = io.readButtons();
    Serial.printf("Buttons right now: 0x%02X (a 0 bit means pressed)\n",
                  buttons);
  } else {
    Serial.println("I/O expander NOT detected");
  }
}

void loop() {
  delay(5000);
  scanBus();
}
