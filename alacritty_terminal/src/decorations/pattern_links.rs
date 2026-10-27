//! Pattern links decoration.
//!
//! Connects the lines of every repeated text pattern, see `repeated_text`, with very faint lines
//! in the color of the pattern. Every pattern has a vertical trunk in the padding of the window
//! and a branch along the bottom of each of its lines, between the lines so that it does not
//! cross any text. Patterns on the left half of the window have their trunk in the left padding
//! and branches up to the end of the repeated text, patterns on the right half have their trunk
//! in the right padding and branches up to the start of the repeated text, so that the links do
//! not cross the whole window.

use super::nannou::{NannouVertices, new_draw};
use super::repeated_text::{PatternInstance, PatternSpan};
use crate::term::SizeInfo;
use bevy::color::Color;
use bevy::math::Vec2;
use lyon::math::{Box2D, point};
use lyon::path::builder::BorderRadii;
use lyon::path::{Path, Winding};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Opacity and width of the links.
const BASE_ALPHA: f32 = 0.12;
const BASE_WEIGHT: f32 = 1.;

/// Opacity and width of the links of the pattern under the mouse.
const HOVERED_ALPHA: f32 = 0.6;
const HOVERED_WEIGHT: f32 = 1.5;

/// Opacity and width of the outlines of the repeated text of the pattern under the mouse.
const OUTLINE_ALPHA: f32 = 0.85;
const OUTLINE_WEIGHT: f32 = 1.5;

/// Space between the trunks of the patterns in pixels.
const MIN_LANE_SPACING: f32 = 2.;
const MAX_LANE_SPACING: f32 = 6.;

/// Configuration of the pattern links decoration.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct PatternLinksConfig {
    /// Whether the lines of the repeated text patterns are connected.
    pub enabled: bool,
}

impl Default for PatternLinksConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// The links of a pattern, in window pixels from its top left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternLinks {
    pub pattern: u64,
    /// Horizontal position of the trunk.
    pub trunk_x: f32,
    /// For every line of the pattern, the height of its branch and where the branch ends.
    pub branches: Vec<(f32, f32)>,
}

/// Assigns lanes to the vertical extents of trunks, ordered by their top, so that trunks that
/// overlap vertically are in different lanes. Returns the lane of every trunk and the number of
/// lanes.
fn assign_lanes(extents: &[(f32, f32)]) -> (Vec<usize>, usize) {
    let mut lanes_bottom: Vec<f32> = vec![];
    let lanes = extents
        .iter()
        .map(|&(top, bottom)| match lanes_bottom.iter().position(|&lane| lane < top) {
            Some(lane) => {
                lanes_bottom[lane] = bottom;
                lane
            },
            None => {
                lanes_bottom.push(bottom);
                lanes_bottom.len() - 1
            },
        })
        .collect();
    (lanes, lanes_bottom.len())
}

/// Places the links of the patterns with at least two lines.
///
/// Patterns whose lines overlap vertically get their own lanes for their trunks, from the edge
/// of the window towards the text.
pub fn layout(instances: &[PatternInstance], size_info: SizeInfo) -> Vec<PatternLinks> {
    let (cw, ch) = (size_info.cell_width, size_info.cell_height);
    let (padding_x, padding_y) = (size_info.padding_x, size_info.padding_y);
    let half_columns = size_info.columns as f32 / 2.;

    let mut patterns: BTreeMap<u64, Vec<&PatternInstance>> = BTreeMap::new();
    for instance in instances {
        patterns.entry(instance.pattern).or_default().push(instance);
    }

    // Branches of the patterns on each side, ordered by their first line.
    let (mut left, mut right): (Vec<_>, Vec<_>) = (vec![], vec![]);
    for (pattern, mut instances) in patterns {
        if instances.len() < 2 {
            continue;
        }
        instances.sort_by_key(|instance| instance.line);
        let center = instances
            .iter()
            .map(|instance| (instance.start + instance.end) as f32 / 2.)
            .sum::<f32>()
            / instances.len() as f32;
        let on_right = center > half_columns;
        let branches: Vec<(f32, f32)> = instances
            .iter()
            .map(|instance| {
                // Between the line and the next one, under the repeated text.
                let y = padding_y + (instance.line + 1) as f32 * ch;
                let column = if on_right { instance.start } else { instance.end };
                (y, padding_x + column as f32 * cw)
            })
            .collect();
        if on_right { right.push((pattern, branches)) } else { left.push((pattern, branches)) }
    }

    let grid_right = padding_x + size_info.columns as f32 * cw;
    let right_padding = (size_info.width - grid_right).max(0.);
    let mut res = vec![];
    for (mut side, on_right) in [(left, false), (right, true)] {
        side.sort_by(|a, b| a.1[0].0.total_cmp(&b.1[0].0));
        let extents: Vec<(f32, f32)> = side
            .iter()
            .map(|(_, branches)| (branches[0].0, branches[branches.len() - 1].0))
            .collect();
        let (lanes, count) = assign_lanes(&extents);
        let padding = if on_right { right_padding } else { padding_x };
        let spacing = (padding / (count + 1) as f32).clamp(MIN_LANE_SPACING, MAX_LANE_SPACING);
        for ((pattern, branches), lane) in side.into_iter().zip(lanes) {
            let offset = spacing * (lane + 1) as f32;
            let trunk_x = if on_right { size_info.width - offset } else { offset };
            res.push(PatternLinks { pattern, trunk_x, branches });
        }
    }
    res
}

