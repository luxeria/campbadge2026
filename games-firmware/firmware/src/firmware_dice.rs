//! The Farkle/dice game firmware, driven by the [`Game`] engine hot-seat logic.
//!
//! Two players pass the badge around; each throws, picks scoring dice, rolls
//! the leftovers and banks. This module hosts the `main_dice` entry point and
//! splits the formerly-monolithic loop into discrete steps: input handling per
//! phase, rendering, and scheduled result sounds.

use raylib_camp::canvas::Canvas;
use raylib_camp::color::Color;
use raylib_camp::input::Button;
use raylib_camp::rand::Prng;

use games::dice::{slso8, Game, DICE_COUNT};

use crate::board::Board;
use crate::{buzzer, expander};

/// Side length of a settled die in pixels.
const DIE: i32 = 38;
/// Extra pixels a selected (cursor) die grows past the base size.
const SELECTED_DIE_EXTRA: i32 = 8;
/// Extra pixels a plain idle die shrinks below the base size.
const IDLE_DIE_SHRINK: i32 = 8;
/// Horizontal spacing between dice slots.
const SLOT_SPACING: i32 = 46;
/// Softest step of the die face's bottom shading gradient.
const DIE_SHADE_LIGHT: Color = Color::rgb565(0xf7, 0xea, 0xd3);
/// Mid step of the die face's bottom shading gradient.
const DIE_SHADE_MID: Color = Color::rgb565(0xf0, 0xdf, 0xc4);
/// Deepest step of the die face's bottom shading gradient.
const DIE_SHADE_DARK: Color = Color::rgb565(0xe8, 0xd4, 0xb4);
/// Frames a farkle turn-over screen waits before handing itself over (~2 s).
const FARKLE_HOLD_FRAMES: u32 = 60;
/// How many pixels the selected die lifts above the row.
const DIE_LIFT: i32 = 14;
/// Scale of the button-hint lines at the bottom of the screen.
const HELP_SCALE: f32 = 1.5;
/// Extra empty pixels between glyphs in the button-hint lines.
const HELP_LETTER_SPACING: i32 = 2;
/// Extra empty pixels between digits in scores and other numbers.
const NUMBER_SPACING: i32 = 2;
/// A single scoring action netting more than this plays the milestone arpeggio.
const SCORE_ARPEGGIO_MIN: u32 = 1000;
/// Scale of the big turn-indicator letter under the score.
const ACTIVE_PLAYER_SCALE: i32 = 2;
/// Button that toggles the on-screen button-hint text.
const HELP_TOGGLE_BUTTON: Button = Button::Btn5;
/// Number of quick BTN7 presses that reset the whole game.
const RESET_PRESS_COUNT: u32 = 3;
/// How long a full BTN7 press sequence may take before it is discarded.
const RESET_PRESS_WINDOW_MS: u32 = 700;
/// LED marking the player on the left seat (the board's bottom-left LED).
const PLAYER_A_LED: expander::Led = expander::Led::Green;
/// LED marking the player on the right seat (the board's bottom-right LED).
const PLAYER_B_LED: expander::Led = expander::Led::Yellow;

/// Running phase of the local game loop.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Slots empty; waiting for the player to roll.
    AwaitRoll,
    /// Dice landed; the player selects, scores, rolls or banks.
    Select,
    /// The turn ended (banked or farkle); waiting to continue.
    TurnOver,
    /// The game is over; a player reached the winning score.
    Winner,
}

/// Per-throw state of the Farkle game and its local UI layer.
struct DiceGame {
    rng: Prng,
    game: Game,
    selector: usize,
    marked: [bool; DICE_COUNT],
    phase: Phase,
    bad_frames: u32,
    last_banker: usize,
    last_banked: u32,
    farkle_sound_pending: bool,
    win_sound_pending: bool,
    last_farkle: bool,
    turnover_frames: u32,
    lift: [i32; DICE_COUNT],
    reset_last_press_ms: u32,
    reset_press_count: u32,
    active_player: usize,
    scored_since_throw: bool,
    help_visible: bool,
}

