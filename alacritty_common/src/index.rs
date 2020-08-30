//! Line and Column newtypes for strongly typed tty/grid/terminal APIs.

/// Indexing types and implementations for Grid and Line.
use std::cmp::{Ord, Ordering};
use std::fmt;
use std::ops::{self, Add, AddAssign, Deref, Range, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// The side of a cell.
pub type Side = Direction;

/// Horizontal direction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Direction {
    Left,
    Right,
}

impl Direction {
    pub fn opposite(self) -> Self {
        match self {
            Side::Right => Side::Left,
            Side::Left => Side::Right,
        }
    }
}

/// Behavior for handling grid boundaries.
pub enum Boundary {
    /// Clamp to grid boundaries.
    ///
    /// When an operation exceeds the grid boundaries, the last point will be returned no matter
    /// how far the boundaries were exceeded.
    Clamp,

    /// Wrap around grid bondaries.
    ///
    /// When an operation exceeds the grid boundaries, the point will wrap around the entire grid
    /// history and continue at the other side.
    Wrap,
}

/// Index in the grid using row, column notation.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Point<L = Line> {
    pub line: L,
    pub col: Column,
}

impl<L> Point<L> {
    pub fn new(line: L, col: Column) -> Point<L> {
        Point { line, col }
    }

    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn sub(mut self, num_cols: Column, rhs: usize) -> Point<L>
    where
        L: Copy + Default + Into<Line> + Add<usize, Output = L> + Sub<usize, Output = L>,
    {
        let num_cols = num_cols.0;
        let line_changes = (rhs + num_cols - 1).saturating_sub(self.col.0) / num_cols;
        if self.line.into() >= Line(line_changes) {
            self.line = self.line - line_changes;
            self.col = Column((num_cols + self.col.0 - rhs % num_cols) % num_cols);
            self
        } else {
            Point::new(L::default(), Column(0))
        }
    }

    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn add(mut self, num_cols: Column, rhs: usize) -> Point<L>
    where
        L: Copy + Default + Into<Line> + Add<usize, Output = L> + Sub<usize, Output = L>,
    {
        let num_cols = num_cols.0;
        self.line = self.line + (rhs + self.col.0) / num_cols;
        self.col = Column((self.col.0 + rhs) % num_cols);
        self
    }
}

impl Point<usize> {
    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn sub_absolute<D>(mut self, dimensions: &D, boundary: Boundary, rhs: usize) -> Point<usize>
    where
        D: Dimensions,
    {
        let total_lines = dimensions.total_lines();
        let num_cols = dimensions.cols().0;

        self.line += (rhs + num_cols - 1).saturating_sub(self.col.0) / num_cols;
        self.col = Column((num_cols + self.col.0 - rhs % num_cols) % num_cols);

        if self.line >= total_lines {
            match boundary {
                Boundary::Clamp => Point::new(total_lines - 1, Column(0)),
                Boundary::Wrap => Point::new(self.line - total_lines, self.col),
            }
        } else {
            self
        }
    }

    #[inline]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn add_absolute<D>(mut self, dimensions: &D, boundary: Boundary, rhs: usize) -> Point<usize>
    where
        D: Dimensions,
    {
        let num_cols = dimensions.cols();

        let line_delta = (rhs + self.col.0) / num_cols.0;

        if self.line >= line_delta {
            self.line -= line_delta;
            self.col = Column((self.col.0 + rhs) % num_cols.0);
            self
        } else {
            match boundary {
                Boundary::Clamp => Point::new(0, num_cols - 1),
                Boundary::Wrap => {
                    let col = Column((self.col.0 + rhs) % num_cols.0);
                    let line = dimensions.total_lines() + self.line - line_delta;
                    Point::new(line, col)
                }
            }
        }
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Point) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Point) -> Ordering {
        match (self.line.cmp(&other.line), self.col.cmp(&other.col)) {
            (Ordering::Equal, ord) | (ord, _) => ord,
        }
    }
}

