//! Interactive demo firmware.
//!
//! Renders a couple of engine primitives plus a bouncing, colour-changing
//! disc controlled by the eight buttons, to exercise the display, expander
//! and input pipeline.

use raylib_camp::canvas::Canvas;
use raylib_camp::color::palette;
use raylib_camp::input::Button;

use crate::board::Board;

/// Draws a static demonstration scene using the engine's drawing primitives.
fn draw_demo_scene(canvas: &mut Canvas) {
    canvas.rect(30, 40, 70, 50, palette::RED);
    canvas.rect_filled(120, 40, 70, 50, palette::GREEN);
    canvas.circle(70, 150, 20, palette::YELLOW);
    canvas.circle_filled(150, 150, 20, palette::CYAN);
    canvas.triangle_filled((100, 200), (160, 200), (130, 165), palette::MAGENTA);
    canvas.line(20, 20, 220, 30, palette::ORANGE);
    canvas.draw_text("CAMP 2026", 70, 4, 2, palette::WHITE);
}

/// Plays the interactive demo until power is removed.
pub fn main_demo(mut board: Board) -> ! {
    const CENTRE: (f32, f32) = (120.0, 120.0);
    const BOUNDARY: f32 = 92.0;

    let mut ball_radius: i32 = 8;
    let mut position: (f32, f32) = (120.0, 60.0);
    let mut velocity: (f32, f32) = (2.5, 2.0);
    let mut ball_color = palette::WHITE;

    loop {
        board.frame();
        board.canvas.clear(palette::BLACK);
        draw_demo_scene(&mut board.canvas);

        if board.input.just_pressed(Button::Btn1) {
            ball_color = palette::RED;
        } else if board.input.just_pressed(Button::Btn2) {
            ball_color = palette::GREEN;
        } else if board.input.just_pressed(Button::Btn3) {
            ball_color = palette::BLUE;
        } else if board.input.just_pressed(Button::Btn4) {
            ball_color = palette::YELLOW;
        }

        if board.input.pressed(Button::Btn5) {
            velocity.0 *= 1.15;
            velocity.1 *= 1.15;
        } else if board.input.pressed(Button::Btn6) {
            velocity.0 *= 0.85;
            velocity.1 *= 0.85;
        }
        if board.input.pressed(Button::Btn7) {
            ball_radius = ball_radius.saturating_add(3).min(40);
        } else if board.input.pressed(Button::Btn8) {
            ball_radius = ball_radius.saturating_sub(3).max(2);
        }

        let speed = raylib_camp::math::sqrt(velocity.0 * velocity.0 + velocity.1 * velocity.1);
        if speed > 20.0 {
            let scale = 20.0 / speed;
            velocity.0 *= scale;
            velocity.1 *= scale;
        } else if speed < 0.3 {
            let scale = 0.3 / speed;
            velocity.0 *= scale;
            velocity.1 *= scale;
        }

        let relative_x = position.0 - CENTRE.0;
        let relative_y = position.1 - CENTRE.1;
        let distance = raylib_camp::math::sqrt(relative_x * relative_x + relative_y * relative_y);
        if distance > 0.0 && distance + ball_radius as f32 > BOUNDARY {
            let normal_x = relative_x / distance;
            let normal_y = relative_y / distance;
            let dot = velocity.0 * normal_x + velocity.1 * normal_y;
            velocity.0 -= 2.0 * dot * normal_x;
            velocity.1 -= 2.0 * dot * normal_y;
            let overhang = distance + ball_radius as f32 - BOUNDARY;
            position.0 -= normal_x * overhang;
            position.1 -= normal_y * overhang;
        }
        position.0 += velocity.0;
        position.1 += velocity.1;
        board.canvas.circle_filled(
            position.0 as i32,
            position.1 as i32,
            ball_radius,
            ball_color,
        );

        board.flush();
        board.delay.delay_millis(33);
    }
}
