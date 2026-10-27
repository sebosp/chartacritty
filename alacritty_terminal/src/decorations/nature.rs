//! Nature decoration: faint grass along the bottom of the window and a big origami bonsai, a red
//! maple made of triangles, in the background.
//!
//! Both move with the same wind, which blows as the characters and colors of the screen change:
//! the more cells change, the stronger the wind. While the terminal is idle the leaves of the
//! bonsai grow, when it gets busy they shake and fall to the ground, and once it calms down again
//! they grow back.

use super::repeated_text::CellColor;
use crate::term::SizeInfo;
use bevy::math::Vec2;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Decay of the activity of the screen, in seconds.
const ACTIVITY_SECS: f32 = 2.5;

/// How much wind the activity makes, the activity is the share of the cells of the screen that
/// changed, summed over the frames and decaying with `ACTIVITY_SECS`.
const WIND_GAIN: f32 = 0.6;

/// How fast the wind rises and calms down, in seconds.
const WIND_RISE_SECS: f32 = 0.5;
const WIND_CALM_SECS: f32 = 4.;

/// Below this wind the leaves grow, above `WIND_STRESS` they start to break off.
const WIND_CALM: f32 = 0.2;
const WIND_STRESS: f32 = 0.45;

/// Seconds for a leaf to grow fully and for a fallen leaf to fade away.
const GROW_SECS: f32 = 40.;
const GROUND_SECS: f32 = 30.;

/// Horizontal space between grass blades in pixels.
const BLADE_SPACING: f32 = 3.;

/// Segments of a grass blade.
const BLADE_SEGMENTS: usize = 4;

/// Depth of the branches of the bonsai.
const BRANCH_DEPTH: usize = 5;

/// Leaves of the foliage pad at the tip of every branch.
const PAD_LEAVES: usize = 9;

/// Opacities of the parts of the nature decoration, the bonsai is only drawn with the borders of
/// its triangles.
const GRASS_ALPHA: f32 = 0.14;
const TRUNK_ALPHA: f32 = 0.16;
const POT_ALPHA: f32 = 0.12;
const LEAF_ALPHA: f32 = 0.28;

/// Width of the borders of the bonsai in pixels.
const BORDER_WIDTH: f32 = 1.;

/// Colors of a red maple.
const MAPLE: [[f32; 3]; 6] = [
    [0.70, 0.11, 0.11],
    [0.80, 0.20, 0.13],
    [0.86, 0.30, 0.12],
    [0.62, 0.07, 0.16],
    [0.90, 0.42, 0.16],
    [0.75, 0.15, 0.20],
];
const BARK: [f32; 3] = [0.45, 0.30, 0.22];
const POT: [f32; 3] = [0.30, 0.34, 0.40];

/// Configuration of the nature decoration.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct NatureConfig {
    /// Whether the grass is drawn along the bottom of the window.
    pub grass: bool,
    /// Height of the grass in lines.
    pub grass_lines: f32,
    /// Whether the bonsai is drawn.
    pub bonsai: bool,
    /// Horizontal position of the bonsai, from 0 at the left of the window to 1 at its right.
    pub bonsai_x: f32,
    /// Height of the bonsai, as a share of the height of the window.
    pub bonsai_height: f32,
    /// Multiplies the opacity of everything.
    pub alpha: f32,
}

impl Default for NatureConfig {
    fn default() -> Self {
        Self {
            grass: true,
            grass_lines: 1.2,
            bonsai: true,
            bonsai_x: 0.72,
            bonsai_height: 0.6,
            alpha: 1.,
        }
    }
}

/// A small deterministic random number generator, splitmix64, so that the bonsai and the grass
/// keep their shape.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// Uniform in `[min, max)`.
    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (self.next() >> 40) as f32 / (1u64 << 24) as f32 * (max - min)
    }
}

