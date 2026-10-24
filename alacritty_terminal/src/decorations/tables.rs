//! Table decoration.
//!
//! Finds tables drawn with text in the visible lines, either with box drawing characters:
//!
//! ```text
//! ┌────────┬────────┐
//! │ Commit │ Change │
//! ├────────┼────────┤
//! │ abc123 │ Fix it │
//! └────────┴────────┘
//! ```
//!
//! or as plain markdown/ASCII tables (`| a | b |`, `|---|---|`, `+---+---+`), and draws a rounded
//! rectangle behind every cell, with every table in the view in a different color.

use super::nannou::{NannouVertices, new_draw};
use crate::term::SizeInfo;
use bevy::color::Color;
use lyon::math::{Box2D, point};
use lyon::path::builder::BorderRadii;
use lyon::path::{Path, Winding};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Minimum number of content lines for a block of lines to be considered a table.
const MIN_CONTENT_LINES: usize = 2;

/// Minimum number of horizontal characters for a line to be considered a rule.
const MIN_RULE_LEN: usize = 3;

/// Opacity of the fill and the outline of the cells.
const FILL_ALPHA: f32 = 0.1;
const STROKE_ALPHA: f32 = 0.55;

/// Width of the outline of the cells in pixels.
const STROKE_WEIGHT: f32 = 1.5;

/// Configuration of the table decoration.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct TablesConfig {
    /// Whether the cells of the tables in the view are drawn as rounded rectangles.
    pub enabled: bool,
}

impl Default for TablesConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// A cell of a table, in viewport lines and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableCell {
    /// First line of the cell.
    pub top: usize,
    /// Line after the last line of the cell.
    pub bottom: usize,
    /// Column of the separator on the left of the cell.
    pub left: usize,
    /// Column of the separator on the right of the cell.
    pub right: usize,
    /// Whether the line above/below the cell is a rule, the cell then extends to its center.
    pub rule_above: bool,
    pub rule_below: bool,
}

/// A table found in the visible lines.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Table {
    pub cells: Vec<TableCell>,
}

/// Vertical separators of the cells.
fn is_vertical(c: char) -> bool {
    matches!(c, '|' | '│' | '┃' | '║' | '╎' | '╏' | '┆' | '┇' | '┊' | '┋')
}

/// Characters of horizontal rules, `:` is used by markdown to align the columns.
fn is_horizontal(c: char) -> bool {
    matches!(c, '-' | '=' | ':' | '─' | '━' | '═' | '╌' | '╍' | '┄' | '┅' | '┈' | '┉')
}

/// Where rules and vertical separators meet, i.e. "+", "┼" or the corners "┌".
fn is_junction(c: char) -> bool {
    c == '+' || (('\u{2500}'..='\u{257F}').contains(&c) && !is_vertical(c) && !is_horizontal(c))
}

/// The kind of a visible line.
#[derive(Debug, Clone, PartialEq)]
enum LineKind {
    /// A horizontal rule, from its first to its last column.
    Rule {
        start: usize,
        end: usize,
    },
    /// A line of cells, with the columns of its vertical separators.
    Content {
        separators: Vec<usize>,
    },
    Other,
}

fn classify(line: &[char]) -> LineKind {
    let Some(start) = line.iter().position(|c| !c.is_whitespace()) else {
        return LineKind::Other;
    };
    let end = line.iter().rposition(|c| !c.is_whitespace()).unwrap_or(start);
    let text = &line[start..=end];

    let horizontal = text.iter().filter(|&&c| is_horizontal(c)).count();
    if horizontal >= MIN_RULE_LEN
        && text.iter().all(|&c| is_horizontal(c) || is_junction(c) || is_vertical(c))
    {
        return LineKind::Rule { start, end };
    }

    let separators: Vec<usize> =
        (start..=end).filter(|&column| is_vertical(line[column])).collect();
    if separators.len() >= 2 { LineKind::Content { separators } } else { LineKind::Other }
}

/// A table being built from consecutive lines.
#[derive(Default)]
struct TableLines {
    /// Number of vertical separators of the content lines, unknown until the first one.
    columns: Option<usize>,
    /// Column of the first separator of the first content line.
    first_column: Option<usize>,
    /// The lines of the table, with the separators of the content lines, `None` for rules.
    lines: Vec<(usize, Option<Vec<usize>>)>,
}

impl TableLines {
    /// Whether a rule from `start` to `end` belongs to the table, i.e. starts at its left border.
    fn accepts_rule(&self, start: usize, end: usize) -> bool {
        self.first_column.is_none_or(|first| start <= first && end > first)
    }