impl PartialOrd for Point<usize> {
    fn partial_cmp(&self, other: &Point<usize>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point<usize> {
    fn cmp(&self, other: &Point<usize>) -> Ordering {
        match (self.line.cmp(&other.line), self.col.cmp(&other.col)) {
            (Ordering::Equal, ord) => ord,
            (Ordering::Less, _) => Ordering::Greater,
            (Ordering::Greater, _) => Ordering::Less,
        }
    }
}

impl From<Point<usize>> for Point<isize> {
    fn from(point: Point<usize>) -> Self {
        Point::new(point.line as isize, point.col)
    }
}

impl From<Point<usize>> for Point<Line> {
    fn from(point: Point<usize>) -> Self {
        Point::new(Line(point.line), point.col)
    }
}

impl From<Point<isize>> for Point<usize> {
    fn from(point: Point<isize>) -> Self {
        Point::new(point.line as usize, point.col)
    }
}

impl From<Point> for Point<usize> {
    fn from(point: Point) -> Self {
        Point::new(point.line.0, point.col)
    }
}

/// A line.
///
/// Newtype to avoid passing values incorrectly.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Line(pub usize);

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A column.
///
/// Newtype to avoid passing values incorrectly.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Column(pub usize);

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A linear index.
///
/// Newtype to avoid passing values incorrectly.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Linear(pub usize);

impl Linear {
    pub fn new(columns: Column, column: Column, line: Line) -> Self {
        Linear(line.0 * columns.0 + column.0)
    }

    pub fn from_point(columns: Column, point: Point<usize>) -> Self {
        Linear(point.line * columns.0 + point.col.0)
    }
}

impl fmt::Display for Linear {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Linear({})", self.0)
    }
}

// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// implements binary operators "&T op U", "T op &U", "&T op &U"
// based on "T op U" where T and U are expected to be `Copy`able
macro_rules! forward_ref_binop {
    (impl $imp:ident, $method:ident for $t:ty, $u:ty) => {
        impl<'a> $imp<$u> for &'a $t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            fn $method(self, other: $u) -> <$t as $imp<$u>>::Output {
                $imp::$method(*self, other)
            }
        }

        impl<'a> $imp<&'a $u> for $t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            fn $method(self, other: &'a $u) -> <$t as $imp<$u>>::Output {
                $imp::$method(self, *other)
            }
        }

        impl<'a, 'b> $imp<&'a $u> for &'b $t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            fn $method(self, other: &'a $u) -> <$t as $imp<$u>>::Output {
                $imp::$method(*self, *other)
            }
        }
    };
}

/// Macro for deriving deref.
macro_rules! deref {
    ($ty:ty, $target:ty) => {
        impl Deref for $ty {
            type Target = $target;

            #[inline]
            fn deref(&self) -> &$target {
                &self.0
            }
        }
    };
}

macro_rules! add {
    ($ty:ty, $construct:expr) => {
        impl ops::Add<$ty> for $ty {
            type Output = $ty;

            #[inline]
            fn add(self, rhs: $ty) -> $ty {
                $construct(self.0 + rhs.0)
            }
        }
    };
}

macro_rules! sub {
    ($ty:ty, $construct:expr) => {
        impl ops::Sub<$ty> for $ty {
            type Output = $ty;

            #[inline]
            fn sub(self, rhs: $ty) -> $ty {
                $construct(self.0 - rhs.0)
            }
        }

        impl<'a> ops::Sub<$ty> for &'a $ty {
            type Output = $ty;

            #[inline]
            fn sub(self, rhs: $ty) -> $ty {
                $construct(self.0 - rhs.0)
            }
        }

        impl<'a> ops::Sub<&'a $ty> for $ty {
            type Output = $ty;

            #[inline]
            fn sub(self, rhs: &'a $ty) -> $ty {
                $construct(self.0 - rhs.0)
            }
        }

        impl<'a, 'b> ops::Sub<&'a $ty> for &'b $ty {
            type Output = $ty;

            #[inline]
            fn sub(self, rhs: &'a $ty) -> $ty {
                $construct(self.0 - rhs.0)
            }
        }
    };
}

