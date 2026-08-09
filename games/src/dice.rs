//! A two-player push-your-luck dice game (a "Farkle"/"10,000" style variant).
//!
//! The player rolls up to five dice, scores a legal subset of them (which are
//! removed), and either banks the accumulated turn score or rolls the leftovers
//! again. A throw with no scorable dice is a farkle and discards the turn. Pure
//! rules logic - `no_std`, allocation free - shared by the badge and any server.

use raylib_camp::rand::Prng;

/// Number of players.
pub const PLAYERS: usize = 2;
/// Dice rolled at the start of a turn (and again on "hot dice").
pub const DICE_COUNT: usize = 5;
/// The score a player must reach to win.
pub const WIN_SCORE: u32 = 4000;

/// A tally of how many of each die face value (1..=6) are present.
type Counts = [u8; 7];

/// Computes the points for a single multiset of die values, choosing the best
/// combination of streets, n-of-a-kind and single ones/fives.
fn score(counts: &Counts) -> u32 {
    let mut best = singles(counts);
    let total: u8 = counts.iter().sum();

    // Full street: any five consecutive faces.
    if total >= 5 {
        for value in 1..=2 {
            if (value..value + 5).all(|v| counts[v] >= 1) {
                best = best.max(1000);
            }
        }
    }

    // Small street: any four consecutive faces, leftover dice scored as singles.
    if total >= 4 {
        for value in 1..=3 {
            if (value..value + 4).all(|v| counts[v] >= 1) {
                let mut leftover = *counts;
                for cell in leftover[value..value + 4].iter_mut() {
                    *cell -= 1;
                }
                best = best.max(500 + singles(&leftover));
            }
        }
    }

    // N of a kind (three or more equal faces).
    for value in 1..=6 {
        let matched = counts[value] as usize;
        if matched >= 3 {
            let mut leftover = *counts;
            leftover[value] -= matched as u8;
            let group = if value == 1 {
                match matched {
                    3 => 1000,
                    4 => 2000,
                    _ => 4000,
                }
            } else {
                (value as u32) * 100 * (matched - 2) as u32
            };
            best = best.max(group + singles(&leftover));
        }
    }

    best
}

/// Points from single ones (100) and fives (50).
fn singles(counts: &Counts) -> u32 {
    100 * counts[1] as u32 + 50 * counts[5] as u32
}

/// Scores the given dice, returning how many points they are worth.
pub fn dice_score(dice: &[u8]) -> u32 {
    let mut counts = [0u8; 7];
    for &value in dice {
        counts[value as usize] += 1;
    }
    score(&counts)
}

/// Why a dice selection was rejected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectError {
    /// No dice were selected.
    Empty,
    /// A selected index is outside the dice currently in play.
    OutOfRange,
    /// The selected dice do not form a scoring combination.
    NoScore,
}

/// The state of a running two-player game.
pub struct Game {
    scores: [u32; PLAYERS],
    current: usize,
    turn_score: u32,
    dice: [u8; DICE_COUNT],
    dice_count: usize,
    winner: Option<usize>,
}

impl Game {
    /// Creates a fresh game with no dice rolled yet.
    pub fn new() -> Self {
        Game {
            scores: [0; PLAYERS],
            current: 0,
            turn_score: 0,
            dice: [0; DICE_COUNT],
            dice_count: DICE_COUNT,
            winner: None,
        }
    }

    /// Index of the player whose turn it is.
    pub fn current_player(&self) -> usize {
        self.current
    }

    /// A player's banked score.
    pub fn score(&self, player: usize) -> u32 {
        self.scores[player]
    }

    /// The score accumulated so far this turn (not yet banked).
    pub fn turn_score(&self) -> u32 {
        self.turn_score
    }

    /// The winner, once someone has reached [`WIN_SCORE`].
    pub fn winner(&self) -> Option<usize> {
        self.winner
    }

    /// The values of the dice currently in play.
    pub fn dice(&self) -> &[u8] {
        &self.dice[..self.dice_count]
    }

    /// How many dice are currently in play.
    pub fn dice_count(&self) -> usize {
        self.dice_count
    }

