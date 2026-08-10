//! Snake firmware, driven by the `games::snake` engine.
//!
//! The snake is steered with the four direction buttons, quickens as it grows,
//! and B5 turbo-scrolls it. B8 restarts after a game over.

use raylib_camp::canvas::Canvas;
use raylib_camp::color::palette;
use raylib_camp::input::Button;

use games::snake::{pastel, Direction, Snake, StepResult, CELL, GRID, ORIGIN_X, ORIGIN_Y};

use crate::board::Board;

/// Milliseconds between snake steps at the start of a game.
const BASE_TICK_MS: u32 = 180;
/// Fastest allowed step interval as the snake grows.
const MIN_TICK_MS: u32 = 45;
/// Milliseconds credited per frame to the step accumulator.
const FRAME_MS: u32 = 33;

/// Draws a small score numeral near the bottom of the round panel.
fn draw_score(canvas: &mut Canvas, score: usize) {
    let mut buffer = [0u8; 8];
    let mut value = score;
    let mut length = 0;
    if value == 0 {
        buffer[0] = b'0';
        length = 1;
    }
    while value > 0 && length < buffer.len() {
        buffer[length] = b'0' + (value % 10) as u8;
        value /= 10;
        length += 1;
    }
    buffer[..length].reverse();
    let text = core::str::from_utf8(&buffer[..length]).unwrap_or("0");
    let width = canvas.measure_text(text, 2);
    canvas.draw_text(text, 120 - width / 2, 210, 2, palette::WHITE);
}

/// Plays the snake game until power is removed.
pub fn main(mut board: Board) -> ! {
    let mut snake = Snake::new(0x5eed);
    let mut tick_accum: u32 = 0;
    let mut game_over = false;

    loop {
        board.frame();

        if board.input.just_pressed(Button::Btn1) {
            snake.set_direction(Direction::Left);
        } else if board.input.just_pressed(Button::Btn2) {
            snake.set_direction(Direction::Up);
        } else if board.input.just_pressed(Button::Btn3) {
            snake.set_direction(Direction::Right);
        } else if board.input.just_pressed(Button::Btn4) {
            snake.set_direction(Direction::Down);
        }
        if game_over && board.input.just_pressed(Button::Btn8) {
            snake = Snake::new(board.now_ms);
            game_over = false;
            tick_accum = 0;
        }

        let mut tick_ms = BASE_TICK_MS
            .saturating_sub(snake.score() as u32 * 16)
            .max(MIN_TICK_MS);
        if !game_over && board.input.pressed(Button::Btn5) {
            tick_ms /= 4;
        }
        tick_accum += FRAME_MS;
        if !game_over && tick_accum >= tick_ms {
            tick_accum -= tick_ms;
            if snake.update() == StepResult::Died {
                game_over = true;
            }
        }

        board.canvas.clear(palette::BLACK);
        board.canvas.rect_filled(
            ORIGIN_X,
            ORIGIN_Y,
            GRID * CELL,
            GRID * CELL,
            pastel::BACKGROUND,
        );
        board
            .canvas
            .rect(ORIGIN_X, ORIGIN_Y, GRID * CELL, GRID * CELL, palette::BLACK);
        if game_over {
            board
                .canvas
                .draw_text("GAME OVER", 76, 100, 2, pastel::BODY);
        } else {
            snake.draw(&mut board.canvas);
        }
        draw_score(&mut board.canvas, snake.score());

        board.flush();
        board.delay.delay_millis(FRAME_MS);
    }
}
