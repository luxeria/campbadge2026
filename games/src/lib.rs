//! Game crate aggregator. Individual games implement `no_std` logic over the
//! `raylib_camp` engine and are driven by the firmware.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod snake;