impl DiceGame {
    fn new() -> Self {
        Self {
            rng: Prng::new(esp_hal::rng::Rng::new().random()),
            game: Game::new(),
            selector: 0,
            marked: [false; DICE_COUNT],
            phase: Phase::AwaitRoll,
            bad_frames: 0,
            last_banker: 0,
            last_banked: 0,
            farkle_sound_pending: false,
            win_sound_pending: false,
            last_farkle: false,
            turnover_frames: 0,
            lift: [0i32; DICE_COUNT],
            reset_last_press_ms: 0,
            reset_press_count: 0,
            active_player: 0,
            scored_since_throw: false,
            help_visible: false,
        }
    }

    /// Reverts all session state to a fresh game, without sound.
    fn reset_game(&mut self) {
        self.game = Game::new();
        self.selector = 0;
        self.marked = [false; DICE_COUNT];
        self.lift = [0i32; DICE_COUNT];
        self.turnover_frames = 0;
        self.last_farkle = false;
        self.active_player = 0;
        self.scored_since_throw = false;
        self.phase = Phase::AwaitRoll;
    }

    /// Watches for a BTN7 triple-press and restarts the whole game with a
    /// startup chirp when it happens, from any phase.
    fn handle_reset(&mut self, board: &mut Board) {
        if !board.input.just_pressed(Button::Btn7) {
            return;
        }
        self.reset_press_count =
            if board.now_ms.wrapping_sub(self.reset_last_press_ms) <= RESET_PRESS_WINDOW_MS {
                self.reset_press_count + 1
            } else {
                1
            };
        self.reset_last_press_ms = board.now_ms;
        if self.reset_press_count >= RESET_PRESS_COUNT {
            self.reset_press_count = 0;
            self.reset_game();
            buzzer::play_startup(&board.ledc, &mut board.delay);
            crate::board::log_line(&mut board.tx, "badge: game reset by BTN7 triple-press\n");
        }
    }

    /// Advances the game by one frame's worth of input, per current phase.
    fn handle_phase_input(&mut self, board: &mut Board) {
        let in_play = self.game.dice_count();
        match self.phase {
            Phase::AwaitRoll => self.handle_await_roll(board),
            Phase::Select if in_play > 0 => self.handle_select(board),
            Phase::Winner => self.handle_winner(board),
            _ => self.handle_turnover(board),
        }
    }

    /// Roll (first of the turn) and the hot-dice bank shortcut in AwaitRoll.
    fn handle_await_roll(&mut self, board: &mut Board) {
        if board.input.just_pressed(Button::Btn6) {
            self.perform_roll(board, false);
        }
        if self.game.turn_score() > 0 && board.input.just_pressed(Button::Btn8) {
            self.last_banker = self.game.current_player();
            self.last_banked = self.game.turn_score();
            self.last_farkle = false;
            self.game.bank();
            self.phase = Phase::TurnOver;
        }
    }

    /// Selection, marking, scoring, leftover re-roll and banking in Select.
    fn handle_select(&mut self, board: &mut Board) {
        let in_play = self.game.dice_count();
        if board.input.just_pressed(Button::Btn1) {
            self.selector = (self.selector + in_play - 1) % in_play;
        }
        if board.input.just_pressed(Button::Btn3) {
            self.selector = (self.selector + 1) % in_play;
        }
        if board.input.just_pressed(Button::Btn2) {
            self.marked[self.selector] = !self.marked[self.selector];
        }
        if board.input.just_pressed(Button::Btn4) {
            let mut chosen = [0usize; DICE_COUNT];
            let mut count = 0;
            for index in 0..in_play {
                if self.marked[index] && count < DICE_COUNT {
                    chosen[count] = index;
                    count += 1;
                }
            }
            if count == 0 && self.selector < in_play {
                chosen[count] = self.selector;
                count += 1;
            }
            match self.game.score_selected(&chosen[..count]) {
                Ok(gained) => {
                    self.selector = 0;
                    self.marked = [false; DICE_COUNT];
                    self.scored_since_throw = true;
                    if count == in_play {
                        buzzer::play_hot_dice(&board.ledc, &mut board.delay);
                    } else if gained > SCORE_ARPEGGIO_MIN {
                        buzzer::play_arpeggio(&board.ledc, &mut board.delay);
                    } else {
                        buzzer::play_dupdap(&board.ledc, &mut board.delay);
                    }
                    self.phase = if count == in_play {
                        Phase::AwaitRoll
                    } else {
                        Phase::Select
                    };
                }
                Err(_) => {
                    self.bad_frames = 9;
                    buzzer::play_invalid(&board.ledc, &mut board.delay);
                }
            }
        }
        if self.scored_since_throw && board.input.just_pressed(Button::Btn6) {
            self.perform_roll(board, true);
        }
        if board.input.just_pressed(Button::Btn8) {
            self.last_banker = self.game.current_player();
            self.last_banked = self.game.turn_score();
            self.last_farkle = false;
            self.game.bank();
            self.phase = Phase::TurnOver;
        }
    }

