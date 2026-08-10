//! Firmware entry point for the LuxCamp badge (M5Stack Atom S3 Lite).
//!
//! One program is compiled per game, chosen by the active cargo feature
//! (`demo`, `snake`, `seven` or `dice`, defaulting to `dice`). The shared board
//! is initialised once and handed to that game's entry point in the `apps`
//! module, each behind the matching feature gate.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

mod apps;
mod board;

#[cfg(feature = "dice")]
mod buzzer;
#[cfg(feature = "dice")]
mod expander;

// Embeds an ESP-IDF application descriptor so `espflash` can flash the binary.
esp_bootloader_esp_idf::esp_app_desc!();

/// Brings up the shared board, then runs the single game selected by the
/// active cargo feature. Exactly one of the `#[cfg]`ged calls is compiled in,
/// and each never returns.
#[main]
fn main() -> ! {
    let board = board::Board::new();
    #[cfg(feature = "demo")]
    apps::demo::main(board);
    #[cfg(feature = "snake")]
    apps::snake::main(board);
    #[cfg(feature = "seven")]
    apps::seven::main(board);
    #[cfg(feature = "dice")]
    apps::dice::main(board);
}