    /// Whether a content line belongs to the table, markdown tables are not always aligned so
    /// only the number of cells has to match.
    fn accepts_content(&self, separators: &[usize]) -> bool {
        self.columns.is_none_or(|columns| columns == separators.len())
    }

    fn push_content(&mut self, line: usize, separators: Vec<usize>) {
        self.columns.get_or_insert(separators.len());
        self.first_column.get_or_insert(separators[0]);
        self.lines.push((line, Some(separators)));
    }

    /// Splits the content lines into rows and the rows into cells.
    fn into_table(self) -> Option<Table> {
        let has_rule = self.lines.iter().any(|(_, separators)| separators.is_none());
        // Rules at the edges are borders, not part of the rows.
        let first = self.lines.iter().position(|(_, separators)| separators.is_some())?;
        let last = self.lines.iter().rposition(|(_, separators)| separators.is_some())?;
        let lines = &self.lines[first..=last];

        let contents: Vec<&Vec<usize>> = lines.iter().filter_map(|(_, s)| s.as_ref()).collect();
        // Without rules, lines with pipes, i.e. shell pipelines, are only a table when aligned.
        let aligned = contents.windows(2).all(|pair| pair[0] == pair[1]);
        if contents.len() < MIN_CONTENT_LINES || !(has_rule || aligned) {
            return None;
        }

        // Box drawn tables separate every row with a rule, then consecutive lines are a single
        // row with wrapped text. Otherwise, i.e. markdown with a rule below its header, every
        // line is a row.
        let inner_rules = lines.iter().filter(|(_, separators)| separators.is_none()).count();
        let rules_separate_rows = inner_rules >= 2;

        // Rows as their first line, the line after their last one and their separators.
        let mut rows: Vec<(usize, usize, &Vec<usize>)> = vec![];
        let mut row: Option<(usize, usize, &Vec<usize>)> = None;
        for (line, separators) in lines {
            match separators {
                None => rows.extend(row.take()),
                Some(separators) => match row.as_mut() {
                    Some((_, bottom, row_separators))
                        if rules_separate_rows && *row_separators == separators =>
                    {
                        *bottom = line + 1;
                    },
                    _ => {
                        rows.extend(row.take());
                        row = Some((*line, line + 1, separators));
                    },
                },
            }
        }
        rows.extend(row);

        let rules: Vec<usize> = self
            .lines
            .iter()
            .filter(|(_, separators)| separators.is_none())
            .map(|&(line, _)| line)
            .collect();
        let cells = rows
            .into_iter()
            .flat_map(|(top, bottom, separators)| {
                let rule_above = top > 0 && rules.contains(&(top - 1));
                let rule_below = rules.contains(&bottom);
                separators.windows(2).filter(|pair| pair[1] > pair[0] + 1).map(move |pair| {
                    TableCell { top, bottom, left: pair[0], right: pair[1], rule_above, rule_below }
                })
            })
            .collect();
        Some(Table { cells })
    }
}

/// Finds the tables in the visible `lines`, each made of one character per column.
pub fn find_tables(lines: &[Vec<char>]) -> Vec<Table> {
    let mut tables = vec![];
    let mut current: Option<TableLines> = None;
    for (index, line) in lines.iter().enumerate() {
        match classify(line) {
            LineKind::Rule { start, end } => match &mut current {
                Some(table) if table.accepts_rule(start, end) => table.lines.push((index, None)),
                _ => {
                    tables.extend(current.take().and_then(TableLines::into_table));
                    let mut table = TableLines::default();
                    table.lines.push((index, None));
                    current = Some(table);
                },
            },
            LineKind::Content { separators } => match &mut current {
                Some(table) if table.accepts_content(&separators) => {
                    table.push_content(index, separators)
                },
                _ => {
                    tables.extend(current.take().and_then(TableLines::into_table));
                    let mut table = TableLines::default();
                    table.push_content(index, separators);
                    current = Some(table);
                },
            },
            LineKind::Other => tables.extend(current.take().and_then(TableLines::into_table)),
        }
    }
    tables.extend(current.and_then(TableLines::into_table));
    tables
}

/// The color of the `index`th table of the view, the hues are spread by the golden ratio so that
/// every table looks different from the others.
pub fn table_color(index: usize, alpha: f32) -> Color {
    let hue = (0.55 + index as f32 * 0.618_034).fract() * 360.;
    Color::hsla(hue, 0.65, 0.6, alpha)
}