/// The share of the cells whose character or color differs between two screens, 0 when their
/// size differs.
pub fn changed_fraction(
    previous: (&[Vec<char>], &[Vec<CellColor>]),
    current: (&[Vec<char>], &[Vec<CellColor>]),
) -> f32 {
    let ((prev_lines, prev_colors), (lines, colors)) = (previous, current);
    if prev_lines.len() != lines.len() || prev_colors.len() != colors.len() {
        return 0.;
    }
    let (mut changed, mut total) = (0usize, 0usize);
    for (i, line) in lines.iter().enumerate() {
        let prev = &prev_lines[i];
        if prev.len() != line.len() {
            return 0.;
        }
        let (color, prev_color) = (colors.get(i), prev_colors.get(i));
        for (column, c) in line.iter().enumerate() {
            let color_changed = color.and_then(|colors| colors.get(column))
                != prev_color.and_then(|colors| colors.get(column));
            changed += usize::from(*c != prev[column] || color_changed);
        }
        total += line.len();
    }
    if total == 0 { 0. } else { changed as f32 / total as f32 }
}

/// The wind, blowing as the screen changes.
#[derive(Debug, Default)]
pub struct Wind {
    activity: f32,
    strength: f32,
    previous: Option<(Vec<Vec<char>>, Vec<Vec<CellColor>>)>,
}

impl Wind {
    /// Compares the screen with the previous one.
    pub fn observe(&mut self, lines: &[Vec<char>], colors: &[Vec<CellColor>]) {
        if let Some((prev_lines, prev_colors)) = &self.previous {
            self.activity += changed_fraction((prev_lines, prev_colors), (lines, colors));
        }
        match &mut self.previous {
            Some((prev_lines, prev_colors)) if prev_lines.len() == lines.len() => {
                prev_lines.clone_from_slice(lines);
                prev_colors.clear();
                prev_colors.extend_from_slice(colors);
            },
            previous => *previous = Some((lines.to_vec(), colors.to_vec())),
        }
    }

    /// Moves the wind `dt` seconds forward.
    fn step(&mut self, dt: f32) {
        self.activity *= (-dt / ACTIVITY_SECS).exp();
        let target = 1. - (-self.activity * WIND_GAIN).exp();
        let secs = if target > self.strength { WIND_RISE_SECS } else { WIND_CALM_SECS };
        self.strength += (target - self.strength) * (1. - (-dt / secs).exp());
    }

    /// Strength of the wind from 0, still, to 1.
    pub fn strength(&self) -> f32 {
        self.strength
    }
}

/// Gusts of wind travelling from the left to the right of the window, from 0 to 1 at the
/// horizontal position `x` in pixels.
fn gust(time: f32, x: f32) -> f32 {
    let wave = (time * 0.9 - x * 0.006).sin() * (0.6 + 0.4 * (time * 0.31 + 1.7).sin());
    0.5 + 0.5 * wave
}

/// The direction of `angle`, in radians counter clockwise from the right.
fn dir(angle: f32) -> Vec2 {
    Vec2::new(angle.cos(), angle.sin())
}

/// Triangles in window pixels, from the bottom left corner of the window, converted to the
/// xyzrgba vertices of the nannou renderer.
pub(crate) struct Triangles<'a> {
    size_info: &'a SizeInfo,
    alpha: f32,
    pub(crate) vertices: Vec<f32>,
}

impl<'a> Triangles<'a> {
    /// Triangles whose opacity is multiplied by `alpha`.
    pub(crate) fn new(size_info: &'a SizeInfo, alpha: f32) -> Self {
        Self { size_info, alpha, vertices: vec![] }
    }

    pub(crate) fn push(&mut self, points: [Vec2; 3], [r, g, b]: [f32; 3], alpha: f32) {
        let alpha = (alpha * self.alpha).clamp(0., 1.);
        for point in points {
            self.vertices.extend_from_slice(&[
                self.size_info.scale_x(point.x - self.size_info.padding_x),
                self.size_info.scale_y(point.y),
                0.,
                r.min(1.),
                g.min(1.),
                b.min(1.),
                alpha,
            ]);
        }
    }

    /// An axis aligned rectangle from `min` to `max`.
    pub(crate) fn rect(&mut self, min: Vec2, max: Vec2, color: [f32; 3], alpha: f32) {
        let (top_left, bottom_right) = (Vec2::new(min.x, max.y), Vec2::new(max.x, min.y));
        self.push([min, bottom_right, max], color, alpha);
        self.push([min, max, top_left], color, alpha);
    }