    /// Turn-over screen: hand off automatically after a farkle, or on B8.
    fn handle_turnover(&mut self, board: &mut Board) {
        self.turnover_frames += 1;
        if board.input.just_pressed(Button::Btn8)
            || (self.last_farkle && self.turnover_frames >= FARKLE_HOLD_FRAMES)
        {
            self.turnover_frames = 0;
            self.selector = 0;
            self.marked = [false; DICE_COUNT];
            self.active_player = self.game.current_player();
            if self.game.winner().is_some() {
                self.win_sound_pending = true;
                self.phase = Phase::Winner;
            } else {
                buzzer::play_handoff(&board.ledc, &mut board.delay, self.active_player);
                self.phase = Phase::AwaitRoll;
            }
        }
    }

    /// Winner screen: B8 starts a fresh game.
    fn handle_winner(&mut self, board: &mut Board) {
        if board.input.just_pressed(Button::Btn8) {
            self.reset_game();
        }
    }

    /// Throws the dice in play, animates the tumble and settles on the outcome.
    fn perform_roll(&mut self, board: &mut Board, reroll: bool) {
        if reroll {
            buzzer::play_reroll(&board.ledc, &mut board.delay);
        } else {
            buzzer::play_roll(&board.ledc, &mut board.delay);
        }
        let turn_player = self.game.current_player();
        let thrown = self.game.dice_count();
        self.scored_since_throw = false;
        let scorable = self.game.throw(&mut self.rng);
        let mut values = [0u8; DICE_COUNT];
        values[..thrown].copy_from_slice(&self.game.dice()[..thrown]);
        animate_roll(board, &mut self.rng, &values[..thrown]);
        self.marked = [false; DICE_COUNT];
        self.selector = first_scorable(self.game.dice()).unwrap_or(0);
        if scorable {
            self.phase = Phase::Select;
        } else {
            self.last_farkle = true;
            self.last_banker = turn_player;
            let rolled = self.game.dice();
            blink_farkle(board, &rolled[..thrown]);
            self.farkle_sound_pending = true;
            self.phase = Phase::TurnOver;
        }
    }