/// Generates the vertices of the links, `color` gives the color of every pattern and `hovered`
/// is the pattern under the mouse.
pub fn gen_vertices(
    links: &[PatternLinks],
    size_info: SizeInfo,
    hovered: Option<u64>,
    color: impl Fn(u64) -> [u8; 3],
) -> Vec<NannouVertices> {
    if links.is_empty() {
        return vec![];
    }

    // From window pixels to decoration coordinates: pixels from the left of the grid and from
    // the bottom of the window, as expected by `SizeInfo::scale_x/scale_y`.
    let to_decoration =
        |point: Vec2| Vec2::new(point.x - size_info.padding_x, size_info.height - point.y);
    let polyline = |points: Vec<Vec2>| points.into_iter().map(to_decoration).collect::<Vec<_>>();

    let draw = new_draw();
    for links in links {
        let [r, g, b] = color(links.pattern);
        let (alpha, weight) = if hovered == Some(links.pattern) {
            (HOVERED_ALPHA, HOVERED_WEIGHT)
        } else {
            (BASE_ALPHA, BASE_WEIGHT)
        };
        let color = Color::srgba_u8(r, g, b, (alpha * 255.) as u8);

        // The trunk and the branches.
        let (top, bottom) = (links.branches[0].0, links.branches[links.branches.len() - 1].0);
        let trunk = polyline(vec![Vec2::new(links.trunk_x, top), Vec2::new(links.trunk_x, bottom)]);
        draw.path().stroke().weight(weight).color(color).points(trunk);
        for &(y, end) in &links.branches {
            let branch = polyline(vec![Vec2::new(links.trunk_x, y), Vec2::new(end, y)]);
            draw.path().stroke().weight(weight).color(color).points(branch);
        }
    }
    super::NannouDecoration::gen_vertices_from_nannou_draw(draw, size_info)
}

