#include "BadgeIOExpander.h"

bool BadgeIOExpander::begin(TwoWire &wire, uint8_t address) {
  _wire = &wire;
  _address = 0;
  _errors = 0;

  _wire->begin(BADGE_PIN_I2C_SDA, BADGE_PIN_I2C_SCL);
  _wire->setClock(400000);

  if (address != 0) {
    _wire->beginTransmission(address);
    if (_wire->endTransmission() == 0) {
      _address = address;
    }
  } else {
    // Which address the expander uses depends on how its address pins are
    // strapped, so rather than hard-coding a guess we probe the ranges both
    // compatible parts can occupy and take the first one that answers.
    for (uint8_t i = 0; i < BADGE_IO_ADDRESS_COUNT && _address == 0; i++) {
      _wire->beginTransmission(BADGE_IO_ADDRESSES[i]);
      if (_wire->endTransmission() == 0) {
        _address = BADGE_IO_ADDRESSES[i];
      }
    }
  }

  if (_address == 0) {
    return false;
  }

  // Buttons: all of port 0 as inputs.
  writeRegister(BADGE_IO_REG_CONFIG_0, 0xFF);

  // Port 1 starts with everything as an input, which means all four LEDs are
  // off and the header pins are left alone.
  _output1 = 0xFF;
  _config1 = 0xFF;
  writeRegister(BADGE_IO_REG_OUTPUT_1, _output1);
  writeRegister(BADGE_IO_REG_CONFIG_1, _config1);

  // The display reset line is the exception: drive it high so the panel is
  // held out of reset instead of floating.
  setPort1PinOutput(BADGE_IO_BIT_TFT_RST, true);

  return true;
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

uint8_t BadgeIOExpander::readButtons() {
  uint8_t value = 0xFF;
  if (!readRegister(BADGE_IO_REG_INPUT_0, value)) {
    return 0xFF; // read failed: report "nothing pressed"
  }
  return value;
}

bool BadgeIOExpander::isPressed(BadgeButton button) {
  return (readButtons() & (1 << button)) == 0;
}

// ---------------------------------------------------------------------------
// LEDs
// ---------------------------------------------------------------------------

void BadgeIOExpander::ledOn(BadgeLed led) {
  setPort1PinOutput(badgeLedBit(led), false);
}

void BadgeIOExpander::ledOff(BadgeLed led) {
  setPort1PinInput(badgeLedBit(led));
}

void BadgeIOExpander::setLed(BadgeLed led, bool on) {
  if (on) {
    ledOn(led);
  } else {
    ledOff(led);
  }
}

void BadgeIOExpander::setAllLeds(bool on) {
  for (uint8_t i = 0; i < BADGE_LED_COUNT; i++) {
    setLed((BadgeLed)i, on);
  }
}

// ---------------------------------------------------------------------------
// Display reset
// ---------------------------------------------------------------------------

void BadgeIOExpander::resetDisplay() {
  setPort1PinOutput(BADGE_IO_BIT_TFT_RST, true);
  delay(10);
  setPort1PinOutput(BADGE_IO_BIT_TFT_RST, false);
  delay(10);
  setPort1PinOutput(BADGE_IO_BIT_TFT_RST, true);
  delay(120); // the GC9A01A needs a moment before it accepts commands
}

// ---------------------------------------------------------------------------
// Port 1 helpers
// ---------------------------------------------------------------------------

void BadgeIOExpander::setPort1PinOutput(uint8_t bit, bool high) {
  const uint8_t mask = (uint8_t)(1 << bit);

  if (high) {
    _output1 |= mask;
  } else {
    _output1 &= (uint8_t)~mask;
  }

  // Latch first, direction second, so the pin cannot glitch to the previous
  // output value while it becomes an output.
  writeRegister(BADGE_IO_REG_OUTPUT_1, _output1);

  if (_config1 & mask) {
    _config1 &= (uint8_t)~mask; // clear = output
    writeRegister(BADGE_IO_REG_CONFIG_1, _config1);
  }
}

void BadgeIOExpander::setPort1PinInput(uint8_t bit) {
  const uint8_t mask = (uint8_t)(1 << bit);
  if (_config1 & mask) {
    return; // already an input
  }
  _config1 |= mask; // set = input
  writeRegister(BADGE_IO_REG_CONFIG_1, _config1);
}

// ---------------------------------------------------------------------------
// Raw register access
// ---------------------------------------------------------------------------

bool BadgeIOExpander::writeRegister(uint8_t reg, uint8_t value) {
  if (_address == 0) {
    return false;
  }
  _wire->beginTransmission(_address);
  _wire->write(reg);
  _wire->write(value);
  if (_wire->endTransmission() != 0) {
    _errors++;
    return false;
  }
  return true;
}

bool BadgeIOExpander::readRegister(uint8_t reg, uint8_t &value) {
  if (_address == 0) {
    return false;
  }
  _wire->beginTransmission(_address);
  _wire->write(reg);
  if (_wire->endTransmission(false) != 0) { // repeated start
    _errors++;
    return false;
  }
  if (_wire->requestFrom((int)_address, 1) != 1) {
    _errors++;
    return false;
  }
  value = (uint8_t)_wire->read();
  return true;
}
