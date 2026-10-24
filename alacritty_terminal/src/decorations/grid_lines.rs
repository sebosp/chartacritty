//! Grid line decoration.
//!
//! Applications like nvim draw indentation guides and borders with box drawing characters, i.e.
//! "▏ ▏ ▏" or "│", when these characters span several consecutive cells they are meant to be a
//! continuous line. This decoration finds such runs in the visible grid and draws them as nannou
//! lines, with a single star travelling from line to line.

use super::nannou::{NannouVertices, new_draw};
use crate::term::SizeInfo;
use bevy::color::Color;
use bevy::math::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f32::consts::TAU;
use vte::ansi::Rgb;

/// Minimum number of consecutive cells for a run of line characters to be considered a line.
const MIN_RUN_LEN: usize = 2;

/// Width of the drawn lines in pixels.
const LINE_WEIGHT: f32 = 2.;

/// Opacity of the lines, they are drawn over their characters as a faint highlight.
const LINE_ALPHA: f32 = 0.35;

/// Speed of the star travelling along the lines in cells per second.
const STAR_CELLS_PER_SEC: f32 = 4.;

/// Configuration of the grid lines decoration.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct GridLinesConfig {
    /// Whether vertical/horizontal runs of line characters are drawn as nannou lines.
    pub enabled: bool,
}

impl Default for GridLinesConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Orientation of a run of line characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

/// Where, across the cell, a line character draws its line, as a fraction of the cell size.
fn vertical_offset(c: char) -> Option<f32> {
    match c {
        // Left one eighth block, used by indent-blankline and friends.
        '▏' => Some(1. / 16.),
        // Right one eighth block.
        '▕' => Some(15. / 16.),
        // Box drawing characters with a vertical arm, these are centered.
        '│' | '┃' | '║' | '╎' | '╏' | '┆' | '┇' | '┊' | '┋' | '├' | '┤' | '┼' | '┬' | '┴' | '┌'
        | '┐' | '└' | '┘' | '╭' | '╮' | '╰' | '╯' | '╠' | '╣' | '╬' | '╦' | '╩' | '╔' | '╗'
        | '╚' | '╝' => Some(0.5),
        _ => None,
    }
}

/// Where, along the cell height, a line character draws its horizontal line.
fn horizontal_offset(c: char) -> Option<f32> {
    match c {
        '─' | '━' | '═' | '╌' | '╍' | '┄' | '┅' | '┈' | '┉' | '├' | '┤' | '┼' | '┬' | '┴' | '┌'
        | '┐' | '└' | '┘' | '╭' | '╮' | '╰' | '╯' | '╠' | '╣' | '╬' | '╦' | '╩' | '╔' | '╗'
        | '╚' | '╝' => Some(0.5),
        _ => None,
    }
}

/// Whether the character may be part of a grid line.
pub fn is_line_char(c: char) -> bool {
    vertical_offset(c).is_some() || horizontal_offset(c).is_some()
}

/// A run of consecutive line characters in the viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLine {
    pub orientation: Orientation,
    /// Column of a vertical line or viewport line of a horizontal line.
    pub fixed: usize,
    /// First cell of the run along the line.
    pub start: usize,
    /// Number of cells in the run.
    pub len: usize,
    /// Position of the line across the cell, as a fraction of the cell size.
    pub offset: f32,
    /// Foreground color of the characters forming the line.
    pub color: Rgb,
}

/// A line character in the viewport: `(viewport line, column, character, foreground color)`.
pub type LineCell = (usize, usize, char, Rgb);

