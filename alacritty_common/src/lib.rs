//! A set of functions that are shared and can be used to extend alacritty.
// This has been created so that other modules/extensions can depend on
// alacritty_terminal utilities without having to redefine them.

pub mod index;
pub mod renderer;

pub use crate::index::*;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::min;
use std::fmt;
use std::str::FromStr;

// TODO: SEB: width and height should be screen_width and screen_size
/// Terminal size info.
#[derive(Default, Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
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

    /// `scale_x` Scales the value from the current display boundary to
    /// a cartesian plane from [-1.0, 1.0], where -1.0 is 0px (left-most) and
    /// 1.0 is the `display_width` parameter (right-most), i.e. 1024px.
    pub fn scale_x(&self, input_value: f32) -> f32 {
        let center_x = self.width / 2.;
        let x = self.padding_x + input_value;
        (x - center_x) / center_x
    }

    /// `scale_y` Scales the value from the current display boundary to
    /// a cartesian plane from [-1.0, 1.0], where 1.0 is 0px (top) and -1.0 is
    /// the `display_height` parameter (bottom), i.e. 768px.
    pub fn scale_y(&self, input_value: f32) -> f32 {
        let center_y = self.height / 2.;
        let y = self.height - 2. * self.padding_y - input_value;
        -(y - center_y) / center_y
    }
}

/// `Rgb` is a copy of alacritty_terminal/src/term/color.rs
#[derive(Debug, Eq, PartialEq, Copy, Clone, Default, Serialize)]
pub struct Rgb {
    // TODO: Move to alacalacritty_common
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Transform from a hex string, copy from alacritty_terminal/src/term/colors.rs
impl FromStr for Rgb {
    type Err = ();

    fn from_str(s: &str) -> ::std::result::Result<Rgb, ()> {
        let mut chars = s.chars();
        let mut rgb = Rgb::default();

        macro_rules! component {
            ($($c:ident),*) => {
                $(
                    match chars.next().and_then(|c| c.to_digit(16)) {
                        Some(val) => rgb.$c = (val as u8) << 4,
                        None => return Err(())
                    }

                    match chars.next().and_then(|c| c.to_digit(16)) {
                        Some(val) => rgb.$c |= val as u8,
                        None => return Err(())
                    }
                )*
            }
        }

        match chars.next() {
            Some('0') => {
                if chars.next() != Some('x') {
                    return Err(());
                }
            }
            Some('#') => (),
            _ => return Err(()),
        }

        component!(r, g, b);

        Ok(rgb)
    }
}

/// Deserialize an Rgb from a hex string, copy from alacritty_terminal/src/term/colors.rs
impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RgbVisitor;

        // Used for deserializing reftests
        #[derive(Deserialize)]
        struct RgbDerivedDeser {
            r: u8,
            g: u8,
            b: u8,
        }

        impl<'a> Visitor<'a> for RgbVisitor {
            type Value = Rgb;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("hex color like 0xff00ff")
            }

            fn visit_str<E>(self, value: &str) -> ::std::result::Result<Rgb, E>
            where
                E: ::serde::de::Error,
            {
                Rgb::from_str(&value[..])
                    .map_err(|_| E::custom("failed to parse rgb; expected hex color like 0xff00ff"))
            }
        }

        // Return an error if the syntax is incorrect
        let value = serde_yaml::Value::deserialize(deserializer)?;

        // Attempt to deserialize from struct form
        if let Ok(RgbDerivedDeser { r, g, b }) = RgbDerivedDeser::deserialize(value.clone()) {
            return Ok(Rgb { r, g, b });
        }

        // Deserialize from hex notation (either 0xff00ff or #ff00ff)
        match value.deserialize_str(RgbVisitor) {
            Ok(rgb) => Ok(rgb),
            Err(err) => {
                error!("Rgb::deserialize: Problem with config: {}; using color #000000", err);
                Ok(Rgb::default())
            }
        }
    }
}
