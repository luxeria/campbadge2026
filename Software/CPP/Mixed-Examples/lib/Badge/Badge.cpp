#include "Badge.h"

Badge badge;

bool Badge::begin(bool withDisplay) {
  // The buzzer hangs off a plain GPIO, so it works even if the I2C bus does
  // not. Set it up first so a failed badge can at least beep at you.
  buzzer.begin();

  if (!io.begin()) {
    return false;
  }

  buttons.begin(io);

  if (withDisplay) {
    display.begin(io);
  }

  return true;
}

void Badge::update() {
  buttons.update();
  buzzer.update();
}