/// Finds the runs of at least `MIN_RUN_LEN` vertically or horizontally consecutive line
/// characters of the same color.
pub fn find_grid_lines(cells: &[LineCell]) -> Vec<GridLine> {
    // Keyed by column for vertical lines and by line for horizontal lines, the values are the
    // positions along the line, kept sorted by the BTreeMap.
    let mut vertical: BTreeMap<usize, BTreeMap<usize, (f32, Rgb)>> = BTreeMap::new();
    let mut horizontal: BTreeMap<usize, BTreeMap<usize, (f32, Rgb)>> = BTreeMap::new();
    for &(line, column, c, color) in cells {
        if let Some(offset) = vertical_offset(c) {
            vertical.entry(column).or_default().insert(line, (offset, color));
        }
        if let Some(offset) = horizontal_offset(c) {
            horizontal.entry(line).or_default().insert(column, (offset, color));
        }
    }

    let mut res = vec![];
    for (orientation, groups) in
        [(Orientation::Vertical, vertical), (Orientation::Horizontal, horizontal)]
    {
        for (fixed, positions) in groups {
            let mut run: Option<GridLine> = None;
            for (pos, (offset, color)) in positions {
                match &mut run {
                    // Extend the run while the cells are consecutive and the line is drawn at the
                    // same position across the cell with the same color, i.e. nvim highlights the
                    // indent guide of the cursor scope with its own color.
                    Some(line)
                        if line.start + line.len == pos
                            && line.offset == offset
                            && line.color == color =>
                    {
                        line.len += 1;
                    },
                    _ => {
                        if let Some(line) = run.take().filter(|line| line.len >= MIN_RUN_LEN) {
                            res.push(line);
                        }
                        run = Some(GridLine {
                            orientation,
                            fixed,
                            start: pos,
                            len: 1,
                            offset,
                            color,
                        });
                    },
                }
            }
            if let Some(line) = run.filter(|line| line.len >= MIN_RUN_LEN) {
                res.push(line);
            }
        }
    }
    res
}

impl GridLine {
    /// Start and end points of the line in decoration coordinates: pixels from the left of the
    /// grid and from the bottom of the window, as expected by `SizeInfo::scale_x/scale_y`.
    fn endpoints(&self, size_info: SizeInfo) -> (Vec2, Vec2) {
        let (cw, ch) = (size_info.cell_width, size_info.cell_height);
        // Viewport lines start at the top, decoration coordinates start at the bottom.
        let y = |line_px: f32| size_info.height - size_info.padding_y - line_px;
        match self.orientation {
            Orientation::Vertical => {
                let x = (self.fixed as f32 + self.offset) * cw;
                let top = y(self.start as f32 * ch);
                let bottom = y((self.start + self.len) as f32 * ch);
                (Vec2::new(x, top), Vec2::new(x, bottom))
            },
            Orientation::Horizontal => {
                let line_y = y((self.fixed as f32 + self.offset) * ch);
                let left = self.start as f32 * cw;
                let right = (self.start + self.len) as f32 * cw;
                (Vec2::new(left, line_y), Vec2::new(right, line_y))
            },
        }
    }

    /// Whether both lines are at the same place, regardless of their color.
    fn same_place(&self, other: &GridLine) -> bool {
        self.orientation == other.orientation
            && self.fixed == other.fixed
            && self.start == other.start
            && self.len == other.len
            && self.offset == other.offset
    }

    /// Seconds the star needs to travel the line.
    fn travel_secs(&self) -> f32 {
        self.len as f32 / STAR_CELLS_PER_SEC
    }
}

/// The trip of the star along one line.
#[derive(Debug, Clone, Copy)]
struct StarTrip {
    line: GridLine,
    /// Whether the star travels from the end of the line to its start.
    reverse: bool,
    /// Time at which the star started the trip, in seconds.
    started: f32,
}

/// A single star that travels along a random grid line and, once it reaches the end, jumps to
/// another random line.
#[derive(Debug, Default)]
pub struct GridLineStar {
    trip: Option<StarTrip>,
}

impl GridLineStar {
    /// Returns the position of the star at `time` and its progress through the trip, starting a
    /// trip on a new random line when the current one is done or the line is no longer visible.
    fn position(&mut self, lines: &[GridLine], size_info: SizeInfo, time: f32) -> (Vec2, f32) {
        let trip = match self.trip {
            Some(trip)
                if time >= trip.started
                    && time - trip.started < trip.line.travel_secs()
                    && lines.iter().any(|line| line.same_place(&trip.line)) =>
            {
                trip
            },
            previous => {
                // Prefer a different line than the one just travelled.
                let candidates: Vec<&GridLine> = lines
                    .iter()
                    .filter(|line| previous.is_none_or(|trip| !line.same_place(&trip.line)))
                    .collect();
                let line = if candidates.is_empty() {
                    lines[rand::random_range(0..lines.len())]
                } else {
                    *candidates[rand::random_range(0..candidates.len())]
                };
                let trip = StarTrip { line, reverse: rand::random(), started: time };
                self.trip = Some(trip);
                trip
            },
        };

        let (mut start, mut end) = trip.line.endpoints(size_info);
        if trip.reverse {
            std::mem::swap(&mut start, &mut end);
        }
        let progress = ((time - trip.started) / trip.line.travel_secs()).clamp(0., 1.);
        (start.lerp(end, progress), progress)
    }
}