    /// Rolls the dice currently in play.
    ///
    /// Returns `true` if the throw contains something scorable (play continues),
    /// or `false` if it is a farkle (the turn score is discarded and the turn
    /// passes to the next player).
    pub fn throw(&mut self, rng: &mut Prng) -> bool {
        for die in self.dice[..self.dice_count].iter_mut() {
            *die = 1 + rng.next_range(6) as u8;
        }
        for die in self.dice[self.dice_count..].iter_mut() {
            *die = 0;
        }
        if dice_score(&self.dice[..self.dice_count]) == 0 {
            self.turn_score = 0;
            self.dice_count = DICE_COUNT; // fresh five for the next player
            self.advance_turn();
            false
        } else {
            true
        }
    }

    /// Scores the selected dice (indexes into [`Game::dice`]).
    ///
    /// On success the gained points are added to the turn score, the dice are
    /// removed from play, and - if none remain - the turn resets to a fresh five
    /// dice (hot dice). Returns the points gained, or an error if the selection
    /// is empty, out of range or not a legal scoring combination.
    pub fn score_selected(&mut self, selected: &[usize]) -> Result<u32, SelectError> {
        if self.winner.is_some() || selected.is_empty() {
            return Err(SelectError::Empty);
        }
        let mut values = [0u8; DICE_COUNT];
        let mut count = 0;
        for &index in selected {
            if index >= self.dice_count {
                return Err(SelectError::OutOfRange);
            }
            values[count] = self.dice[index];
            count += 1;
        }
        let gained = dice_score(&values[..count]);
        if gained == 0 {
            return Err(SelectError::NoScore);
        }
        // Reject selections that silently carry a worthless die (e.g. a 3
        // tucked next to a 1 and a 5): every chosen die must actually add
        // points, otherwise that dead die would leave play for free.
        for drop in 0..count {
            let mut reduced = [0u8; DICE_COUNT];
            let mut reduced_count = 0;
            for (index, &value) in values[..count].iter().enumerate() {
                if index != drop {
                    reduced[reduced_count] = value;
                    reduced_count += 1;
                }
            }
            if dice_score(&reduced[..reduced_count]) == gained {
                return Err(SelectError::NoScore);
            }
        }
        self.turn_score += gained;

        // Remove the chosen dice and compact the remainder.
        let mut removed = [false; DICE_COUNT];
        for &index in selected {
            removed[index] = true;
        }
        let mut keep = [0u8; DICE_COUNT];
        let mut keep_count = 0;
        for (index, &value) in self.dice[..self.dice_count].iter().enumerate() {
            if !removed[index] {
                keep[keep_count] = value;
                keep_count += 1;
            }
        }
        self.dice_count = keep_count;
        for (index, value) in keep[..keep_count].iter().enumerate() {
            self.dice[index] = *value;
        }
        if self.dice_count == 0 {
            self.dice_count = DICE_COUNT; // hot dice: back to a full five
        }
        Ok(gained)
    }

    /// Banks the current turn score, ending the turn.
    ///
    /// Returns the winner, if a player has reached [`WIN_SCORE`].
    pub fn bank(&mut self) -> Option<usize> {
        self.scores[self.current] += self.turn_score;
        if self.scores[self.current] >= WIN_SCORE {
            self.winner = Some(self.current);
            return self.winner;
        }
        self.turn_score = 0;
        self.dice_count = DICE_COUNT;
        self.advance_turn();
        None
    }

    fn advance_turn(&mut self) {
        self.current = (self.current + 1) % PLAYERS;
    }
}

impl Default for Game {
    fn default() -> Self {
        Game::new()
    }
}

/// The SLSO8 palette, from the game's visual theme.
pub mod slso8 {
    use raylib_camp::color::Color;

