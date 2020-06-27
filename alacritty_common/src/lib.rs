//! A set of functions that are shared and can be used to extend alacritty.
// This has been created so that other modules/extensions can depend on
// alacritty_terminal utilities without having to redefine them.

pub mod index;

pub use crate::index::*;
use serde::{Deserialize, Serialize};
use std::cmp::min;

/// Terminal size info.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub struct SizeInfo {
    /// Terminal window width.
    pub width: f32,

    /// Terminal window height.
    pub height: f32,

    /// Width of individual cell.
    pub cell_width: f32,

    /// Height of individual cell.
    pub cell_height: f32,

    /// Horizontal window padding.
    pub padding_x: f32,

    /// Horizontal window padding.
    pub padding_y: f32,

    /// DPR of the current window.
    #[serde(default)]
    pub dpr: f64,
}

impl SizeInfo {
    #[inline]
    pub fn lines(&self) -> Line {
        Line(((self.height - 2. * self.padding_y) / self.cell_height) as usize)
    }

    #[inline]
    pub fn cols(&self) -> Column {
        Column(((self.width - 2. * self.padding_x) / self.cell_width) as usize)
    }

    #[inline]
    pub fn padding_right(&self) -> usize {
        (self.padding_x + (self.width - 2. * self.padding_x) % self.cell_width) as usize
    }

    #[inline]
    pub fn padding_bottom(&self) -> usize {
        (self.padding_y + (self.height - 2. * self.padding_y) % self.cell_height) as usize
    }

    /// Check if coordinates are inside the terminal grid.
    ///
    /// The padding is not counted as part of the grid.
    #[inline]
    pub fn contains_point(&self, x: usize, y: usize) -> bool {
        x < (self.width as usize - self.padding_right())
            && x >= self.padding_x as usize
            && y < (self.height as usize - self.padding_bottom())
            && y >= self.padding_y as usize
    }

    pub fn pixels_to_coords(&self, x: usize, y: usize) -> Point {
        let col = Column(x.saturating_sub(self.padding_x as usize) / (self.cell_width as usize));
        let line = Line(y.saturating_sub(self.padding_y as usize) / (self.cell_height as usize));

        Point {
            line: min(line, Line(self.lines().saturating_sub(1))),
            col: min(col, Column(self.cols().saturating_sub(1))),
        }
    }
}
