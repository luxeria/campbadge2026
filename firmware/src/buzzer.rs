//! Driver for the badge's passive buzzer (SMD8530 on GPIO2).
//!
//! The SMD8530 is a passive transducer: it has no oscillator of its own, so it
//! only sounds while a square wave at the note's frequency is fed to it. On
//! the ESP32-S3 that signal is generated with the LEDC (PWM) peripheral, since
//! the chip has no audio DAC wired to this pin.
//!
//! Short sounds are described as `(frequency, duration_ms)` lists where a
//! frequency of zero is a rest, the same convention ChipSeq's Arduino
//! exporters use for badge melodies.

use esp_hal::delay::Delay;
use esp_hal::gpio::AnyPin;
use esp_hal::gpio::DriveMode;
use esp_hal::ledc::channel::{
    config::Config as ChannelConfig, ChannelIFace, Number as ChannelNumber,
};
use esp_hal::ledc::timer::{
    config::{Config as TimerConfig, Duty},
    LSClockSource, Number as TimerNumber, TimerIFace,
};
use esp_hal::ledc::{Ledc, LowSpeed};
use esp_hal::time::Rate;

/// The buzzer's GPIO number on the badge.
const BUZZER_PIN: u8 = 2;
/// Square-wave duty of a beep, as a percentage. Below 50% makes the passive
/// buzzer quieter since it only has a 50% "full" square wave available.
const BEEP_DUTY_PCT: u8 = 10;

/// Startup chirp: two short C6 beeps with a silence between them.
const STARTUP_NOTES: [(u32, u32); 3] = [(1047, 100), (0, 70), (1047, 100)];
/// Farkle jingle: three descending notes, the last held a little longer.
const FARKLE_NOTES: [(u32, u32); 3] = [(523, 135), (440, 135), (349, 350)];
/// Hot-dice arpeggio: three quick ascending notes, shorter than the held
/// melody before it.
const HOT_DICE_NOTES: [(u32, u32); 3] = [(349, 70), (440, 70), (523, 90)];
/// Win fanfare: a bright rising "ta-daaaah" with the final note held.
const WIN_NOTES: [(u32, u32); 2] = [(784, 150), (1047, 460)];
/// B6 roll blip: a single very short note so rolls stay a quiet click (used
/// for both a fresh throw and re-rolling the leftovers).
const ROLL_NOTES: [(u32, u32); 1] = [(523, 30)];
/// Bank milestone: a pleasant rising note when a large turn is banked.
const BIG_BANK_NOTES: [(u32, u32); 2] = [(659, 90), (1047, 150)];

/// Plays the startup chirp, also used when the game is reset by triple-BTN7.
pub fn play_startup(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &STARTUP_NOTES);
}

/// Plays the descending bust jingle when a player farkles.
pub fn play_farkle(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &FARKLE_NOTES);
}

/// Plays the ascending jingle when a player clears all five dice (hot dice).
pub fn play_hot_dice(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &HOT_DICE_NOTES);
}

/// Plays the rising fanfare when a player wins the game.
pub fn play_win(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &WIN_NOTES);
}

/// Plays the quick riffle that opens a fresh throw.
pub fn play_roll(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &ROLL_NOTES);
}

/// Plays the same quiet B6 blip when re-rolling the leftover dice.
pub fn play_reroll(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &ROLL_NOTES);
}

/// Plays the short note marking that `player` takes the table.
///
/// Player A keeps a lower pitch and player B a step up, echoing the seat
/// colours so the hot-seat rotation is audible without looking at the screen.
pub fn play_handoff(ledc: &Ledc<'_>, delay: &mut Delay, player: usize) {
    let notes: &[(u32, u32)] = if player == 0 {
        &[(523, 45), (659, 70)]
    } else {
        &[(659, 45), (784, 70)]
    };
    play_notes(ledc, delay, notes);
}

/// Plays the rising note that celebrates banking a large turn.
pub fn play_big_bank(ledc: &Ledc<'_>, delay: &mut Delay) {
    play_notes(ledc, delay, &BIG_BANK_NOTES);
}

/// Drives the buzzer through a short sequence of `(frequency, duration_ms)`.
///
/// Each sounding note re-acquires the buzzer pin by its number: the LEDC
/// channel consumes the GPIO value when it attaches, so the pin is re-stolen
/// once the previous note's channel has been dropped. A zero frequency is a
/// rest and just delays.
fn play_notes(ledc: &Ledc<'_>, delay: &mut Delay, notes: &[(u32, u32)]) {
    for &(frequency, duration_ms) in notes {
        if frequency == 0 {
            delay.delay_millis(duration_ms);
            continue;
        }
        // SAFETY: GPIO2 is the badge buzzer and is only ever used by this
        // module, so stealing it by number each note is sound; the previous
        // note's channel has already been dropped and its output disconnected.
        let buzzer_pin = unsafe { AnyPin::steal(BUZZER_PIN) };
        let mut timer = ledc.timer::<LowSpeed>(TimerNumber::Timer0);
        timer
            .configure(TimerConfig {
                duty: Duty::Duty8Bit,
                clock_source: LSClockSource::APBClk,
                frequency: Rate::from_hz(frequency),
            })
            .expect("configure buzzer timer");
        let mut channel = ledc.channel(ChannelNumber::Channel0, buzzer_pin);
        channel
            .configure(ChannelConfig {
                timer: &timer,
                duty_pct: BEEP_DUTY_PCT,
                drive_mode: DriveMode::PushPull,
            })
            .expect("configure buzzer channel");
        delay.delay_millis(duration_ms);
        channel.set_duty(0).expect("silence buzzer");
    }
}
