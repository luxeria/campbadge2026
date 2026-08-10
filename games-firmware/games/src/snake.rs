//! A snake game built on a square grid centred on the round badge display.
//!
//! The playfield is an inscribed square grid so every cell stays within the
//! round bezel. The snake logic is pure and hardware free, so it can be unit
//! tested on the host and driven by the firmware through [`Snake`].

use raylib_camp::canvas::Canvas;
use raylib_camp::color::Color;
use raylib_camp::rand::Prng;

/// The "Kirokaze Gameboy" palette (Lospec) used to render the game.
/// Colors: #332c50 ink, #46878f teal, #94e344 lime, #e2f3e4 pale mint.
pub mod pastel {
    use super::Color;

    /// Pale mint playfield background.
    pub const BACKGROUND: Color = Color::rgb565(0xe2, 0xf3, 0xe4);
    /// Dark ink snake body.
    pub const BODY: Color = Color::rgb565(0x33, 0x2c, 0x50);
    /// Teal snake head.
    pub const HEAD: Color = Color::rgb565(0x46, 0x87, 0x8f);
    /// Lime apple.
    pub const APPLE: Color = Color::rgb565(0x94, 0xe3, 0x44);
}

/// Playfield side length in grid cells.
pub const GRID: i32 = 15;
/// Pixel size of each cell.
pub const CELL: i32 = 10;
/// Pixel coordinates of the playfield's top-left corner, centring the
/// 150px square on the 240px round panel.
pub const ORIGIN_X: i32 = 45;
/// Pixel coordinates of the playfield's top-left corner.
pub const ORIGIN_Y: i32 = 45;
/// Upper bound on snake length, one segment per grid cell.
pub const MAX_LEN: usize = (GRID * GRID) as usize;

/// A cardinal movement direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

impl Direction {
    fn step(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    fn is_opposite(self, other: Direction) -> bool {
        matches!(
            (self, other),
            (Direction::Up, Direction::Down)
                | (Direction::Down, Direction::Up)
                | (Direction::Left, Direction::Right)
                | (Direction::Right, Direction::Left)
        )
    }
}

/// Outcome of one snake update tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepResult {
    /// The snake moved without incident.
    Ongoing,
    /// The snake ate the food.
    Ate,
    /// The snake hit a wall or itself.
    Died,
}

/// The game state for one snake.
pub struct Snake {
    segments: [(i16, i16); MAX_LEN],
    length: usize,
    direction: Direction,
    food: (i16, i16),
    rng: Prng,
}

impl Snake {
    /// Creates a snake with three centred segments heading right.
    pub fn new(seed: u32) -> Self {
        let rng = Prng::new(seed);
        let mut segments = [(0i16, 0i16); MAX_LEN];
        let middle = (GRID / 2) as i16;
        segments[0] = (middle, middle);
        segments[1] = (middle - 1, middle);
        segments[2] = (middle - 2, middle);
        let mut snake = Snake {
            segments,
            length: 3,
            direction: Direction::Right,
            food: (0, 0),
            rng,
        };
        snake.place_food();
        snake
    }

    /// Steers the snake, ignoring a 180 degree reversal into itself.
    pub fn set_direction(&mut self, direction: Direction) {
        if !direction.is_opposite(self.direction) {
            self.direction = direction;
        }
    }

    /// Returns the score, i.e. segments beyond the initial three.
    pub fn score(&self) -> usize {
        self.length - 3
    }

    /// Advances the snake by one cell.
    pub fn update(&mut self) -> StepResult {
        if self.length >= MAX_LEN {
            return StepResult::Died;
        }
        let (dx, dy) = self.direction.step();
        let head = self.segments[0];
        let grid = GRID as i16;
        let mut new_head = (head.0 + dx as i16, head.1 + dy as i16);
        // Toroidal board: leaving one edge reappears at the opposite edge.
        if new_head.0 < 0 {
            new_head.0 += grid;
        } else if new_head.0 >= grid {
            new_head.0 -= grid;
        }
        if new_head.1 < 0 {
            new_head.1 += grid;
        } else if new_head.1 >= grid {
            new_head.1 -= grid;
        }

        let eating = new_head == self.food;
        // The tail stays put when growing, so it is only safe to overlap it when
        // we are about to eat and the tail will move away.
        let checked = if eating { self.length } else { self.length - 1 };
        if self.segments[..checked].contains(&new_head) {
            return StepResult::Died;
        }

        // Push a new head on; the tail is naturally dropped unless we grow.
        for index in (1..=self.length).rev() {
            self.segments[index] = self.segments[index - 1];
        }
        self.segments[0] = new_head;

        if eating {
            self.length = (self.length + 1).min(MAX_LEN);
            self.place_food();
            StepResult::Ate
        } else {
            StepResult::Ongoing
        }
    }