/// Generates the vertices of the rounded rectangles of the cells of the `tables`.
pub fn gen_vertices(tables: &[Table], size_info: SizeInfo) -> Vec<NannouVertices> {
    if tables.is_empty() {
        return vec![];
    }
    let draw = new_draw();
    let (cw, ch) = (size_info.cell_width, size_info.cell_height);
    // Viewport lines start at the top, decoration coordinates start at the bottom.
    let y = |line_px: f32| size_info.height - size_info.padding_y - line_px;
    for (index, table) in tables.iter().enumerate() {
        let (fill, stroke) = (table_color(index, FILL_ALPHA), table_color(index, STROKE_ALPHA));
        for cell in &table.cells {
            // Along the lines drawn by the table: from the center of the separator on the left to
            // the one on the right and between the centers of the rules above and below, or the
            // edges of the cell lines when there is no rule.
            let left = (cell.left as f32 + 0.5) * cw;
            let right = (cell.right as f32 + 0.5) * cw;
            let top_line = if cell.rule_above { cell.top as f32 - 0.5 } else { cell.top as f32 };
            let bottom_line =
                if cell.rule_below { cell.bottom as f32 + 0.5 } else { cell.bottom as f32 };
            let top = y(top_line * ch);
            let bottom = y(bottom_line * ch);
            if right <= left || top <= bottom {
                continue;
            }

            let radius = (ch * 0.4).min((right - left) / 2.).min((top - bottom) / 2.);
            let mut builder = Path::builder();
            builder.add_rounded_rectangle(
                &Box2D::new(point(left, bottom), point(right, top)),
                &BorderRadii::new(radius),
                Winding::Positive,
            );
            let path = builder.build();
            draw.path().fill().color(fill).events(path.iter());
            draw.path().stroke().weight(STROKE_WEIGHT).color(stroke).events(path.iter());
        }
    }
    super::NannouDecoration::gen_vertices_from_nannou_draw(draw, size_info)
}

/// Keeps the tables of the view and their vertices until the visible lines or size change.
#[derive(Debug, Default)]
pub struct Tables {
    lines_hash: Option<u64>,
    size_info: Option<SizeInfo>,
    tables: Vec<Table>,
    vertices: Vec<f32>,
}

impl Tables {
    /// Finds the tables in the `lines`, returns whether what is drawn changed since the last call.
    pub fn update(&mut self, lines: &[Vec<char>], size_info: SizeInfo) -> bool {
        let mut hasher = DefaultHasher::new();
        lines.hash(&mut hasher);
        let lines_hash = hasher.finish();
        if self.lines_hash == Some(lines_hash) && self.size_info == Some(size_info) {
            return false;
        }
        self.lines_hash = Some(lines_hash);
        let resized = self.size_info.replace(size_info) != Some(size_info);

        // Without tables nothing is drawn, whatever the size.
        let tables = find_tables(lines);
        if tables == self.tables && (!resized || tables.is_empty()) {
            return false;
        }
        self.vertices = gen_vertices(&tables, size_info)
            .into_iter()
            .flat_map(|vertices| vertices.vecs)
            .collect();
        self.tables = tables;
        true
    }

    /// The vertices of the rounded rectangles, xyzrgba triangle lists.
    pub fn vertices(&self) -> &[f32] {
        &self.vertices
    }