/// This exists because we can't implement Iterator on Range
/// and the existing impl needs the unstable Step trait
/// This should be removed and replaced with a Step impl
/// in the ops macro when `step_by` is stabilized.
pub struct IndexRange<T>(pub Range<T>);

impl<T> From<Range<T>> for IndexRange<T> {
    fn from(from: Range<T>) -> Self {
        IndexRange(from)
    }
}

macro_rules! ops {
    ($ty:ty, $construct:expr) => {
        add!($ty, $construct);
        sub!($ty, $construct);
        deref!($ty, usize);
        forward_ref_binop!(impl Add, add for $ty, $ty);

        impl $ty {
            #[inline]
            fn steps_between(start: $ty, end: $ty, by: $ty) -> Option<usize> {
                if by == $construct(0) { return None; }
                if start < end {
                    // Note: We assume $t <= usize here.
                    let diff = (end - start).0;
                    let by = by.0;
                    if diff % by > 0 {
                        Some(diff / by + 1)
                    } else {
                        Some(diff / by)
                    }
                } else {
                    Some(0)
                }
            }

            #[inline]
            fn steps_between_by_one(start: $ty, end: $ty) -> Option<usize> {
                Self::steps_between(start, end, $construct(1))
            }
        }

        impl Iterator for IndexRange<$ty> {
            type Item = $ty;
            #[inline]
            fn next(&mut self) -> Option<$ty> {
                if self.0.start < self.0.end {
                    let old = self.0.start;
                    self.0.start = old + 1;
                    Some(old)
                } else {
                    None
                }
            }
            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                match Self::Item::steps_between_by_one(self.0.start, self.0.end) {
                    Some(hint) => (hint, Some(hint)),
                    None => (0, None)
                }
            }
        }

        impl DoubleEndedIterator for IndexRange<$ty> {
            #[inline]
            fn next_back(&mut self) -> Option<$ty> {
                if self.0.start < self.0.end {
                    let new = self.0.end - 1;
                    self.0.end = new;
                    Some(new)
                } else {
                    None
                }
            }
        }
        impl AddAssign<$ty> for $ty {
            #[inline]
            fn add_assign(&mut self, rhs: $ty) {
                self.0 += rhs.0
            }
        }

        impl SubAssign<$ty> for $ty {
            #[inline]
            fn sub_assign(&mut self, rhs: $ty) {
                self.0 -= rhs.0
            }
        }

        impl AddAssign<usize> for $ty {
            #[inline]
            fn add_assign(&mut self, rhs: usize) {
                self.0 += rhs
            }
        }

        impl SubAssign<usize> for $ty {
            #[inline]
            fn sub_assign(&mut self, rhs: usize) {
                self.0 -= rhs
            }
        }

        impl From<usize> for $ty {
            #[inline]
            fn from(val: usize) -> $ty {
                $construct(val)
            }
        }

        impl Add<usize> for $ty {
            type Output = $ty;

            #[inline]
            fn add(self, rhs: usize) -> $ty {
                $construct(self.0 + rhs)
            }
        }

        impl Sub<usize> for $ty {
            type Output = $ty;

            #[inline]
            fn sub(self, rhs: usize) -> $ty {
                $construct(self.0 - rhs)
            }
        }
    }
}

ops!(Line, Line);
ops!(Column, Column);
ops!(Linear, Linear);

#[derive(Copy, Clone, Debug)]
pub struct RenderableCell {
    /// A _Display_ line (not necessarily an _Active_ line).
    pub line: Line,
    pub column: Column,
    pub inner: RenderableCellContent,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bg_alpha: f32,
    pub flags: Flags,
}