    /// A line from `a` to `b`, `width` pixels wide.
    pub(crate) fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: [f32; 3], alpha: f32) {
        let offset = (b - a).perp().normalize_or_zero() * width / 2.;
        self.push([a + offset, a - offset, b + offset], color, alpha);
        self.push([b + offset, a - offset, b - offset], color, alpha);
    }

    /// The borders of a polygon.
    fn outline(&mut self, points: &[Vec2], color: [f32; 3], alpha: f32) {
        for (i, &point) in points.iter().enumerate() {
            let next = points[(i + 1) % points.len()];
            self.line(point, next, BORDER_WIDTH, color, alpha);
        }
    }

    /// The borders of a branch from `a` with width `wa` to `b` with width `wb`, folded along its
    /// middle like origami, with one side lighter than the other.
    fn folded_branch(&mut self, a: Vec2, b: Vec2, wa: f32, wb: f32, color: [f32; 3], alpha: f32) {
        let normal = (b - a).perp().normalize_or_zero();
        let (la, ra) = (a + normal * wa / 2., a - normal * wa / 2.);
        let (lb, rb) = (b + normal * wb / 2., b - normal * wb / 2.);
        self.line(la, lb, BORDER_WIDTH, color, alpha);
        self.line(ra, rb, BORDER_WIDTH, color.map(|c| c * 0.7), alpha);
        // The fold, thin twigs are only their fold.
        self.line(a, b, BORDER_WIDTH, color.map(|c| c * 0.85), alpha * 0.6);
    }
}

#[derive(Debug, Clone)]
struct Blade {
    x: f32,
    height: f32,
    width: f32,
    lean: f32,
    phase: f32,
    color: [f32; 3],
    alpha: f32,
}

/// Grass along the bottom of the window.
#[derive(Debug, Clone)]
struct Grass {
    width: f32,
    height: f32,
    blades: Vec<Blade>,
}

impl Grass {
    fn new(width: f32, height: f32) -> Self {
        let mut rng = Rng(0x6772617373);
        let blades = (0..(width / BLADE_SPACING) as usize)
            .map(|i| Blade {
                x: (i as f32 + rng.range(0., 1.)) * BLADE_SPACING,
                height: height * rng.range(0.35, 1.),
                width: rng.range(1.5, 3.),
                lean: rng.range(-0.25, 0.25),
                phase: rng.range(0., TAU),
                color: [rng.range(0.22, 0.4), rng.range(0.45, 0.65), rng.range(0.14, 0.26)],
                alpha: GRASS_ALPHA * rng.range(0.6, 1.2),
            })
            .collect();
        Self { width, height, blades }
    }

    fn draw(&self, triangles: &mut Triangles<'_>, time: f32, wind: f32) {
        for blade in &self.blades {
            // Bent along the wind, swaying faster as the wind gets stronger.
            let sway = (time * (1.3 + 2.5 * wind) + blade.phase).sin() * (0.04 + 0.12 * wind);
            let bend = blade.lean + wind * (0.5 + 0.6 * gust(time, blade.x)) + sway;
            let step = blade.height / BLADE_SEGMENTS as f32;
            let mut point = Vec2::new(blade.x, 0.);
            for segment in 0..BLADE_SEGMENTS {
                let (t0, t1) = (
                    segment as f32 / BLADE_SEGMENTS as f32,
                    (segment + 1) as f32 / BLADE_SEGMENTS as f32,
                );
                // The tip bends more than the base, angles from the vertical.
                let angle = bend * t1.powf(1.5);
                let next = point + Vec2::new(angle.sin(), angle.cos()) * step;
                let shade = blade.color.map(|c| c * (0.7 + 0.4 * t0));
                let (w0, w1) = (blade.width * (1. - t0), blade.width * (1. - t1));
                let normal = (next - point).perp().normalize_or_zero();
                let (l0, r0) = (point + normal * w0 / 2., point - normal * w0 / 2.);
                let (l1, r1) = (next + normal * w1 / 2., next - normal * w1 / 2.);
                triangles.push([l0, r0, l1], shade, blade.alpha);
                if w1 > 0. {
                    triangles.push([l1, r0, r1], shade, blade.alpha);
                }
                point = next;
            }
        }
    }
}