    fn place_food(&mut self) {
        for _ in 0..1000 {
            let candidate = (
                self.rng.next_range(GRID as u32) as i16,
                self.rng.next_range(GRID as u32) as i16,
            );
            if !self.segments[..self.length].contains(&candidate) {
                self.food = candidate;
                return;
            }
        }
        // The board is nearly full; fall back to any cell that is not the head.
        let mut candidate = self.segments[0];
        while candidate == self.segments[0] {
            candidate = (
                self.rng.next_range(GRID as u32) as i16,
                self.rng.next_range(GRID as u32) as i16,
            );
        }
        self.food = candidate;
    }

    /// Rasterises the food, body and head onto the canvas.
    pub fn draw(&self, canvas: &mut Canvas) {
        let food_px = (
            ORIGIN_X + self.food.0 as i32 * CELL + CELL / 2,
            ORIGIN_Y + self.food.1 as i32 * CELL + CELL / 2,
        );
        canvas.circle_filled(food_px.0, food_px.1, CELL / 2, pastel::APPLE);

        for segment in &self.segments[1..self.length] {
            canvas.rect_filled(
                ORIGIN_X + segment.0 as i32 * CELL,
                ORIGIN_Y + segment.1 as i32 * CELL,
                CELL,
                CELL,
                pastel::BODY,
            );
        }

        let head = self.segments[0];
        canvas.rect_filled(
            ORIGIN_X + head.0 as i32 * CELL,
            ORIGIN_Y + head.1 as i32 * CELL,
            CELL,
            CELL,
            pastel::HEAD,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, Snake, StepResult, GRID, MAX_LEN};

    #[test]
    fn new_snake_is_three_long_and_centred() {
        let snake = Snake::new(1);
        assert_eq!(snake.length, 3);
        let middle = (GRID / 2) as i16;
        assert_eq!(snake.segments[0], (middle, middle));
    }

    #[test]
    fn moving_advances_the_head() {
        let mut snake = Snake::new(1);
        let before = snake.segments[0];
        let result = snake.update();
        assert_eq!(result, StepResult::Ongoing);
        assert_eq!(snake.segments[0], (before.0 + 1, before.1));
        assert_eq!(snake.length, 3);
    }

    #[test]
    fn opposite_direction_is_rejected() {
        let mut snake = Snake::new(1);
        snake.set_direction(Direction::Right); // already heading right
        let forward = snake.segments[0];
        snake.set_direction(Direction::Left); // reversal, ignored
        snake.update();
        assert_eq!(snake.segments[0], (forward.0 + 1, forward.1));
    }

    #[test]
    fn eating_grows_the_snake() {
        let mut snake = Snake::new(1);
        // Force the food directly in front of the head.
        let head = snake.segments[0];
        snake.food = (head.0 + 1, head.1);
        let result = snake.update();
        assert_eq!(result, StepResult::Ate);
        assert_eq!(snake.length, 4);
    }

    #[test]
    fn wrapping_reappears_at_opposite_edge() {
        let mut snake = Snake::new(1); // defaults to heading right, tail trailing left
        let mut result = StepResult::Ongoing;
        // From the centre to the opposite edge wraps the head to column 0.
        let steps = GRID - GRID / 2;
        for _ in 0..steps {
            result = snake.update();
        }
        assert_eq!(result, StepResult::Ongoing);
        assert_eq!(snake.segments[0].0, 0);
    }

    #[test]
    fn array_capacity_covers_the_full_grid() {
        assert_eq!(MAX_LEN, (GRID * GRID) as usize);
    }
}