/// Generates solid outlines, clear inside, around the `spans` of repeated text, i.e. those of
/// the pattern under the mouse, in the color given by `color`.
pub fn gen_outline_vertices(
    spans: &[PatternSpan],
    size_info: SizeInfo,
    color: impl Fn(u64) -> [u8; 3],
) -> Vec<NannouVertices> {
    if spans.is_empty() {
        return vec![];
    }
    let (cw, ch) = (size_info.cell_width, size_info.cell_height);
    // Viewport lines start at the top, decoration coordinates start at the bottom.
    let y = |line: f32| size_info.height - size_info.padding_y - line * ch;

    let draw = new_draw();
    for span in spans {
        let [r, g, b] = color(span.pattern);
        let left = (span.start as f32 - 0.25).max(0.) * cw;
        let right = (span.end as f32 + 0.25) * cw;
        let (top, bottom) = (y(span.line as f32), y((span.line + 1) as f32));
        let radius = (ch * 0.3).min((right - left) / 2.);
        let mut builder = Path::builder();
        builder.add_rounded_rectangle(
            &Box2D::new(point(left, bottom), point(right, top)),
            &BorderRadii::new(radius),
            Winding::Positive,
        );
        let path = builder.build();
        draw.path()
            .stroke()
            .weight(OUTLINE_WEIGHT)
            .color(Color::srgba_u8(r, g, b, (OUTLINE_ALPHA * 255.) as u8))
            .events(path.iter());
    }
    super::NannouDecoration::gen_vertices_from_nannou_draw(draw, size_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size_info() -> SizeInfo {
        SizeInfo {
            width: 800.,
            height: 600.,
            cell_width: 10.,
            cell_height: 20.,
            padding_x: 20.,
            padding_y: 4.,
            screen_lines: 29,
            columns: 76,
        }
    }

    fn instance(line: usize, start: usize, pattern: u64) -> PatternInstance {
        PatternInstance { line, start, end: start + 8, pattern }
    }

    #[test]
    fn branches_run_under_the_repeated_text() {
        let instances = [instance(1, 0, 7), instance(3, 0, 7), instance(5, 4, 7)];
        let links = layout(&instances, size_info());
        assert_eq!(links.len(), 1);
        let (cw, ch, pad_x, pad_y) = (10., 20., 20., 4.);
        // Between each line and the next one, up to the end of the repeated text.
        assert_eq!(links[0].branches, [
            (pad_y + 2. * ch, pad_x + 8. * cw),
            (pad_y + 4. * ch, pad_x + 8. * cw),
            (pad_y + 6. * ch, pad_x + 12. * cw),
        ]);
        assert!(links[0].trunk_x < 20.);
    }

    #[test]
    fn overlapping_patterns_get_their_own_lanes() {
        let instances = [
            instance(1, 0, 1),
            instance(5, 0, 1),
            // Overlaps the first pattern.
            instance(3, 0, 2),
            instance(7, 0, 2),
            // After the first pattern, re-uses its lane.
            instance(9, 0, 3),
            instance(11, 0, 3),
            // A single line is not linked.
            instance(10, 0, 4),
        ];
        let links = layout(&instances, size_info());
        let trunks: Vec<(u64, f32)> = links.iter().map(|l| (l.pattern, l.trunk_x)).collect();
        assert_eq!(trunks.len(), 3);
        assert_ne!(trunks[0].1, trunks[1].1);
        assert_eq!(trunks[0].1, trunks[2].1);
        assert!(trunks.iter().all(|&(_, x)| x > 0. && x < 20.));
    }

    #[test]
    fn patterns_on_the_right_link_from_the_right() {
        // Centered after the middle of the 76 columns.
        let instances =
            [instance(2, 50, 5), instance(6, 52, 5), instance(1, 0, 6), instance(7, 0, 6)];
        let links = layout(&instances, size_info());
        let right = links.iter().find(|links| links.pattern == 5).unwrap();
        let (cw, ch, pad_x, pad_y) = (10., 20., 20., 4.);
        // In the right padding, after the end of the grid at 780 pixels.
        assert!(right.trunk_x > 780. && right.trunk_x < 800.);
        // Up to the start of the repeated text.
        assert_eq!(right.branches, [
            (pad_y + 3. * ch, pad_x + 50. * cw),
            (pad_y + 7. * ch, pad_x + 52. * cw)
        ]);
        // Overlapping patterns on different sides may share their lane.
        let left = links.iter().find(|links| links.pattern == 6).unwrap();
        assert!(left.trunk_x < 20.);
        assert_eq!(left.trunk_x, 800. - right.trunk_x);
    }

    #[test]
    fn vertices_in_the_pattern_color() {
        let links = layout(&[instance(0, 0, 9), instance(2, 0, 9)], size_info());
        let vertices = gen_vertices(&links, size_info(), None, |_| [255, 0, 0]);
        // The trunk and two branches.
        assert_eq!(vertices.len(), 3);
        for vertex in vertices.iter().flat_map(|vertices| vertices.vecs.chunks_exact(7)) {
            assert!((vertex[3] - 1.).abs() < 1e-3 && vertex[4] < 1e-3, "{vertex:?}");
            assert!(vertex[6] <= BASE_ALPHA + 1e-2, "{vertex:?}");
        }

        // The links of the pattern under the mouse stand out.
        let max_alpha = |vertices: &[NannouVertices]| {
            vertices[0].vecs.chunks_exact(7).map(|vertex| vertex[6]).fold(0., f32::max)
        };
        let hovered = gen_vertices(&links, size_info(), Some(9), |_| [255, 0, 0]);
        assert!(max_alpha(&hovered) > max_alpha(&vertices) * 2.);
    }
}
