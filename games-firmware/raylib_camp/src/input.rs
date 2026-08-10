//! Button input tracking with pressed/released edge detection.
//!
//! This module is hardware agnostic: the firmware supplies a bitmask of which
//! buttons are currently held down together with a millisecond timestamp, and
//! [`Input`] derives the answers games care about (held, just pressed, just
//! released, hold duration). A game never talks to the I/O expander directly.

/// A single button on the badge, numbered 1 through 8.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    /// First button.
    Btn1,
    /// Second button.
    Btn2,
    /// Third button.
    Btn3,
    /// Fourth button.
    Btn4,
    /// Fifth button.
    Btn5,
    /// Sixth button.
    Btn6,
    /// Seventh button.
    Btn7,
    /// Eighth button.
    Btn8,
}

impl Button {
    /// Returns the bit index used in the raw button mask.
    pub fn bit(self) -> u8 {
        match self {
            Button::Btn1 => 0,
            Button::Btn2 => 1,
            Button::Btn3 => 2,
            Button::Btn4 => 3,
            Button::Btn5 => 4,
            Button::Btn6 => 5,
            Button::Btn7 => 6,
            Button::Btn8 => 7,
        }
    }
}

/// Number of buttons connected to the badge.
pub const BUTTON_COUNT: usize = 8;

/// Tracks the state of all buttons across frames.
pub struct Input {
    previous_mask: u8,
    current_mask: u8,
    pressed_at: [u32; BUTTON_COUNT],
}

impl Input {
    /// Creates an input tracker in which no button is pressed.
    pub const fn new() -> Self {
        Input {
            previous_mask: 0,
            current_mask: 0,
            pressed_at: [0; BUTTON_COUNT],
        }
    }

    /// Advances the tracker to the latest hardware state.
    ///
    /// `active_mask` has a bit set for every button currently held and
    /// `now_ms` is the current monotonic time in milliseconds.
    pub fn update(&mut self, active_mask: u8, now_ms: u32) {
        self.previous_mask = self.current_mask;
        self.current_mask = active_mask;
        let newly_pressed = self.current_mask & !self.previous_mask;
        for button in 0..BUTTON_COUNT {
            let bit = 1u8 << button;
            if newly_pressed & bit != 0 {
                self.pressed_at[button] = now_ms;
            }
        }
    }

    /// Returns whether a button is held down.
    pub fn pressed(&self, button: Button) -> bool {
        self.current_mask & (1 << button.bit()) != 0
    }

    /// Returns whether a button was pressed since the last update.
    pub fn just_pressed(&self, button: Button) -> bool {
        let bit = 1 << button.bit();
        self.current_mask & bit != 0 && self.previous_mask & bit == 0
    }

    /// Returns whether a button was released since the last update.
    pub fn just_released(&self, button: Button) -> bool {
        let bit = 1 << button.bit();
        self.current_mask & bit == 0 && self.previous_mask & bit != 0
    }

    /// Returns how many milliseconds a button has been held, or zero.
    pub fn hold_ms(&self, button: Button, now_ms: u32) -> u32 {
        if !self.pressed(button) {
            return 0;
        }
        now_ms.saturating_sub(self.pressed_at[button.bit() as usize])
    }

    /// Returns whether any button is held down.
    pub fn any_pressed(&self) -> bool {
        self.current_mask != 0
    }

    /// Returns whether any button was pressed since the last update.
    pub fn any_just_pressed(&self) -> bool {
        self.current_mask & !self.previous_mask != 0
    }
}

impl Default for Input {
    fn default() -> Self {
        Input::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, Input};

    fn mask(buttons: &[Button]) -> u8 {
        buttons
            .iter()
            .fold(0, |mask, button| mask | (1 << button.bit()))
    }

    #[test]
    fn edges_are_detected_on_transitions() {
        let mut input = Input::new();
        input.update(0, 0);
        input.update(mask(&[Button::Btn1]), 0);
        assert!(input.just_pressed(Button::Btn1));
        assert!(input.pressed(Button::Btn1));
        input.update(mask(&[Button::Btn1]), 10);
        assert!(!input.just_pressed(Button::Btn1));
        input.update(0, 20);
        assert!(input.just_released(Button::Btn1));
        assert!(!input.pressed(Button::Btn1));
    }

    #[test]
    fn hold_duration_tracks_press_time() {
        let mut input = Input::new();
        input.update(0, 0);
        input.update(mask(&[Button::Btn2]), 100);
        assert_eq!(input.hold_ms(Button::Btn2, 100), 0);
        assert_eq!(input.hold_ms(Button::Btn2, 260), 160);
        input.update(0, 300);
        assert_eq!(input.hold_ms(Button::Btn2, 300), 0);
    }

    #[test]
    fn simultaneous_presses_are_isolated() {
        let mut input = Input::new();
        input.update(mask(&[Button::Btn3]), 0);
        assert!(input.any_just_pressed());
        let just = [
            Button::Btn1,
            Button::Btn2,
            Button::Btn4,
            Button::Btn5,
            Button::Btn6,
            Button::Btn7,
            Button::Btn8,
        ]
        .iter()
        .all(|&button| !input.just_pressed(button));
        assert!(just);
        assert!(input.just_pressed(Button::Btn3));
    }
}