/// Where a leaf grows on the bonsai.
#[derive(Debug, Clone)]
struct LeafSlot {
    /// The vertex of the leaf tied to the branch.
    anchor: Vec2,
    angle: f32,
    size: f32,
    color: [f32; 3],
    /// From 0 to 1, offsets the movements and the timings of the leaf.
    phase: f32,
    /// How much wind the leaf takes before it breaks off.
    strength: f32,
}

#[derive(Debug, Clone, PartialEq)]
enum LeafState {
    Attached { growth: f32, stress: f32 },
    Falling { position: Vec2, velocity: Vec2, angle: f32, growth: f32 },
    Ground { position: Vec2, angle: f32, growth: f32, age: f32 },
    Gone { since: f32 },
}

#[derive(Debug, Clone)]
struct Leaf {
    slot: LeafSlot,
    state: LeafState,
}

impl Leaf {
    /// The angle of the attached leaf: very slowly rotating around its anchor, fluttering in the
    /// wind.
    fn attached_angle(&self, time: f32, wind: f32) -> f32 {
        let slot = &self.slot;
        let rotation = 0.35 * (time * 0.03 + slot.phase * TAU).sin();
        let flutter = (time * (2. + 5. * wind) + slot.phase * 50.).sin()
            * wind
            * (0.15 + 0.45 * gust(time, slot.anchor.x));
        slot.angle + rotation + flutter
    }

    fn step(&mut self, dt: f32, time: f32, wind: f32, ground: f32) {
        let slot = &self.slot;
        self.state = match self.state.clone() {
            LeafState::Attached { mut growth, mut stress } => {
                if wind < WIND_CALM {
                    growth = (growth + dt / GROW_SECS).min(1.);
                }
                if wind > WIND_STRESS {
                    stress += dt * (wind - WIND_STRESS) * (0.5 + gust(time, slot.anchor.x));
                } else {
                    stress = (stress - dt * 0.1).max(0.);
                }
                if stress > slot.strength && growth > 0.25 {
                    let angle = self.attached_angle(time, wind);
                    let velocity = Vec2::ZERO;
                    LeafState::Falling { position: slot.anchor, velocity, angle, growth }
                } else {
                    LeafState::Attached { growth, stress }
                }
            },
            LeafState::Falling { mut position, mut velocity, mut angle, growth } => {
                // Carried by the wind, swinging from side to side while falling.
                let push = wind * 70. * (0.4 + gust(time, position.x));
                let swing = (time * 1.7 + slot.phase * TAU).sin() * 12.;
                velocity.x += (push + swing - velocity.x) * (dt * 1.5).min(1.);
                velocity.y = -(18. + 10. * (time * 2.3 + slot.phase * 9.).sin());
                position += velocity * dt;
                angle += (slot.phase - 0.5) * 3. * dt;
                let rest = ground + slot.phase * 5.;
                if position.y <= rest {
                    position.y = rest;
                    LeafState::Ground { position, angle, growth, age: 0. }
                } else {
                    LeafState::Falling { position, velocity, angle, growth }
                }
            },
            LeafState::Ground { mut position, angle, growth, age } => {
                position.x += wind * 8. * dt;
                let age = age + dt;
                if age > GROUND_SECS {
                    LeafState::Gone { since: time }
                } else {
                    LeafState::Ground { position, angle, growth, age }
                }
            },
            // New leaves bud once it is calm again, not all at once.
            LeafState::Gone { since } if wind < WIND_CALM && time > since + slot.phase * 20. => {
                LeafState::Attached { growth: dt / GROW_SECS, stress: 0. }
            },
            state @ LeafState::Gone { .. } => state,
        };
    }