    /// Darkest navy.
    pub const NAVY: Color = Color::rgb565(0x0d, 0x2b, 0x45);
    /// Blue.
    pub const BLUE: Color = Color::rgb565(0x20, 0x3c, 0x56);
    /// Purple.
    pub const PURPLE: Color = Color::rgb565(0x54, 0x4e, 0x68);
    /// Mauve.
    pub const MAUVE: Color = Color::rgb565(0x8d, 0x69, 0x7a);
    /// Bright leaf green, the identity colour of player A (her LED).
    pub const GREEN: Color = Color::rgb565(0x6a, 0xc0, 0x3a);
    /// Muted olive green for player A's label when it is not her turn.
    pub const GREEN_DIM: Color = Color::rgb565(0x3a, 0x6b, 0x2c);
    /// Burnt orange.
    pub const BURNT: Color = Color::rgb565(0xd0, 0x81, 0x59);
    /// Orange.
    pub const ORANGE: Color = Color::rgb565(0xff, 0xaa, 0x5e);
    /// Peach.
    pub const PEACH: Color = Color::rgb565(0xff, 0xd4, 0xa3);
    /// Pale cream.
    pub const CREAM: Color = Color::rgb565(0xff, 0xec, 0xd6);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_ones_and_fives() {
        assert_eq!(dice_score(&[1]), 100);
        assert_eq!(dice_score(&[5]), 50);
        assert_eq!(dice_score(&[1, 5]), 150);
        assert_eq!(dice_score(&[2]), 0);
    }

    #[test]
    fn n_of_a_kind_scoring() {
        assert_eq!(dice_score(&[2, 2, 2]), 200);
        assert_eq!(dice_score(&[2, 2, 2, 2]), 400); // 4 twos: 2*100*2
        assert_eq!(dice_score(&[4, 4, 4, 4]), 800); // 4 fours: 4*100*2
        assert_eq!(dice_score(&[6, 6, 6, 6, 6]), 1800);
        assert_eq!(dice_score(&[1, 1, 1]), 1000);
        assert_eq!(dice_score(&[1, 1, 1, 1]), 2000);
        assert_eq!(dice_score(&[1, 1, 1, 1, 1]), 4000);
    }

    #[test]
    fn n_of_a_kind_with_leftovers() {
        assert_eq!(dice_score(&[3, 3, 3, 1, 5]), 450); // 300 + 100 + 50
    }

    #[test]
    fn streets_score() {
        assert_eq!(dice_score(&[1, 2, 3, 4, 5]), 1000);
        assert_eq!(dice_score(&[2, 3, 4, 5, 6]), 1000);
        assert_eq!(dice_score(&[1, 2, 3, 4, 6]), 500); // small street + lone 6
        assert_eq!(dice_score(&[1, 2, 3, 4]), 500); // four dice small street
    }

    #[test]
    fn a_throw_with_no_score_is_a_farkle() {
        let game = Game::new();
        assert_eq!(dice_score(&[2, 3, 4, 6, 6]), 0);
        assert_eq!(game.dice_count(), DICE_COUNT);
    }

    #[test]
    fn scoring_adds_to_turn_and_takes_dice() {
        let mut game = Game::new();
        // Force a specific throw: [1, 1, 1, 5, 2].
        game.dice = [1, 1, 1, 5, 2];
        game.dice_count = 5;
        let gained = game.score_selected(&[0, 1, 2, 3]).unwrap();
        assert_eq!(gained, 1050); // three ones (1000) + one five (50)
        assert_eq!(game.turn_score(), 1050);
        assert_eq!(game.dice_count(), 1); // only the 2 remains
    }

    #[test]
    fn farkle_resets_dice_count_for_next_player() {
        let mut game = Game::new();
        game.dice = [1, 1, 1, 5, 2];
        game.dice_count = 5;
        game.score_selected(&[0, 1, 2, 3]).unwrap();
        assert_eq!(game.dice_count(), 1);

        // A single leftover die that rolls a non-scorable face farkles.
        let mut seed = 1u32;
        loop {
            let mut probe = Prng::new(seed);
            let value = 1 + probe.next_range(6);
            if value != 1 && value != 5 {
                break;
            }
            seed += 1;
        }
        assert!(!game.throw(&mut Prng::new(seed)));

        // The next player starts from a full five dice, not the leftover one.
        assert_eq!(game.dice_count(), DICE_COUNT);
    }

    #[test]
    fn a_selection_cannot_carry_a_dead_die() {
        let mut game = Game::new();
        game.dice = [1, 3, 5, 2, 6];
        game.dice_count = 5;
        // A worthless 3 cannot be scored just because it sits next to a 1
        // and a 5: it would leave play without adding any points.
        assert_eq!(game.score_selected(&[0, 1, 2]), Err(SelectError::NoScore));
        // The same dice score fine without the dead 3.
        assert_eq!(game.score_selected(&[0, 2]).unwrap(), 150);
    }

