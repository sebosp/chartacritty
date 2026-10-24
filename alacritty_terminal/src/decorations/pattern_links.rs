//! Pattern links decoration.
//!
//! Connects the lines of every repeated text pattern, see `repeated_text`, with very faint lines
//! in the color of the pattern. Every pattern has a vertical trunk in the left padding of the
//! window and a branch along the bottom of each of its lines, between the lines so that it does
//! not cross any text, up to the end of the repeated text. Brighter sections travel back and
//! forth along the links, from one line of the pattern to the next.

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

/// Opacity and width of the sections travelling along the links, and of their glow.
const PULSE_ALPHA: f32 = 0.45;
const PULSE_WEIGHT: f32 = 1.5;
const GLOW_ALPHA: f32 = 0.1;
const GLOW_WEIGHT: f32 = 4.;

/// Length of the travelling sections and their speed, in cells and cells per second.
const PULSE_CELLS: f32 = 3.;
const PULSE_CELLS_PER_SEC: f32 = 8.;

/// Number of travelling sections per pattern.
const PULSES: usize = 2;

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

impl PatternLinks {
    /// The route from the first line of the pattern to the last one, through the trunk.
    fn route(&self) -> Vec<Vec2> {
        let mut route = vec![];
        let last = self.branches.len() - 1;
        for (index, &(y, end)) in self.branches.iter().enumerate() {
            if index > 0 {
                route.push(Vec2::new(self.trunk_x, y));
            }
            route.push(Vec2::new(end, y));
            if index < last {
                route.push(Vec2::new(self.trunk_x, y));
            }
        }
        route
    }
}

/// Places the links of the patterns with at least two lines.
///
/// Patterns whose lines overlap vertically get their own lanes for their trunks, from the left
/// edge of the window towards the text.
pub fn layout(instances: &[PatternInstance], size_info: SizeInfo) -> Vec<PatternLinks> {
    let (cw, ch) = (size_info.cell_width, size_info.cell_height);
    let (padding_x, padding_y) = (size_info.padding_x, size_info.padding_y);

    let mut patterns: BTreeMap<u64, Vec<&PatternInstance>> = BTreeMap::new();
    for instance in instances {
        patterns.entry(instance.pattern).or_default().push(instance);
    }

    // Branches of patterns, ordered by their first line.
    let mut branches: Vec<(u64, Vec<(f32, f32)>)> = patterns
        .into_iter()
        .filter(|(_, instances)| instances.len() >= 2)
        .map(|(pattern, mut instances)| {
            instances.sort_by_key(|instance| instance.line);
            let branches = instances
                .iter()
                .map(|instance| {
                    // Between the line and the next one, under the repeated text.
                    let y = padding_y + (instance.line + 1) as f32 * ch;
                    (y, padding_x + instance.end as f32 * cw)
                })
                .collect();
            (pattern, branches)
        })
        .collect();
    branches.sort_by(|a, b| a.1[0].0.total_cmp(&b.1[0].0));

    // Greedy assignment of lanes to the vertical extents of the trunks.
    let mut lanes_bottom: Vec<f32> = vec![];
    let lanes: Vec<usize> = branches
        .iter()
        .map(|(_, branches)| {
            let (top, bottom) = (branches[0].0, branches[branches.len() - 1].0);
            match lanes_bottom.iter().position(|&lane_bottom| lane_bottom < top) {
                Some(lane) => {
                    lanes_bottom[lane] = bottom;
                    lane
                },
                None => {
                    lanes_bottom.push(bottom);
                    lanes_bottom.len() - 1
                },
            }
        })
        .collect();
    let spacing =
        (padding_x / (lanes_bottom.len() + 1) as f32).clamp(MIN_LANE_SPACING, MAX_LANE_SPACING);

    branches
        .into_iter()
        .zip(lanes)
        .map(|((pattern, branches), lane)| PatternLinks {
            pattern,
            trunk_x: spacing * (lane + 1) as f32,
            branches,
        })
        .collect()
}

/// Length of a polyline.
fn length(points: &[Vec2]) -> f32 {
    points.windows(2).map(|pair| pair[0].distance(pair[1])).sum()
}

/// The part of the polyline between the distances `from` and `to` along it.
fn sub_polyline(points: &[Vec2], from: f32, to: f32) -> Vec<Vec2> {
    let mut res = vec![];
    let mut travelled = 0.;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let segment = a.distance(b);
        let (start, end) = (travelled, travelled + segment);
        travelled = end;
        if segment == 0. || end < from || start > to {
            continue;
        }
        let at = |distance: f32| a.lerp(b, ((distance - start) / segment).clamp(0., 1.));
        if res.is_empty() {
            res.push(at(from));
        }
        res.push(at(to));
    }
    res.dedup();
    res
}