    fn draw(&self, triangles: &mut Triangles<'_>, time: f32, wind: f32) {
        let slot = &self.slot;
        let (anchor, angle, growth, fade) = match self.state {
            LeafState::Attached { growth, .. } => {
                (slot.anchor, self.attached_angle(time, wind), growth, 1.)
            },
            LeafState::Falling { position, angle, growth, .. } => (position, angle, growth, 1.),
            LeafState::Ground { position, angle, growth, age } => {
                (position, angle, growth, 1. - age / GROUND_SECS)
            },
            LeafState::Gone { .. } => return,
        };
        let size = slot.size * growth;
        if size < 0.5 {
            return;
        }

        // Now and then a leaf catches the light.
        let shine = (time * 0.17 + slot.phase * 40.).sin().max(0.).powi(24);
        let color = slot.color.map(|c| c * (1. + 0.8 * shine));

        // A single triangle tied to the branch by one vertex, folded along its middle.
        let tip = anchor + dir(angle) * size;
        let left = anchor + dir(angle + 0.55) * size * 0.62;
        let right = anchor + dir(angle - 0.55) * size * 0.62;
        let alpha = LEAF_ALPHA * fade * (1. + 0.5 * shine);
        triangles.outline(&[anchor, left, tip, right], color, alpha);
        triangles.line(anchor, tip, BORDER_WIDTH, color.map(|c| c * 0.7), alpha * 0.7);
    }
}

#[derive(Debug, Clone)]
struct Branch {
    a: Vec2,
    b: Vec2,
    wa: f32,
    wb: f32,
}

/// An origami bonsai in its pot.
#[derive(Debug, Clone)]
struct Bonsai {
    branches: Vec<Branch>,
    pot: Vec<([Vec2; 3], f32)>,
    leaves: Vec<Leaf>,
}

impl Bonsai {
    /// A bonsai standing on the bottom of the window at `x`, `height` pixels tall.
    fn new(x: f32, height: f32) -> Self {
        let mut rng = Rng(0x626f6e736169);
        let mut bonsai = Self { branches: vec![], pot: vec![], leaves: vec![] };

        // A wide and shallow pot, folded in facets.
        let (pot_width, pot_height) = (height * 0.42, height * 0.07);
        let (bottom, top) = (Vec2::new(x, 0.), Vec2::new(x, pot_height));
        let corners = [
            bottom - Vec2::X * pot_width * 0.42,
            bottom + Vec2::X * pot_width * 0.42,
            top + Vec2::X * pot_width / 2.,
            top - Vec2::X * pot_width / 2.,
        ];
        let center = (bottom + top) / 2.;
        for (i, shade) in [(0, 0.8), (1, 1.), (2, 0.6), (3, 0.9)] {
            bonsai.pot.push(([corners[i], corners[(i + 1) % 4], center], shade));
        }

        // A thick trunk leaning a bit, with branches spreading wide like a bonsai.
        let trunk = height * 0.22;
        let angle = FRAC_PI_2 + rng.range(-0.25, 0.25);
        bonsai.grow(&mut rng, top, angle, trunk, height * 0.075, 0, height * 0.034);
        bonsai
    }

    #[allow(clippy::too_many_arguments)]
    fn grow(
        &mut self,
        rng: &mut Rng,
        start: Vec2,
        mut angle: f32,
        length: f32,
        width: f32,
        depth: usize,
        leaf_size: f32,
    ) {
        // A slightly curved branch.
        let mut point = start;
        let end_width = width * 0.65;
        for segment in 0..3 {
            angle += rng.range(-0.18, 0.18);
            let next = point + dir(angle) * length / 3.;
            let (t0, t1) = (segment as f32 / 3., (segment + 1) as f32 / 3.);
            let (wa, wb) = (width + (end_width - width) * t0, width + (end_width - width) * t1);
            self.branches.push(Branch { a: point, b: next, wa, wb });
            point = next;
        }

        if depth == BRANCH_DEPTH {
            self.foliage_pad(rng, point, leaf_size);
            return;
        }

        let children = if depth >= 1 && rng.range(0., 1.) < 0.3 { 3 } else { 2 };
        for child in 0..children {
            let side = if child % 2 == 0 { 1. } else { -1. };
            let mut child_angle = angle + side * rng.range(0.3, 0.75);
            // Bonsai branches spread out sideways.
            if depth >= 2 {
                let sideways = if dir(child_angle).x >= 0. { 0.25 } else { PI - 0.25 };
                child_angle += (sideways - child_angle) * 0.25;
            }
            let child_length = length * rng.range(0.62, 0.8);
            self.grow(
                rng,
                point,
                child_angle,
                child_length,
                end_width * 0.75,
                depth + 1,
                leaf_size,
            );
        }

        // Some leaves along the outer branches too.
        if depth >= 3 {
            self.foliage_pad(rng, point, leaf_size * 0.8);
        }
    }

