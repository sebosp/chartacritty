/// Utilities  moved from alacritty_terminal/src/selection.rs
use alacritty_common::index::{Column, Line, Point, Side};
/// Represents a range of selected cells.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SelectionRange<L = usize> {
    /// Start point, top left of the selection.
    pub start: Point<L>,
    /// End point, bottom right of the selection.
    pub end: Point<L>,
    /// Whether this selection is a block selection.
    pub is_block: bool,
}