impl RenderableCell {
    fn new<'a, C>(iter: &mut RenderableCellsIter<'a, C>, cell: Indexed<Cell>) -> Self {
        let point = Point::new(cell.line, cell.column);

        // Lookup RGB values.
        let mut fg_rgb = Self::compute_fg_rgb(iter.config, iter.colors, cell.fg, cell.flags);
        let mut bg_rgb = Self::compute_bg_rgb(iter.colors, cell.bg);

        let mut bg_alpha = if cell.inverse() {
            mem::swap(&mut fg_rgb, &mut bg_rgb);
            1.0
        } else {
            Self::compute_bg_alpha(cell.bg)
        };

        if iter.is_selected(point) {
            let config_bg = iter.config.colors.selection.background();
            let selected_fg = iter.config.colors.selection.text().color(fg_rgb, bg_rgb);
            bg_rgb = config_bg.color(fg_rgb, bg_rgb);
            fg_rgb = selected_fg;

            if fg_rgb == bg_rgb && !cell.flags.contains(Flags::HIDDEN) {
                // Reveal inversed text when fg/bg is the same.
                fg_rgb = iter.colors[NamedColor::Background];
                bg_rgb = iter.colors[NamedColor::Foreground];
                bg_alpha = 1.0;
            } else if config_bg != CellRgb::CellBackground {
                bg_alpha = 1.0;
            }
        } else if iter.search.advance(iter.grid.visible_to_buffer(point)) {
            // Highlight the cell if it is part of a search match.
            let config_bg = iter.config.colors.search.matches.background;
            let matched_fg = iter.config.colors.search.matches.foreground.color(fg_rgb, bg_rgb);
            bg_rgb = config_bg.color(fg_rgb, bg_rgb);
            fg_rgb = matched_fg;

            if config_bg != CellRgb::CellBackground {
                bg_alpha = 1.0;
            }
        }

        RenderableCell {
            line: cell.line,
            column: cell.column,
            inner: RenderableCellContent::Chars(cell.chars()),
            fg: fg_rgb,
            bg: bg_rgb,
            bg_alpha,
            flags: cell.flags,
        }
    }

    fn is_empty(&self) -> bool {
        self.bg_alpha == 0.
            && !self.flags.intersects(Flags::UNDERLINE | Flags::STRIKEOUT | Flags::DOUBLE_UNDERLINE)
            && self.inner == RenderableCellContent::Chars([' '; cell::MAX_ZEROWIDTH_CHARS + 1])
    }

    fn compute_fg_rgb<C>(config: &Config<C>, colors: &color::List, fg: Color, flags: Flags) -> Rgb {
        match fg {
            Color::Spec(rgb) => match flags & Flags::DIM {
                Flags::DIM => rgb * DIM_FACTOR,
                _ => rgb,
            },
            Color::Named(ansi) => {
                match (config.draw_bold_text_with_bright_colors(), flags & Flags::DIM_BOLD) {
                    // If no bright foreground is set, treat it like the BOLD flag doesn't exist.
                    (_, Flags::DIM_BOLD)
                        if ansi == NamedColor::Foreground
                            && config.colors.primary.bright_foreground.is_none() =>
                    {
                        colors[NamedColor::DimForeground]
                    }
                    // Draw bold text in bright colors *and* contains bold flag.
                    (true, Flags::BOLD) => colors[ansi.to_bright()],
                    // Cell is marked as dim and not bold.
                    (_, Flags::DIM) | (false, Flags::DIM_BOLD) => colors[ansi.to_dim()],
                    // None of the above, keep original color..
                    _ => colors[ansi],
                }
            }
            Color::Indexed(idx) => {
                let idx = match (
                    config.draw_bold_text_with_bright_colors(),
                    flags & Flags::DIM_BOLD,
                    idx,
                ) {
                    (true, Flags::BOLD, 0..=7) => idx as usize + 8,
                    (false, Flags::DIM, 8..=15) => idx as usize - 8,
                    (false, Flags::DIM, 0..=7) => idx as usize + 260,
                    _ => idx as usize,
                };

                colors[idx]
            }
        }
    }

    /// Compute background alpha based on cell's original color.
    ///
    /// Since an RGB color matching the background should not be transparent, this is computed
    /// using the named input color, rather than checking the RGB of the background after its color
    /// is computed.
    #[inline]
    fn compute_bg_alpha(bg: Color) -> f32 {
        if bg == Color::Named(NamedColor::Background) {
            0.
        } else {
            1.
        }
    }

    #[inline]
    fn compute_bg_rgb(colors: &color::List, bg: Color) -> Rgb {
        match bg {
            Color::Spec(rgb) => rgb,
            Color::Named(ansi) => colors[ansi],
            Color::Indexed(idx) => colors[idx],
        }
    }
}

