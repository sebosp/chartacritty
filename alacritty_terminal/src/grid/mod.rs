//! A specialized 2D grid implementation optimized for use in a terminal.
/// Utilities moved to alacritty_terminal/src/grid/mod.rs
use crate::ansi::{CharsetIndex, StandardCharset};

pub use alacritty_common::grid::resize;
pub use alacritty_common::grid::row;
pub use alacritty_common::grid::storage;
#[cfg(test)]
mod tests;

pub use self::row::Row;