    #[test]
    fn hot_dice_reset_to_five() {
        let mut game = Game::new();
        // Three ones (1000) + two fives (100): every die scores, so all five
        // leave play and the turn resets to a fresh five (hot dice).
        game.dice = [1, 1, 1, 5, 5];
        game.dice_count = 5;
        game.score_selected(&[0, 1, 2, 3, 4]).unwrap();
        assert_eq!(game.dice_count(), DICE_COUNT);
        assert_eq!(game.turn_score(), 1100);
    }

    #[test]
    fn banking_wins_at_4000() {
        let mut game = Game::new();
        game.current = 1;
        game.turn_score = 4000;
        assert_eq!(game.bank(), Some(1));
        assert_eq!(game.winner(), Some(1));
    }

    #[test]
    fn player_turns_alternate() {
        let mut game = Game::new();
        assert_eq!(game.current_player(), 0);
        game.current = 0;
        let _ = game.bank();
        assert_eq!(game.current_player(), 1);
    }

    #[test]
    fn continued_turn_scoring_removes_a_die() {
        let mut game = Game::new();
        // Three dice in play from a continued turn.
        game.dice_count = 3;
        game.dice = [1, 5, 2, 0, 0];
        let gained = game.score_selected(&[0]).unwrap();
        assert_eq!(gained, 100);
        assert_eq!(game.dice_count(), 2, "scoring one die should leave 2");
        assert_eq!(game.dice(), &[5, 2]);
    }

    #[test]
    fn continued_turn_scoring_removes_a_die_index_2() {
        let mut game = Game::new();
        game.dice_count = 3;
        game.dice = [2, 5, 1, 0, 0];
        let gained = game.score_selected(&[2]).unwrap();
        assert_eq!(gained, 100);
        assert_eq!(game.dice_count(), 2, "scoring one die should leave 2");
        assert_eq!(game.dice(), &[2, 5]);
    }

    #[test]
    fn score_single_one_then_small_street() {
        // Full street 1-2-3-4-5: bank the single 1, then the remaining street.
        let mut game = Game::new();
        game.dice_count = 5;
        game.dice = [1, 2, 3, 4, 5];
        let g1 = game.score_selected(&[0]).unwrap();
        assert_eq!(g1, 100);
        assert_eq!(game.dice(), &[2, 3, 4, 5]);
        assert_eq!(game.dice_count(), 4);
        let g2 = game.score_selected(&[0, 1, 2, 3]).unwrap(); // 2,3,4,5
        assert_eq!(g2, 500);
        assert_eq!(
            game.dice_count(),
            DICE_COUNT,
            "street cleared the remaining dice, so hot dice resets to five"
        );
        assert_eq!(game.turn_score(), 600);
    }

    #[test]
    fn score_small_street_then_single_one() {
        // 2-3-4-5 street first, then the single 1.
        let mut game = Game::new();
        game.dice_count = 5;
        game.dice = [2, 3, 4, 5, 1];
        let g1 = game.score_selected(&[0, 1, 2, 3]).unwrap(); // 2,3,4,5
        assert_eq!(g1, 500);
        assert_eq!(game.dice(), &[1]);
        let g2 = game.score_selected(&[0]).unwrap();
        assert_eq!(g2, 100);
        assert_eq!(game.dice_count(), DICE_COUNT);
        assert_eq!(game.turn_score(), 600);
    }

    #[test]
    fn three_sixes_after_scoring_two_fives() {
        // Two 5s banked (100), then the remaining three 6s are a valid triple.
        let mut game = Game::new();
        game.dice_count = 5;
        game.dice = [5, 5, 6, 6, 6];
        let g1 = game.score_selected(&[0, 1]).unwrap();
        assert_eq!(g1, 100);
        assert_eq!(game.dice(), &[6, 6, 6]);
        let g2 = game.score_selected(&[0, 1, 2]).unwrap();
        assert_eq!(g2, 600);
        assert_eq!(game.dice_count(), DICE_COUNT); // hot dice after clearing all
        assert_eq!(game.turn_score(), 700);
    }
}
