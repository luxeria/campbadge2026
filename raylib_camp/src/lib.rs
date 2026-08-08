//! A slim, badge-tailored immediate-mode 2D graphics engine.
//!
//! `raylib_camp` is inspired by raylib's shape API but is custom built for the
//! LuxCamp badge's round 240x240 RGB565 display. It is allocation free, `no_std`
//! and has no dependencies beyond the Rust core library, so it can be used by
//! bare-metal firmware as well as run unit tests on the host.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod canvas;
pub mod color;
pub mod input;
pub mod math;
pub mod rand;
pub mod shapes;
pub mod sprites;
pub mod text;
pub mod timer;

pub use canvas::{display, Canvas};
pub use color::{palette, Color};
pub use input::Button;
pub use math::Vec2;
pub use rand::Prng;
pub use timer::FrameTimer;
