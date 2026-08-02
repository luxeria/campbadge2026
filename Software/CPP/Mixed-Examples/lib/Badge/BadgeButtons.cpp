#include "BadgeButtons.h"

void BadgeButtons::begin(BadgeIOExpander &io) {
  _io = &io;
  _raw = _candidate = _stable = 0xFF;
  _changedAt = millis();
  _pressed = _pressedEdges = _releasedEdges = 0;
}

void BadgeButtons::update() {
  if (_io == nullptr) {
    return;
  }

  _raw = _io->readButtons();

  // Restart the timer whenever the reading changes; only once it has held
  // still for BADGE_DEBOUNCE_MS do we believe it.
  if (_raw != _candidate) {
    _candidate = _raw;
    _changedAt = millis();
  }

  const uint8_t previous = _pressed;

  if (_candidate != _stable && (millis() - _changedAt) >= BADGE_DEBOUNCE_MS) {
    _stable = _candidate;
  }

  _pressed = (uint8_t)~_stable; // buttons read low while pressed

  _pressedEdges = (uint8_t)(_pressed & ~previous);
  _releasedEdges = (uint8_t)(~_pressed & previous);
}

bool BadgeButtons::isPressed(BadgeButton button) const {
  return (_pressed & (1 << button)) != 0;
}

bool BadgeButtons::wasPressed(BadgeButton button) const {
  return (_pressedEdges & (1 << button)) != 0;
}

bool BadgeButtons::wasReleased(BadgeButton button) const {
  return (_releasedEdges & (1 << button)) != 0;
}
