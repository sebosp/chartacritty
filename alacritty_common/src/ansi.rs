use crate::term::color::Rgb;
/// Utilities from alacritty_terminal/src/ansi.rs
/// Standard or common character sets which can be designated as G0-G3.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardCharset {
    Ascii,
    SpecialCharacterAndLineDrawing,
}

impl Default for StandardCharset {
    fn default() -> Self {
        StandardCharset::Ascii
    }
}

impl StandardCharset {
    /// Switch/Map character to the active charset. Ascii is the common case and
    /// for that we want to do as little as possible.
    #[inline]
    pub fn map(self, c: char) -> char {
        match self {
            StandardCharset::Ascii => c,
            StandardCharset::SpecialCharacterAndLineDrawing => match c {
                '`' => '◆',
                'a' => '▒',
                'b' => '\t',
                'c' => '\u{000c}',
                'd' => '\r',
                'e' => '\n',
                'f' => '°',
                'g' => '±',
                'h' => '\u{2424}',
                'i' => '\u{000b}',
                'j' => '┘',
                'k' => '┐',
                'l' => '┌',
                'm' => '└',
                'n' => '┼',
                'o' => '⎺',
                'p' => '⎻',
                'q' => '─',
                'r' => '⎼',
                's' => '⎽',
                't' => '├',
                'u' => '┤',
                'v' => '┴',
                'w' => '┬',
                'x' => '│',
                'y' => '≤',
                'z' => '≥',
                '{' => 'π',
                '|' => '≠',
                '}' => '£',
                '~' => '·',
                _ => c,
            },
        }
    }
}

/// Standard colors.
///
/// The order here matters since the enum should be castable to a `usize` for
/// indexing a color list.
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NamedColor {
    /// Black.
    Black = 0,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// White.
    White,
    /// Bright black.
    BrightBlack,
    /// Bright red.
    BrightRed,
    /// Bright green.
    BrightGreen,
    /// Bright yellow.
    BrightYellow,
    /// Bright blue.
    BrightBlue,
    /// Bright magenta.
    BrightMagenta,
    /// Bright cyan.
    BrightCyan,
    /// Bright white.
    BrightWhite,
    /// The foreground color.
    Foreground = 256,
    /// The background color.
    Background,
    /// Color for the cursor itself.
    Cursor,
    /// Dim black.
    DimBlack,
    /// Dim red.
    DimRed,
    /// Dim green.
    DimGreen,
    /// Dim yellow.
    DimYellow,
    /// Dim blue.
    DimBlue,
    /// Dim magenta.
    DimMagenta,
    /// Dim cyan.
    DimCyan,
    /// Dim white.
    DimWhite,
    /// The bright foreground color.
    BrightForeground,
    /// Dim foreground.
    DimForeground,
}

impl NamedColor {
    pub fn to_bright(self) -> Self {
        match self {
            NamedColor::Foreground => NamedColor::BrightForeground,
            NamedColor::Black => NamedColor::BrightBlack,
            NamedColor::Red => NamedColor::BrightRed,
            NamedColor::Green => NamedColor::BrightGreen,
            NamedColor::Yellow => NamedColor::BrightYellow,
            NamedColor::Blue => NamedColor::BrightBlue,
            NamedColor::Magenta => NamedColor::BrightMagenta,
            NamedColor::Cyan => NamedColor::BrightCyan,
            NamedColor::White => NamedColor::BrightWhite,
            NamedColor::DimForeground => NamedColor::Foreground,
            NamedColor::DimBlack => NamedColor::Black,
            NamedColor::DimRed => NamedColor::Red,
            NamedColor::DimGreen => NamedColor::Green,
            NamedColor::DimYellow => NamedColor::Yellow,
            NamedColor::DimBlue => NamedColor::Blue,
            NamedColor::DimMagenta => NamedColor::Magenta,
            NamedColor::DimCyan => NamedColor::Cyan,
            NamedColor::DimWhite => NamedColor::White,
            val => val,
        }
    }

    pub fn to_dim(self) -> Self {
        match self {
            NamedColor::Black => NamedColor::DimBlack,
            NamedColor::Red => NamedColor::DimRed,
            NamedColor::Green => NamedColor::DimGreen,
            NamedColor::Yellow => NamedColor::DimYellow,
            NamedColor::Blue => NamedColor::DimBlue,
            NamedColor::Magenta => NamedColor::DimMagenta,
            NamedColor::Cyan => NamedColor::DimCyan,
            NamedColor::White => NamedColor::DimWhite,
            NamedColor::Foreground => NamedColor::DimForeground,
            NamedColor::BrightBlack => NamedColor::Black,
            NamedColor::BrightRed => NamedColor::Red,
            NamedColor::BrightGreen => NamedColor::Green,
            NamedColor::BrightYellow => NamedColor::Yellow,
            NamedColor::BrightBlue => NamedColor::Blue,
            NamedColor::BrightMagenta => NamedColor::Magenta,
            NamedColor::BrightCyan => NamedColor::Cyan,
            NamedColor::BrightWhite => NamedColor::White,
            NamedColor::BrightForeground => NamedColor::Foreground,
            val => val,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Named(NamedColor),
    Spec(Rgb),
    Indexed(u8),
}

/// Identifiers which can be assigned to a graphic character set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharsetIndex {
    /// Default set, is designated as ASCII at startup.
    G0,
    G1,
    G2,
    G3,
}

impl Default for CharsetIndex {
    fn default() -> Self {
        CharsetIndex::G0
    }
}

/// Describes shape of cursor.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash, Deserialize)]
pub enum CursorStyle {
    /// Cursor is a block like `▒`.
    Block,

    /// Cursor is an underscore like `_`.
    Underline,

    /// Cursor is a vertical bar `⎸`.
    Beam,

    /// Cursor is a box like `☐`.
    #[serde(skip)]
    HollowBlock,

    /// Invisible cursor.
    #[serde(skip)]
    Hidden,
}

impl Default for CursorStyle {
    fn default() -> CursorStyle {
        CursorStyle::Block
    }
}