impl From<RenderableCell> for Point<Line> {
    fn from(cell: RenderableCell) -> Self {
        Point::new(cell.line, cell.column)
    }
}

impl<'a, C> Iterator for RenderableCellsIter<'a, C> {
    type Item = RenderableCell;

    /// Gets the next renderable cell.
    ///
    /// Skips empty (background) cells and applies any flags to the cell state
    /// (eg. invert fg and bg colors).
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.cursor.point.line == self.inner.line()
                && self.cursor.point.col == self.inner.column()
            {
                if self.cursor.rendered {
                    // Handle cell below cursor.
                    let cell = self.inner.next()?;
                    let mut cell = RenderableCell::new(self, cell);

                    if self.cursor.key.style == CursorStyle::Block {
                        cell.fg = match self.cursor.cursor_color {
                            // Apply cursor color, or invert the cursor if it has a fixed background
                            // close to the cell's background.
                            CellRgb::Rgb(col) if col.contrast(cell.bg) < MIN_CURSOR_CONTRAST => {
                                cell.bg
                            }
                            _ => self.cursor.text_color.color(cell.fg, cell.bg),
                        };
                    }

                    return Some(cell);
                } else {
                    // Handle cursor.
                    self.cursor.rendered = true;

                    let buffer_point = self.grid.visible_to_buffer(self.cursor.point);
                    let cell = Indexed {
                        inner: self.grid[buffer_point.line][buffer_point.col],
                        column: self.cursor.point.col,
                        line: self.cursor.point.line,
                    };

                    let mut cell = RenderableCell::new(self, cell);
                    cell.inner = RenderableCellContent::Cursor(self.cursor.key);

                    // Apply cursor color, or invert the cursor if it has a fixed background close
                    // to the cell's background.
                    if !matches!(
                        self.cursor.cursor_color,
                        CellRgb::Rgb(color) if color.contrast(cell.bg) < MIN_CURSOR_CONTRAST
                    ) {
                        cell.fg = self.cursor.cursor_color.color(cell.fg, cell.bg);
                    }

                    return Some(cell);
                }
            } else {
                let cell = self.inner.next()?;
                let cell = RenderableCell::new(self, cell);

                if !cell.is_empty() {
                    return Some(cell);
                }
            }
        }
    }
}

/// Grid dimensions.
pub trait Dimensions {
    /// Total number of lines in the buffer, this includes scrollback and visible lines.
    fn total_lines(&self) -> usize;

    /// Height of the viewport in lines.
    fn screen_lines(&self) -> Line;

    /// Width of the terminal in columns.
    fn cols(&self) -> Column;

    /// Number of invisible lines part of the scrollback history.
    #[inline]
    fn history_size(&self) -> usize {
        self.total_lines() - self.screen_lines().0
    }
}

impl<G> Dimensions for Grid<G> {
    #[inline]
    fn total_lines(&self) -> usize {
        self.raw.len()
    }

    #[inline]
    fn screen_lines(&self) -> Line {
        self.lines
    }

    #[inline]
    fn cols(&self) -> Column {
        self.cols
    }
}

#[cfg(test)]
impl Dimensions for (Line, Column) {
    fn total_lines(&self) -> usize {
        *self.0
    }

    fn screen_lines(&self) -> Line {
        self.0
    }