    /// Draws the whole game frame into the board's canvas.
    fn render(&mut self, board: &mut Board) {
        board.canvas.clear(slso8::NAVY);
        let current_player = self.game.current_player();

        board.set_led(PLAYER_A_LED, self.active_player == 0);
        board.set_led(PLAYER_B_LED, self.active_player == 1);

        board
            .canvas
            .draw_text("A", 8, 4, 1, player_color(0, current_player == 0));
        draw_number(
            &mut board.canvas,
            34,
            4,
            self.game.score(0),
            slso8::CREAM,
            1,
        );
        board
            .canvas
            .draw_text("B", 188, 4, 1, player_color(1, current_player == 1));
        draw_number(
            &mut board.canvas,
            214,
            4,
            self.game.score(1),
            slso8::CREAM,
            1,
        );
        let active_x = if current_player == 0 { 8 } else { 188 };
        let underline = if current_player == 0 {
            slso8::GREEN
        } else {
            slso8::ORANGE
        };
        board.canvas.rect(active_x, 14, 24, 2, underline);

        draw_centered(&mut board.canvas, 18, "TURN", slso8::PEACH);

        let playing: &str = if current_player == 0 { "A" } else { "B" };
        let playing_color = if current_player == 0 {
            slso8::GREEN
        } else {
            slso8::PEACH
        };
        let playing_width = board.canvas.measure_text(playing, ACTIVE_PLAYER_SCALE);
        board.canvas.draw_text(
            playing,
            120 - playing_width / 2,
            30,
            ACTIVE_PLAYER_SCALE,
            playing_color,
        );
        draw_number(
            &mut board.canvas,
            120,
            54,
            self.game.turn_score(),
            slso8::CREAM,
            2,
        );

        let dice = self.game.dice();
        let count = dice.len();
        match self.phase {
            Phase::Select => {
                const CENTER_Y: i32 = 120;
                for index in 0..count {
                    let target = if self.marked[index] { DIE_LIFT } else { 0 };
                    let mut lift = self.lift[index];
                    let diff = target - lift;
                    if diff != 0 {
                        lift += diff * 3 / 5;
                        if (target - lift).abs() < 2 {
                            lift = target;
                        }
                        self.lift[index] = lift;
                    }
                }
                for index in 0..count {
                    let x = die_slot(count, index);
                    let selected = index == self.selector;
                    let marked_die = self.marked[index];
                    let y = CENTER_Y - self.lift[index];
                    let size = if selected {
                        DIE + SELECTED_DIE_EXTRA
                    } else if marked_die {
                        DIE
                    } else {
                        DIE - IDLE_DIE_SHRINK
                    };
                    let outline = if marked_die {
                        slso8::ORANGE
                    } else {
                        slso8::BURNT
                    };
                    draw_die(&mut board.canvas, x, y, size, dice[index], outline);
                }
            }
            Phase::AwaitRoll => {
                for index in 0..self.game.dice_count() {
                    draw_slot(
                        &mut board.canvas,
                        die_slot(self.game.dice_count(), index),
                        120,
                        DIE,
                    );
                }
            }
            _ => {}
        }

        if self.bad_frames > 0 {
            draw_centered(&mut board.canvas, 178, "!! INVALID !!", slso8::BURNT);
        } else {
            match self.phase {
                Phase::AwaitRoll => {
                    if self.help_visible {
                        let hint = if self.game.turn_score() > 0 {
                            "ROLL B6 . BANK B8"
                        } else {
                            "ROLL B6"
                        };
                        draw_centered_scaled(
                            &mut board.canvas,
                            186,
                            hint,
                            HELP_SCALE,
                            HELP_LETTER_SPACING,
                            slso8::PEACH,
                        );
                    }
                }
                Phase::Select => {
                    if self.help_visible {
                        draw_centered_scaled(
                            &mut board.canvas,
                            168,
                            "MARK B2 . SCORE B4",
                            HELP_SCALE,
                            HELP_LETTER_SPACING,
                            slso8::PEACH,
                        );
                        draw_centered_scaled(
                            &mut board.canvas,
                            188,
                            "ROLL B6 . BANK B8",
                            HELP_SCALE,
                            HELP_LETTER_SPACING,
                            slso8::PEACH,
                        );
                    }
                }
                Phase::TurnOver => {
                    if self.last_farkle {
                        draw_centered_integer(
                            &mut board.canvas,
                            105,
                            "FARKLE!",
                            3,
                            NUMBER_SPACING,
                            slso8::BURNT,
                        );
                    } else {
                        draw_centered(&mut board.canvas, 97, "BANKED", slso8::PEACH);
                        draw_number(
                            &mut board.canvas,
                            120,
                            113,
                            self.last_banked,
                            slso8::CREAM,
                            3,
                        );
                    }
                    for player in 0..2 {
                        let color = player_color(player, player == self.last_banker);
                        draw_ledger(
                            &mut board.canvas,
                            120,
                            154 + player as i32 * 24,
                            player,
                            self.game.score(player),
                            2,
                            color,
                        );
                    }
                    if self.help_visible {
                        draw_centered_scaled(
                            &mut board.canvas,
                            212,
                            "NEXT B8",
                            HELP_SCALE,
                            HELP_LETTER_SPACING,
                            slso8::CREAM,
                        );
                    }
                }
                Phase::Winner => {
                    let winner = self.game.winner().unwrap_or(0);
                    draw_centered_integer(
                        &mut board.canvas,
                        100,
                        "WINNER!",
                        3,
                        NUMBER_SPACING,
                        slso8::ORANGE,
                    );
                    let color = player_color(winner, winner == self.last_banker);
                    draw_ledger(
                        &mut board.canvas,
                        120,
                        150,
                        winner,
                        self.game.score(winner),
                        2,
                        color,
                    );
                    if self.help_visible {
                        draw_centered_scaled(
                            &mut board.canvas,
                            200,
                            "NEW GAME B8",
                            HELP_SCALE,
                            HELP_LETTER_SPACING,
                            slso8::CREAM,
                        );
                    }
                }
            }
        }
    }
}