    /// Forgets the tables, returns whether some were drawn.
    pub fn clear(&mut self) -> bool {
        let had_tables = !self.tables.is_empty();
        *self = Self::default();
        had_tables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<Vec<char>> {
        text.iter().map(|line| line.chars().collect()).collect()
    }

    #[test]
    fn box_drawn_table() {
        let lines = lines(&[
            "The branch now has:",
            "┌──────────┬─────────────────┐",
            "│ Commit   │ Change          │",
            "├──────────┼─────────────────┤",
            "│ 9999be9e │ Merge upstream  │",
            "├──────────┼─────────────────┤",
            "│ 524608fc │ Port nannou     │",
            "│          │ decorations     │",
            "└──────────┴─────────────────┘",
            "Done.",
        ]);
        let tables = find_tables(&lines);
        assert_eq!(tables.len(), 1);
        let cells = &tables[0].cells;
        // Header and two rows, the last one with wrapped text.
        let cell = |top, bottom, left, right| TableCell {
            top,
            bottom,
            left,
            right,
            rule_above: true,
            rule_below: true,
        };
        assert_eq!(cells, &[
            cell(2, 3, 0, 11),
            cell(2, 3, 11, 29),
            cell(4, 5, 0, 11),
            cell(4, 5, 11, 29),
            cell(6, 8, 0, 11),
            cell(6, 8, 11, 29),
        ]);
    }

    #[test]
    fn markdown_tables() {
        let lines = lines(&[
            "| Commit | Change |",
            "|---|:---:|",
            "| abc | first |",
            "| def | second |",
            "",
            "+-----+-----+",
            "| a   | b   |",
            "| c   | d   |",
            "+-----+-----+",
        ]);
        let tables = find_tables(&lines);
        assert_eq!(tables.len(), 2);

        // Every line is a row, with its own columns since markdown tables are not aligned.
        let rows: Vec<(usize, usize)> =
            tables[0].cells.iter().map(|cell| (cell.top, cell.bottom)).collect();
        assert_eq!(rows, [(0, 1), (0, 1), (2, 3), (2, 3), (3, 4), (3, 4)]);
        assert_eq!((tables[0].cells[0].left, tables[0].cells[0].right), (0, 9));
        assert_eq!((tables[0].cells[2].left, tables[0].cells[2].right), (0, 6));

        // The header ends at the rule below it, the body rows touch each other.
        let header = tables[0].cells[0];
        assert!(!header.rule_above && header.rule_below);
        let (first, second) = (tables[0].cells[2], tables[0].cells[4]);
        assert!(first.rule_above && !first.rule_below);
        assert!(!second.rule_above && !second.rule_below);

        assert_eq!(tables[1].cells.len(), 4);
        assert_eq!(tables[1].cells[3], TableCell {
            top: 7,
            bottom: 8,
            left: 6,
            right: 12,
            rule_above: false,
            rule_below: true,
        });
    }

    #[test]
    fn not_tables() {
        let lines = lines(&[
            "cat file | grep x | wc -l",
            "------",
            "a | b",
            "x || y",
            "-->  src/main.rs:12:5",
            "   |",
            "12 |     let x = 1;",
            "   |         ^",
            "",
            "ls | sort | uniq",
            "cat a | grep b",
        ]);
        // A single line with pipes, rules without cells, rustc diagnostics with one pipe and
        // unaligned shell pipelines.
        assert_eq!(find_tables(&lines), vec![]);
    }

    #[test]
    fn cells_follow_the_table_lines() {
        let size_info = SizeInfo {
            width: 800.,
            height: 600.,
            cell_width: 10.,
            cell_height: 20.,
            padding_x: 0.,
            padding_y: 0.,
            screen_lines: 30,
            columns: 80,
        };
        let lines = lines(&["┌───┬───┐", "│ a │ b │", "├───┼───┤", "│ c │ d │", "└───┴───┘"]);
        let tables = find_tables(&lines);
        let vertices = gen_vertices(&tables[..1], size_info);
        // The first cell, between the centers of the separators and of the rules around it.
        let first_cell_fill = &vertices[0].vecs;
        let xs = first_cell_fill.chunks_exact(7).map(|vertex| vertex[0]);
        let ys = first_cell_fill.chunks_exact(7).map(|vertex| vertex[1]);
        let (min_x, max_x) = xs.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        let (min_y, max_y) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(min_x, size_info.scale_x(5.)), "{min_x}");
        assert!(close(max_x, size_info.scale_x(45.)), "{max_x}");
        assert!(close(max_y, size_info.scale_y(600. - 10.)), "{max_y}");
        assert!(close(min_y, size_info.scale_y(600. - 50.)), "{min_y}");
    }

    #[test]
    fn tables_have_different_colors() {
        let colors: Vec<_> = (0..6).map(|index| table_color(index, 1.).to_srgba()).collect();
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                let distance =
                    (a.red - b.red).abs() + (a.green - b.green).abs() + (a.blue - b.blue).abs();
                assert!(distance > 0.15, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn vertices_are_cached() {
        let size_info = SizeInfo {
            width: 800.,
            height: 600.,
            cell_width: 10.,
            cell_height: 20.,
            padding_x: 5.,
            padding_y: 5.,
            screen_lines: 29,
            columns: 79,
        };
        let lines = lines(&["| a | b |", "|---|---|", "| c | d |"]);
        let mut tables = Tables::default();
        assert!(tables.update(&lines, size_info));
        assert!(!tables.vertices().is_empty());
        assert_eq!(tables.vertices().len() % 21, 0);
        assert!(!tables.update(&lines, size_info));
        assert!(tables.update(&lines, SizeInfo { width: 900., ..size_info }));
        assert!(tables.clear());
        assert!(!tables.update(&super::tests::lines(&["no table"]), size_info));
    }
}