/// Generates the vertices of the links, `color` gives the color of every pattern, `hovered` is
/// the pattern under the mouse and `time` is in seconds.
pub fn gen_vertices(
    links: &[PatternLinks],
    size_info: SizeInfo,
    time: f32,
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
        let color = |alpha: f32| Color::srgba_u8(r, g, b, (alpha * 255.) as u8);
        let (alpha, weight) = if hovered == Some(links.pattern) {
            (HOVERED_ALPHA, HOVERED_WEIGHT)
        } else {
            (BASE_ALPHA, BASE_WEIGHT)
        };

        // The trunk and the branches.
        let (top, bottom) = (links.branches[0].0, links.branches[links.branches.len() - 1].0);
        let trunk = polyline(vec![Vec2::new(links.trunk_x, top), Vec2::new(links.trunk_x, bottom)]);
        draw.path().stroke().weight(weight).color(color(alpha)).points(trunk);
        for &(y, end) in &links.branches {
            let branch = polyline(vec![Vec2::new(links.trunk_x, y), Vec2::new(end, y)]);
            draw.path().stroke().weight(weight).color(color(alpha)).points(branch);
        }

        // Sections going back and forth along the route between the lines of the pattern.
        let route = links.route();
        let route_length = length(&route);
        let pulse = (PULSE_CELLS * size_info.cell_width).min(route_length);
        let travel = route_length - pulse;
        let period = (travel / (PULSE_CELLS_PER_SEC * size_info.cell_width)).max(0.5);
        let phase = (links.pattern % 1000) as f32 / 1000.;
        for index in 0..PULSES {
            let t = (time / (2. * period) + phase + index as f32 / PULSES as f32).fract();
            let from = travel * (1. - (2. * t - 1.).abs());
            let section = polyline(sub_polyline(&route, from, from + pulse));
            if section.len() < 2 {
                continue;
            }
            draw.path()
                .stroke()
                .weight(GLOW_WEIGHT)
                .color(color(GLOW_ALPHA))
                .points(section.clone());
            draw.path().stroke().weight(PULSE_WEIGHT).color(color(PULSE_ALPHA)).points(section);
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
    fn route_goes_through_every_line() {
        let links = PatternLinks {
            pattern: 1,
            trunk_x: 5.,
            branches: vec![(10., 50.), (30., 60.), (50., 40.)],
        };
        assert_eq!(links.route(), [
            Vec2::new(50., 10.),
            Vec2::new(5., 10.),
            Vec2::new(5., 30.),
            Vec2::new(60., 30.),
            Vec2::new(5., 30.),
            Vec2::new(5., 50.),
            Vec2::new(40., 50.),
        ]);
    }

    #[test]
    fn sub_polylines() {
        let points = [Vec2::new(0., 0.), Vec2::new(10., 0.), Vec2::new(10., 10.)];
        assert_eq!(length(&points), 20.);
        assert_eq!(sub_polyline(&points, 5., 15.), [
            Vec2::new(5., 0.),
            Vec2::new(10., 0.),
            Vec2::new(10., 5.)
        ]);
        assert_eq!(sub_polyline(&points, 12., 14.), [Vec2::new(10., 2.), Vec2::new(10., 4.)]);
    }

    #[test]
    fn vertices_in_the_pattern_color() {
        let links = layout(&[instance(0, 0, 9), instance(2, 0, 9)], size_info());
        let vertices = gen_vertices(&links, size_info(), 1.5, None, |_| [255, 0, 0]);
        // The trunk, two branches and the glow and center of two travelling sections.
        assert_eq!(vertices.len(), 7);
        for vertex in vertices.iter().flat_map(|vertices| vertices.vecs.chunks_exact(7)) {
            assert!((vertex[3] - 1.).abs() < 1e-3 && vertex[4] < 1e-3, "{vertex:?}");
            assert!(vertex[6] <= PULSE_ALPHA + 1e-2, "{vertex:?}");
        }

        // The links of the pattern under the mouse stand out.
        let max_alpha = |vertices: &[NannouVertices]| {
            vertices[0].vecs.chunks_exact(7).map(|vertex| vertex[6]).fold(0., f32::max)
        };
        let hovered = gen_vertices(&links, size_info(), 1.5, Some(9), |_| [255, 0, 0]);
        assert!(max_alpha(&hovered) > max_alpha(&vertices) * 2.);
    }
}
