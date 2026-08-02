#include "BadgeBuzzer.h"

/// Fraction of a note's slot that actually sounds. The rest is silence, which
/// is what makes two identical notes in a row audible as two notes.
static const uint32_t BADGE_NOTE_DUTY_PERCENT = 85;

void BadgeBuzzer::begin(int8_t pin, uint8_t ledcChannel) {
  _pin = pin;
  _channel = ledcChannel;

  // 8 bit resolution is plenty: ledcWriteTone() only ever uses a 50% duty
  // cycle, which is exactly the square wave the buzzer wants.
  ledcSetup(_channel, 1000, 8);
  ledcAttachPin((uint8_t)_pin, _channel);
  ledcWrite(_channel, 0); // silent, pin low

  _ready = true;
  _playing = false;
}

void BadgeBuzzer::output(uint16_t frequency) {
  if (!_ready) {
    return;
  }
  if (frequency == NOTE_REST) {
    ledcWrite(_channel, 0);
  } else {
    ledcWriteTone(_channel, frequency);
  }
}

void BadgeBuzzer::playTone(uint16_t frequency, uint32_t durationMs) {
  stopMelody();
  output(frequency);
  _toneUntil = (durationMs > 0) ? (millis() + durationMs) : 0;
  _playing = (frequency != NOTE_REST);
}

void BadgeBuzzer::stopTone() {
  stopMelody();
  output(NOTE_REST);
  _toneUntil = 0;
  _playing = false;
}

void BadgeBuzzer::beep(uint16_t frequency, uint32_t durationMs) {
  playTone(frequency, durationMs);
}

void BadgeBuzzer::playMelody(const BadgeNote *notes, uint16_t count, bool loop) {
  if (notes == nullptr || count == 0) {
    return;
  }
  _melody = notes;
  _melodyCount = count;
  _melodyIndex = 0;
  _melodyLoop = loop;
  _toneUntil = 0;
  startNote();
}

void BadgeBuzzer::stopMelody() {
  // Silence the pin as well as forgetting the melody: without this, stopping
  // part way through a note leaves that note sounding forever, because nothing
  // is left to switch it off.
  if (_melody != nullptr) {
    output(NOTE_REST);
    _playing = false;
  }

  _melody = nullptr;
  _melodyCount = 0;
  _melodyIndex = 0;
  _inGap = false;
  _noteUntil = 0;
}

void BadgeBuzzer::startNote() {
  const BadgeNote &note = _melody[_melodyIndex];
  const uint32_t sounding = (note.durationMs * BADGE_NOTE_DUTY_PERCENT) / 100;

#ifdef BADGE_BUZZER_DEBUG
  Serial.printf("[%lu] note %u/%u freq=%u dur=%u sounding=%lu\n",
                (unsigned long)millis(), _melodyIndex + 1, _melodyCount,
                note.frequency, note.durationMs, (unsigned long)sounding);
#endif

  output(note.frequency);
  _inGap = false;
  _noteUntil = millis() + (sounding > 0 ? sounding : 1);
  _playing = true;
}

void BadgeBuzzer::update() {
  if (!_ready) {
    return;
  }

  const uint32_t now = millis();

  // A plain tone with a duration.
  if (_toneUntil != 0 && (int32_t)(now - _toneUntil) >= 0) {
    output(NOTE_REST);
    _toneUntil = 0;
    _playing = false;
  }

  if (_melody == nullptr || (int32_t)(now - _noteUntil) < 0) {
    return;
  }

  if (!_inGap) {
    // The note has sounded long enough; stay quiet for the rest of its slot.
    const BadgeNote &note = _melody[_melodyIndex];
    const uint32_t sounding = (note.durationMs * BADGE_NOTE_DUTY_PERCENT) / 100;
    const uint32_t gap = note.durationMs - sounding;

    output(NOTE_REST);
    _inGap = true;
    _noteUntil = now + (gap > 0 ? gap : 1);
    return;
  }

  // Gap is over, move on.
  _melodyIndex++;
  if (_melodyIndex >= _melodyCount) {
    if (!_melodyLoop) {
      stopMelody();
      _playing = false;
      return;
    }
    _melodyIndex = 0;
  }
  startNote();
}