/// Plays the selected dice game until power is removed.
pub fn main_dice(mut board: Board) -> ! {
    let mut app = DiceGame::new();
    buzzer::play_startup(&board.ledc, &mut board.delay);
    loop {
        board.frame();

        if board.input.any_just_pressed() {
            for index in 0..8 {
                if board.input.just_pressed(button_at(index)) {
                    log_button_press(&mut board.tx, index);
                }
            }
        }

        if board.input.just_pressed(HELP_TOGGLE_BUTTON) {
            app.help_visible = !app.help_visible;
        }
        app.handle_reset(&mut board);
        app.handle_phase_input(&mut board);
        if app.bad_frames > 0 {
            app.bad_frames -= 1;
        }
        app.render(&mut board);
        board.flush();

        if app.farkle_sound_pending {
            buzzer::play_farkle(&board.ledc, &mut board.delay);
            app.farkle_sound_pending = false;
        }
        if app.win_sound_pending {
            buzzer::play_win(&board.ledc, &mut board.delay);
            app.win_sound_pending = false;
        }

        board.delay.delay_millis(33);
    }
}

/// Maps a zero-based button index to its [`Button`] variant.
fn button_at(index: usize) -> Button {
    match index {
        1 => Button::Btn2,
        2 => Button::Btn3,
        3 => Button::Btn4,
        4 => Button::Btn5,
        5 => Button::Btn6,
        6 => Button::Btn7,
        7 => Button::Btn8,
        _ => Button::Btn1,
    }
}

/// Logs a freshly pressed button by name and hex mask, e.g. `btn1 0x01`.
fn log_button_press(
    tx: &mut esp_hal::usb_serial_jtag::UsbSerialJtagTx<'_, esp_hal::Blocking>,
    index: usize,
) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buffer = *b"btn0 0x00\n";
    buffer[3] = b'0' + (index as u8 + 1);
    let bit = 1u8 << index;
    buffer[7] = HEX[(bit >> 4) as usize];
    buffer[8] = HEX[(bit & 0x0f) as usize];
    if let Ok(text) = core::str::from_utf8(&buffer[..10]) {
        crate::board::log_line(tx, text);
    }
}