    fn cols(&self) -> Column {
        self.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_ordering() {
        assert!(Point::new(Line(0), Column(0)) == Point::new(Line(0), Column(0)));
        assert!(Point::new(Line(1), Column(0)) > Point::new(Line(0), Column(0)));
        assert!(Point::new(Line(0), Column(1)) > Point::new(Line(0), Column(0)));
        assert!(Point::new(Line(1), Column(1)) > Point::new(Line(0), Column(0)));
        assert!(Point::new(Line(1), Column(1)) > Point::new(Line(0), Column(1)));
        assert!(Point::new(Line(1), Column(1)) > Point::new(Line(1), Column(0)));
    }

    #[test]
    fn sub() {
        let num_cols = Column(42);
        let point = Point::new(0, Column(13));

        let result = point.sub(num_cols, 1);

        assert_eq!(result, Point::new(0, point.col - 1));
    }

    #[test]
    fn sub_wrap() {
        let num_cols = Column(42);
        let point = Point::new(1, Column(0));

        let result = point.sub(num_cols, 1);

        assert_eq!(result, Point::new(0, num_cols - 1));
    }

    #[test]
    fn sub_clamp() {
        let num_cols = Column(42);
        let point = Point::new(0, Column(0));

        let result = point.sub(num_cols, 1);

        assert_eq!(result, point);
    }

    #[test]
    fn add() {
        let num_cols = Column(42);
        let point = Point::new(0, Column(13));

        let result = point.add(num_cols, 1);

        assert_eq!(result, Point::new(0, point.col + 1));
    }

    #[test]
    fn add_wrap() {
        let num_cols = Column(42);
        let point = Point::new(0, num_cols - 1);

        let result = point.add(num_cols, 1);

        assert_eq!(result, Point::new(1, Column(0)));
    }

    #[test]
    fn add_absolute() {
        let point = Point::new(0, Column(13));

        let result = point.add_absolute(&(Line(1), Column(42)), Boundary::Clamp, 1);

        assert_eq!(result, Point::new(0, point.col + 1));
    }

    #[test]
    fn add_absolute_wrapline() {
        let point = Point::new(1, Column(41));

        let result = point.add_absolute(&(Line(2), Column(42)), Boundary::Clamp, 1);

        assert_eq!(result, Point::new(0, Column(0)));
    }

    #[test]
    fn add_absolute_multiline_wrapline() {
        let point = Point::new(2, Column(9));

        let result = point.add_absolute(&(Line(3), Column(10)), Boundary::Clamp, 11);

        assert_eq!(result, Point::new(0, Column(0)));
    }

    #[test]
    fn add_absolute_clamp() {
        let point = Point::new(0, Column(41));

        let result = point.add_absolute(&(Line(1), Column(42)), Boundary::Clamp, 1);

        assert_eq!(result, point);
    }

    #[test]
    fn add_absolute_wrap() {
        let point = Point::new(0, Column(41));

        let result = point.add_absolute(&(Line(3), Column(42)), Boundary::Wrap, 1);

        assert_eq!(result, Point::new(2, Column(0)));
    }

    #[test]
    fn add_absolute_multiline_wrap() {
        let point = Point::new(0, Column(9));

        let result = point.add_absolute(&(Line(3), Column(10)), Boundary::Wrap, 11);

        assert_eq!(result, Point::new(1, Column(0)));
    }

    #[test]
    fn sub_absolute() {
        let point = Point::new(0, Column(13));

        let result = point.sub_absolute(&(Line(1), Column(42)), Boundary::Clamp, 1);

        assert_eq!(result, Point::new(0, point.col - 1));
    }

    #[test]
    fn sub_absolute_wrapline() {
        let point = Point::new(0, Column(0));

        let result = point.sub_absolute(&(Line(2), Column(42)), Boundary::Clamp, 1);

        assert_eq!(result, Point::new(1, Column(41)));
    }

    #[test]
    fn sub_absolute_multiline_wrapline() {
        let point = Point::new(0, Column(0));

        let result = point.sub_absolute(&(Line(3), Column(10)), Boundary::Clamp, 11);

        assert_eq!(result, Point::new(2, Column(9)));
    }

    #[test]
    fn sub_absolute_wrap() {
        let point = Point::new(2, Column(0));

        let result = point.sub_absolute(&(Line(3), Column(42)), Boundary::Wrap, 1);

        assert_eq!(result, Point::new(0, Column(41)));
    }

    #[test]
    fn sub_absolute_multiline_wrap() {
        let point = Point::new(2, Column(0));

        let result = point.sub_absolute(&(Line(3), Column(10)), Boundary::Wrap, 11);

        assert_eq!(result, Point::new(1, Column(9)));
    }
}