    /// A cloud of leaves around the tip of a branch, flatter than tall like bonsai foliage.
    fn foliage_pad(&mut self, rng: &mut Rng, tip: Vec2, leaf_size: f32) {
        for _ in 0..PAD_LEAVES {
            let around = rng.range(0., TAU);
            let radius = rng.range(0., leaf_size * 1.4);
            let anchor = tip + Vec2::new(around.cos() * radius * 1.5, around.sin() * radius * 0.6);
            // Pointing away from the tip, mostly upwards.
            let outward = (anchor - tip).y.atan2((anchor - tip).x);
            let angle = outward + (FRAC_PI_2 - outward) * 0.4 + rng.range(-0.3, 0.3);
            let color = MAPLE[(rng.next() % MAPLE.len() as u64) as usize];
            self.branches.push(Branch { a: tip, b: anchor, wa: 1., wb: 0.6 });
            self.leaves.push(Leaf {
                slot: LeafSlot {
                    anchor,
                    angle,
                    size: leaf_size * rng.range(0.75, 1.25),
                    color,
                    phase: rng.range(0., 1.),
                    strength: rng.range(0.3, 2.5),
                },
                state: LeafState::Attached { growth: rng.range(0.6, 1.), stress: 0. },
            });
        }
    }

    fn draw(&self, triangles: &mut Triangles<'_>, time: f32, wind: f32) {
        for &(points, shade) in &self.pot {
            triangles.outline(&points, POT.map(|c| c * shade), POT_ALPHA);
        }
        for branch in &self.branches {
            if branch.wa <= BORDER_WIDTH * 2. {
                triangles.line(branch.a, branch.b, BORDER_WIDTH, BARK, TRUNK_ALPHA * 0.6);
            } else {
                triangles.folded_branch(
                    branch.a,
                    branch.b,
                    branch.wa,
                    branch.wb,
                    BARK,
                    TRUNK_ALPHA,
                );
            }
        }
        for leaf in &self.leaves {
            leaf.draw(triangles, time, wind);
        }
    }
}

/// The state of the nature decoration.
#[derive(Debug, Default)]
pub struct Nature {
    wind: Wind,
    last_time: Option<f32>,
    /// The size and configuration the grass and the bonsai were made for.
    made_for: Option<(SizeInfo, NatureConfig)>,
    grass: Option<Grass>,
    bonsai: Option<Bonsai>,
}

impl Nature {
    /// Compares the screen with the previous one, the changes make the wind blow.
    pub fn observe(&mut self, lines: &[Vec<char>], colors: &[Vec<CellColor>]) {
        self.wind.observe(lines, colors);
    }

    /// Strength of the wind from 0 to 1.
    pub fn wind(&self) -> f32 {
        self.wind.strength()
    }

    /// Moves the grass and the bonsai to `time` in seconds and returns their vertices.
    pub fn frame(&mut self, config: &NatureConfig, size_info: SizeInfo, time: f32) -> Vec<f32> {
        let dt = self.last_time.map_or(0., |last| (time - last).clamp(0., 0.5));
        self.last_time = Some(time);
        self.wind.step(dt);
        let wind = self.wind.strength();

        if self.made_for.as_ref() != Some(&(size_info, config.clone())) {
            self.made_for = Some((size_info, config.clone()));
            let grass_height = size_info.cell_height * config.grass_lines;
            self.grass = config.grass.then(|| Grass::new(size_info.width, grass_height));
            self.bonsai = config.bonsai.then(|| {
                Bonsai::new(
                    size_info.width * config.bonsai_x,
                    size_info.height * config.bonsai_height,
                )
            });
        }

        let mut triangles = Triangles::new(&size_info, config.alpha);
        if let Some(bonsai) = &mut self.bonsai {
            let ground = self.grass.as_ref().map_or(2., |grass| grass.height * 0.2);
            for leaf in &mut bonsai.leaves {
                leaf.step(dt, time, wind, ground);
            }
            bonsai.draw(&mut triangles, time, wind);
        }
        if let Some(grass) = &self.grass {
            if grass.width == size_info.width {
                grass.draw(&mut triangles, time, wind);
            }
        }
        triangles.vertices
    }
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