/// Draws a decimal number centred around the given horizontal point, at a scale.
fn draw_number(canvas: &mut Canvas, x: i32, y: i32, value: u32, color: Color, scale: i32) {
    let mut buffer = [0u8; 8];
    let mut n = 0;
    if value == 0 {
        buffer[n] = b'0';
        n += 1;
    }
    let mut v = value;
    while v > 0 && n < buffer.len() {
        buffer[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    buffer[..n].reverse();
    if let Ok(text) = core::str::from_utf8(&buffer[..n]) {
        let text_width = canvas.measure_text_spaced(text, scale, NUMBER_SPACING);
        canvas.draw_text_spaced(text, x - text_width / 2, y, scale, NUMBER_SPACING, color);
    }
}

/// Centres `text` on the display horizontally at the given `scale` and spacing.
fn draw_centered_scaled(
    canvas: &mut Canvas,
    y: i32,
    text: &str,
    scale: f32,
    letter_spacing: i32,
    color: Color,
) {
    let text_width = canvas.measure_text_scaled(text, scale, letter_spacing);
    canvas.draw_text_scaled(text, 120 - text_width / 2, y, scale, letter_spacing, color);
}

/// Centred one-line helper label at the base size.
fn draw_centered(canvas: &mut Canvas, y: i32, text: &str, color: Color) {
    draw_centered_scaled(canvas, y, text, 1.0, 0, color);
}

/// Draws `text` centred horizontally at an integer `scale` with letter spacing.
fn draw_centered_integer(
    canvas: &mut Canvas,
    y: i32,
    text: &str,
    scale: i32,
    letter_spacing: i32,
    color: Color,
) {
    let text_width = canvas.measure_text_spaced(text, scale, letter_spacing);
    canvas.draw_text_spaced(text, 120 - text_width / 2, y, scale, letter_spacing, color);
}

/// Text colour marking a player's label.
///
/// Player A is drawn green to match her bottom-left LED, player B keeps the
/// warm orange theme; the highlighted player stays brighter.
fn player_color(player: usize, highlighted: bool) -> Color {
    if player == 0 {
        if highlighted {
            slso8::GREEN
        } else {
            slso8::GREEN_DIM
        }
    } else if highlighted {
        slso8::ORANGE
    } else {
        slso8::MAUVE
    }
}

/// Draws a centred `A  <score>` style ledger line for one player.
fn draw_ledger(
    canvas: &mut Canvas,
    cx: i32,
    y: i32,
    player: usize,
    score: u32,
    scale: i32,
    color: Color,
) {
    let mut buffer = [0u8; 12];
    buffer[0] = if player % 2 == 0 { b'A' } else { b'B' };
    buffer[1] = b' ';
    buffer[2] = b' ';
    let mut digits = [0u8; 9];
    let mut n = 0;
    if score == 0 {
        digits[n] = b'0';
        n += 1;
    }
    let mut v = score;
    while v > 0 && n < digits.len() {
        digits[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    digits[..n].reverse();
    buffer[3..3 + n].copy_from_slice(&digits[..n]);
    if let Ok(text) = core::str::from_utf8(&buffer[..3 + n]) {
        let text_width = canvas.measure_text_spaced(text, scale, NUMBER_SPACING);
        canvas.draw_text_spaced(text, cx - text_width / 2, y, scale, NUMBER_SPACING, color);
    }
}

/// Horizontal centre of the `i`-th die in a row of `count`.
fn die_slot(count: usize, index: usize) -> i32 {
    let span = (count as i32 - 1) * SLOT_SPACING;
    120 - span / 2 + index as i32 * SLOT_SPACING
}

/// Draws a single die face with its pip layout.
fn draw_die(canvas: &mut Canvas, cx: i32, cy: i32, size: i32, value: u8, outline: Color) {
    let half = size / 2;
    let x = cx - half;
    let y = cy - half;
    let corner = (size / 6).clamp(2, 14);

    rounded_rect_filled(
        canvas,
        x - 1,
        y - 1,
        size + 2,
        size + 2,
        corner + 1,
        outline,
    );
    rounded_rect_filled(canvas, x, y, size, size, corner, slso8::CREAM);

    const SHADE_STEP: i32 = 2;
    let shade_steps = [DIE_SHADE_LIGHT, DIE_SHADE_MID, DIE_SHADE_DARK];
    for (index, shade) in shade_steps.iter().enumerate() {
        let band_y = y + size - (shade_steps.len() - index) as i32 * SHADE_STEP;
        canvas.rect_filled(x + 2, band_y, size - 4, SHADE_STEP, *shade);
    }

    let step = size / 3;
    let pip = (size / 9).max(3);
    let dots: &[(i32, i32)] = match value {
        1 => &[(0, 0)],
        2 => &[(-step, -step), (step, step)],
        3 => &[(-step, -step), (0, 0), (step, step)],
        4 => &[(-step, -step), (step, -step), (-step, step), (step, step)],
        5 => &[
            (-step, -step),
            (step, -step),
            (0, 0),
            (-step, step),
            (step, step),
        ],
        _ => &[
            (-step, -step),
            (step, -step),
            (-step, 0),
            (step, 0),
            (-step, step),
            (step, step),
        ],
    };
    for &(dx, dy) in dots {
        canvas.circle_filled(cx + dx, cy + dy, pip, slso8::NAVY);
    }
}

/// Returns the index of the leftmost rolled 1 or 5, if any, so the selector
/// can start on a scorable die after a throw.
fn first_scorable(dice: &[u8]) -> Option<usize> {
    dice.iter().position(|&value| value == 1 || value == 5)
}

/// Fills a rectangle with rounded corners of the given radius.
fn rounded_rect_filled(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
    color: Color,
) {
    let radius = radius.clamp(0, width / 2).min(height / 2);
    canvas.rect_filled(x + radius, y, width - 2 * radius, height, color);
    canvas.rect_filled(x, y + radius, radius, height - 2 * radius, color);
    canvas.rect_filled(
        x + width - radius,
        y + radius,
        radius,
        height - 2 * radius,
        color,
    );
    canvas.circle_filled(x + radius, y + radius, radius, color);
    canvas.circle_filled(x + width - radius, y + radius, radius, color);
    canvas.circle_filled(x + radius, y + height - radius, radius, color);
    canvas.circle_filled(x + width - radius, y + height - radius, radius, color);
}

/// Draws an empty, rounded die slot with a question-mark prompt, shown before
/// a roll and after the turn has ended but before matching dice land again.
fn draw_slot(canvas: &mut Canvas, cx: i32, cy: i32, size: i32) {
    let half = size / 2;
    let x = cx - half;
    let y = cy - half;
    let corner = (size / 6).clamp(2, 14);
    rounded_rect_filled(canvas, x, y, size, size, corner, slso8::MAUVE);
    rounded_rect_filled(
        canvas,
        x + 1,
        y + 1,
        size - 2,
        size - 2,
        (corner - 1).max(1),
        slso8::NAVY,
    );
    let text = "?";
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(
        text,
        cx - text_width / 2,
        cy - half + (size - 10) / 2,
        1,
        slso8::MAUVE,
    );
}

/// Animates `final_values` tumbling chaotically before settling into their slots.
fn animate_roll(board: &mut Board, rng: &mut Prng, final_values: &[u8]) {
    const FRAMES: i32 = 13;
    const SETTLE_Y: i32 = 120;
    let count = final_values.len().min(DICE_COUNT);

    let mut px = [0i32; DICE_COUNT];
    let mut py = [0i32; DICE_COUNT];
    for index in 0..count {
        px[index] = 40 + rng.next_range(170) as i32;
        py[index] = 25 + rng.next_range(100) as i32;
    }

    for frame in 0..FRAMES {
        let progress = frame as f32 / (FRAMES - 1) as f32;
        board.canvas.clear(slso8::NAVY);

        for index in 0..count {
            if progress < 0.72 {
                let dx = (rng.next_range(48) as i32) - 24;
                let dy = (rng.next_range(40) as i32) - 20;
                px[index] = (px[index] + dx).clamp(24, 214);
                py[index] = (py[index] + dy).clamp(16, 150);
            } else {
                let slot_x = die_slot(count, index);
                px[index] = px[index] + ((slot_x - px[index]) * 3) / 5;
                py[index] = py[index] + ((SETTLE_Y - py[index]) * 3) / 5;
            }

            let shown = if progress < 0.9 {
                1 + rng.next_range(6) as u8
            } else {
                final_values[index]
            };
            let size = if progress < 0.5 { DIE + 8 } else { DIE };
            draw_die(
                &mut board.canvas,
                px[index],
                py[index],
                size,
                shown,
                slso8::BURNT,
            );
        }

        board.flush();
        board.delay.delay_millis(33);
    }
}

/// Blinks a farkled throw on screen a few times before the turn-over screen.
fn blink_farkle(board: &mut Board, dice: &[u8]) {
    const BLINKS: u32 = 3;
    const VISIBLE_MS: u32 = 220;
    const HIDDEN_MS: u32 = 220;
    let count = dice.len().min(DICE_COUNT);

    for _ in 0..BLINKS {
        for (index, &value) in dice[..count].iter().enumerate() {
            draw_die(
                &mut board.canvas,
                die_slot(count, index),
                120,
                DIE,
                value,
                slso8::BURNT,
            );
        }
        board.flush();
        board.delay.delay_millis(VISIBLE_MS);

        board.canvas.clear(slso8::NAVY);
        board.flush();
        board.delay.delay_millis(HIDDEN_MS);
    }
}
