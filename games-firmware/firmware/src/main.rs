//! Firmware entry point for the LuxCamp badge (M5Stack Atom S3 Lite).
//!
//! One program is compiled per game, chosen by the active cargo feature
//! (`demo`, `snake`, `seven` or `dice`, defaulting to `dice`). The shared board
//! is initialised once and handed to that game's `main_*` function, each of
//! which lives in its own `firmware_*` module behind the matching feature gate.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

mod board;

#[cfg(feature = "dice")]
mod buzzer;
#[cfg(feature = "dice")]
mod expander;

#[cfg(feature = "demo")]
mod firmware_demo;
#[cfg(feature = "dice")]
mod firmware_dice;
#[cfg(feature = "seven")]
mod firmware_seven;
#[cfg(feature = "snake")]
mod firmware_snake;

// Embeds an ESP-IDF application descriptor so `espflash` can flash the binary.
esp_bootloader_esp_idf::esp_app_desc!();

/// Brings up the shared board, then runs the single game selected by the
/// active cargo feature. Exactly one of the `#[cfg]`ged calls is compiled in,
/// and each never returns.
#[main]
fn main() -> ! {
    let board = board::Board::new();
    #[cfg(feature = "demo")]
    firmware_demo::main_demo(board);
    #[cfg(feature = "snake")]
    firmware_snake::main_snake(board);
    #[cfg(feature = "seven")]
    firmware_seven::main_seven(board);
    #[cfg(feature = "dice")]
    firmware_dice::main_dice(board);
}