    fn screen(text: &[&str]) -> Vec<Vec<char>> {
        text.iter().map(|line| line.chars().collect()).collect()
    }

    #[test]
    fn changed_cells() {
        let before = screen(&["abcd", "efgh"]);
        let after = screen(&["abcd", "efXY"]);
        let colors = vec![vec![(0, 0, 0); 4]; 2];
        assert_eq!(changed_fraction((&before, &colors), (&after, &colors)), 0.25);

        // A change of color is a change too.
        let mut recolored = colors.clone();
        recolored[0][0] = (255, 0, 0);
        assert_eq!(changed_fraction((&before, &colors), (&before, &recolored)), 0.125);

        // Resizes are not changes.
        assert_eq!(changed_fraction((&before, &colors), (&after[..1], &colors[..1])), 0.);
    }

    #[test]
    fn wind_rises_with_changes_and_calms_down() {
        let mut wind = Wind::default();
        let colors = vec![vec![(0, 0, 0); 4]; 2];
        let (a, b) = (screen(&["abcd", "efgh"]), screen(&["ijkl", "mnop"]));
        wind.observe(&a, &colors);
        wind.step(0.1);
        assert_eq!(wind.strength(), 0.);

        // The whole screen changing on every frame.
        for frame in 0..30 {
            wind.observe(if frame % 2 == 0 { &b } else { &a }, &colors);
            wind.step(1. / 30.);
        }
        let strong = wind.strength();
        assert!(strong > 0.5, "{strong}");

        // Idle.
        for _ in 0..300 {
            wind.observe(&a, &colors);
            wind.step(0.1);
        }
        assert!(wind.strength() < 0.05, "{}", wind.strength());
    }

    #[test]
    fn leaves_fall_in_the_wind_and_grow_back_when_calm() {
        let mut bonsai = Bonsai::new(500., 360.);
        assert!(bonsai.leaves.len() > 50);
        let attached = |bonsai: &Bonsai| {
            bonsai.leaves.iter().filter(|l| matches!(l.state, LeafState::Attached { .. })).count()
        };
        let all = bonsai.leaves.len();
        assert_eq!(attached(&bonsai), all);

        // A storm.
        let mut time = 0.;
        for _ in 0..200 {
            time += 0.1;
            bonsai.leaves.iter_mut().for_each(|leaf| leaf.step(0.1, time, 1., 2.));
        }
        assert!(attached(&bonsai) < all / 2, "{}", attached(&bonsai));
        assert!(bonsai.leaves.iter().any(|l| matches!(l.state, LeafState::Ground { .. })));

        // A long calm.
        for _ in 0..1000 {
            time += 0.1;
            bonsai.leaves.iter_mut().for_each(|leaf| leaf.step(0.1, time, 0., 2.));
        }
        assert_eq!(attached(&bonsai), all);
        assert!(bonsai.leaves.iter().all(|leaf| match leaf.state {
            LeafState::Attached { growth, .. } => growth > 0.,
            _ => false,
        }));
    }

    #[test]
    fn faint_triangles_in_the_window() {
        let mut nature = Nature::default();
        let vertices = nature.frame(&NatureConfig::default(), size_info(), 1.);
        assert!(!vertices.is_empty());
        assert_eq!(vertices.len() % 21, 0);
        for vertex in vertices.chunks_exact(7) {
            assert!(vertex[0].abs() <= 1.5 && vertex[1].abs() <= 1.5, "{vertex:?}");
            assert!(vertex[6] <= 0.5, "{vertex:?}");
        }

        // Nothing when both parts are disabled.
        let config = NatureConfig { grass: false, bonsai: false, ..Default::default() };
        assert!(nature.frame(&config, size_info(), 2.).is_empty());
    }
}
