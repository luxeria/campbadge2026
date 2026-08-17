//! Game entry points for the firmware.
//!
//! Exactly one build feature is enabled at a time; the matching module here is
//! compiled and its [`main`] drives the board for the rest of the program.

#[cfg(feature = "demo")]
pub mod demo;
#[cfg(feature = "dice")]
pub mod dice;
#[cfg(feature = "seven")]
pub mod seven;
#[cfg(feature = "snake")]
pub mod snake;