/// Generates the vertices for the grid lines: faint lines in the color of their characters and a
/// single star travelling along one of them, `time` is in seconds.
pub fn gen_vertices(
    lines: &[GridLine],
    star: &mut GridLineStar,
    size_info: SizeInfo,
    time: f32,
) -> Vec<NannouVertices> {
    if lines.is_empty() {
        star.trip = None;
        return vec![];
    }
    let draw = new_draw();
    for line in lines {
        let (start, end) = line.endpoints(size_info);
        let Rgb { r, g, b } = line.color;
        let color = Color::srgba_u8(r, g, b, (LINE_ALPHA * 255.) as u8);
        draw.line().start(start).end(end).weight(LINE_WEIGHT).color(color);
    }

    let (center, progress) = star.position(lines, size_info, time);
    let radius = (size_info.cell_width.min(size_info.cell_height) * 0.15).max(1.5);
    // Fade in and out at the ends of the trip so that the jump between lines is not abrupt.
    let fade = (progress * (1. - progress) * 8.).min(1.);
    // Twinkle: the glow and the rays pulse while the star travels.
    let twinkle = 0.5 + 0.5 * (time * 3. * TAU / 2.).sin();
    draw.ellipse().xy(center).radius(radius * (2. + twinkle)).color(Color::srgba(
        1.,
        0.85,
        0.6,
        (0.08 + 0.12 * twinkle) * fade,
    ));
    let ray = radius * (2. + 2. * twinkle);
    let ray_color = Color::srgba(1., 0.95, 0.85, (0.25 + 0.35 * twinkle) * fade);
    draw.line()
        .start(center - Vec2::new(ray, 0.))
        .end(center + Vec2::new(ray, 0.))
        .weight(1.)
        .color(ray_color);
    draw.line()
        .start(center - Vec2::new(0., ray))
        .end(center + Vec2::new(0., ray))
        .weight(1.)
        .color(ray_color);
    draw.ellipse().xy(center).radius(radius).color(Color::srgba(1., 1., 0.9, 0.95 * fade));
    super::NannouDecoration::gen_vertices_from_nannou_draw(draw, size_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUIDE: Rgb = Rgb { r: 0x3b, g: 0x42, b: 0x61 };
    const SCOPE: Rgb = Rgb { r: 0x7a, g: 0xa2, b: 0xf7 };

    fn size_info() -> SizeInfo {
        SizeInfo {
            width: 800.,
            height: 600.,
            cell_width: 10.,
            cell_height: 20.,
            padding_x: 5.,
            padding_y: 5.,
            screen_lines: 29,
            columns: 79,
        }
    }

    #[test]
    fn finds_vertical_and_horizontal_runs() {
        let cells = vec![
            // Indent guide over three lines at column 2, interrupted at line 3.
            (0, 2, '▏', GUIDE),
            (1, 2, '▏', GUIDE),
            (2, 2, '▏', GUIDE),
            (4, 2, '▏', GUIDE),
            // A single guide is not a line.
            (0, 6, '▏', GUIDE),
            // A border: ┌──┐
            (5, 0, '┌', GUIDE),
            (5, 1, '─', GUIDE),
            (5, 2, '─', GUIDE),
            (5, 3, '┐', GUIDE),
            (6, 0, '│', GUIDE),
            (6, 3, '│', GUIDE),
        ];
        let lines = find_grid_lines(&cells);
        let vertical: Vec<_> =
            lines.iter().filter(|l| l.orientation == Orientation::Vertical).collect();
        let horizontal: Vec<_> =
            lines.iter().filter(|l| l.orientation == Orientation::Horizontal).collect();

        assert_eq!(vertical.len(), 3);
        assert!(vertical.contains(&&GridLine {
            orientation: Orientation::Vertical,
            fixed: 2,
            start: 0,
            len: 3,
            offset: 1. / 16.,
            color: GUIDE,
        }));
        // The corners join the borders.
        assert!(vertical.iter().any(|l| l.fixed == 0 && l.start == 5 && l.len == 2));
        assert!(vertical.iter().any(|l| l.fixed == 3 && l.start == 5 && l.len == 2));
        assert_eq!(horizontal, vec![&GridLine {
            orientation: Orientation::Horizontal,
            fixed: 5,
            start: 0,
            len: 4,
            offset: 0.5,
            color: GUIDE,
        }]);
    }

    #[test]
    fn gutter_line() {
        // bat/less style line numbers, the empty lines must not interrupt the line.
        let text = ["  41 │", "  42 │ # TODO", "  43 │", "  44 │ ## Tauri", "  45 │"];
        let cells: Vec<_> = text
            .iter()
            .enumerate()
            .flat_map(|(line, text)| {
                text.chars()
                    .enumerate()
                    .filter(|(_, c)| is_line_char(*c))
                    .map(move |(column, c)| (line, column, c, GUIDE))
            })
            .collect();
        assert_eq!(find_grid_lines(&cells), vec![GridLine {
            orientation: Orientation::Vertical,
            fixed: 5,
            start: 0,
            len: 5,
            offset: 0.5,
            color: GUIDE,
        }]);
    }

    #[test]
    fn splits_runs_by_color() {
        // The cursor scope guide is highlighted from line 2 on.
        let cells: Vec<_> =
            (0..5).map(|line| (line, 4, '▏', if line < 2 { GUIDE } else { SCOPE })).collect();
        let lines = find_grid_lines(&cells);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().any(|l| l.start == 0 && l.len == 2 && l.color == GUIDE));
        assert!(lines.iter().any(|l| l.start == 2 && l.len == 3 && l.color == SCOPE));
    }

    #[test]
    fn gen_vertices_for_lines() {
        let size_info = size_info();
        let lines = find_grid_lines(&[(0, 2, '▏', GUIDE), (1, 2, '▏', GUIDE), (2, 2, '▏', GUIDE)]);
        let mut star = GridLineStar::default();
        let vertices = gen_vertices(&lines, &mut star, size_info, 1.25);
        // Line, glow, two rays and the star core.
        assert_eq!(vertices.len(), 5);
        let line = &vertices[0].vecs;
        assert!(!line.is_empty() && line.len() % 21 == 0);
        let faint_guide = [
            GUIDE.r as f32 / 255.,
            GUIDE.g as f32 / 255.,
            GUIDE.b as f32 / 255.,
            (LINE_ALPHA * 255.) as u8 as f32 / 255.,
        ];
        for vertex in line.chunks_exact(7) {
            // Faint line in the color of the guide characters.
            assert!(
                vertex[3..].iter().zip(faint_guide).all(|(c, e)| (c - e).abs() < 1e-3),
                "{vertex:?}"
            );
            // Around column 2, in the top three lines of the window.
            assert!((vertex[0] - size_info.scale_x(20.625)).abs() < 0.01, "{vertex:?}");
            assert!(vertex[1] > size_info.scale_y(600. - 5. - 61.), "{vertex:?}");
        }
    }

    #[test]
    fn star_travels_a_line_and_jumps_to_another() {
        let size_info = size_info();
        let cells: Vec<_> =
            (0..4).flat_map(|line| [(line, 2, '▏', GUIDE), (line, 10, '▏', GUIDE)]).collect();
        let lines = find_grid_lines(&cells);
        assert_eq!(lines.len(), 2);

        let mut star = GridLineStar::default();
        let (_, progress) = star.position(&lines, size_info, 10.);
        assert_eq!(progress, 0.);
        let first = star.trip.unwrap().line;

        // Half way through the trip the star stays on the same line.
        let half = first.travel_secs() / 2.;
        let (_, progress) = star.position(&lines, size_info, 10. + half);
        assert!((progress - 0.5).abs() < 1e-4);
        assert!(star.trip.unwrap().line.same_place(&first));

        // Once done, it jumps to the other line.
        star.position(&lines, size_info, 10. + first.travel_secs());
        let second = star.trip.unwrap().line;
        assert!(!second.same_place(&first));

        // When its line disappears, it jumps to a visible one.
        star.position(&[first], size_info, 10. + first.travel_secs() + 0.1);
        assert!(star.trip.unwrap().line.same_place(&first));
    }
}
