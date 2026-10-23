//! Nannou-based decorations for Alacritty

use super::PolarClockState;
use super::moon_phase::MoonPhaseState;
use crate::charts::deserialize_rgb_from_str;
use crate::term::SizeInfo;
use bevy::color::{Color, LinearRgba};
use bevy::ecs::entity::Entity;
use bevy::math::Vec2;
use bevy::mesh::Mesh;
use chrono::prelude::*;
use lyon::tessellation::{FillTessellator, StrokeTessellator};
use nannou_draw::draw::mesh::MeshExt;
use nannou_draw::draw::primitive::Primitive;
use nannou_draw::draw::render::{RenderContext, RenderPrimitive};
use nannou_draw::draw::{Draw, DrawCommand, DrawContext};
use nannou_draw::text::font::{NannouTextCxInner, SharedTextCx};
use parley::{FontContext, LayoutContext};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, Mutex};
use vte::ansi::Rgb;

/// Text context shared by every decoration `Draw`, no text is drawn but `Draw::new` requires one.
static TEXT_CX: LazyLock<SharedTextCx> = LazyLock::new(|| {
    SharedTextCx(Arc::new(Mutex::new(NannouTextCxInner {
        font: FontContext::default(),
        layout: LayoutContext::new(),
    })))
});

/// Creates a nannou `Draw` that is not attached to any bevy window, its commands are tessellated
/// by `NannouDecoration::gen_vertices_from_nannou_draw`.
pub fn new_draw() -> Draw {
    Draw::new(Entity::PLACEHOLDER, TEXT_CX.clone())
}

/// Converts a palette sRGB color and an alpha into a bevy color for nannou drawing.
pub fn to_bevy_color(color: palette::Srgb, alpha: f32) -> Color {
    Color::srgba(color.red, color.green, color.blue, alpha)
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum NannouDrawArrayMode {
    Points,
    #[default]
    LineStrip,
    LineLoop,
    GlTriangles,
}

impl From<&Primitive> for NannouDrawArrayMode {
    fn from(src: &Primitive) -> Self {
        match src {
            Primitive::Mesh(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Tri(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Polygon(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Ellipse(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Quad(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Rect(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Line(_) => NannouDrawArrayMode::LineStrip,
            Primitive::Text(_) => NannouDrawArrayMode::GlTriangles,
            Primitive::Path(_) => NannouDrawArrayMode::LineStrip,
            _ => NannouDrawArrayMode::LineStrip,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct NannouVertices {
    #[serde(default)]
    pub draw_array_mode: NannouDrawArrayMode,
    #[serde(default)]
    pub vecs: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct NannouDecoration {
    #[serde(deserialize_with = "deserialize_rgb_from_str", default)]
    pub color: Rgb,
    pub alpha: f32,
    #[serde(default)]
    pub size_info: SizeInfo,
    pub radius: f32,
    #[serde(default)]
    pub polar_clock: PolarClockState,
    #[serde(default)]
    pub moon_state: MoonPhaseState,
    #[serde(default)]
    pub vertices: Vec<NannouVertices>,
    #[serde(default = "local_now")]
    pub now: DateTime<Local>,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// The last time the decoration was drawn.
    #[serde(default)]
    pub last_drawn_msecs: f32,
}

fn local_now() -> DateTime<Local> {
    Local::now()
}

impl NannouDecoration {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        let coords = super::gen_hex_grid_positions(size_info, radius);
        let center_idx = super::find_hexagon_grid_center_idx(&coords, size_info, radius);
        let coord = coords[center_idx];
        // self.alpha = 0.7f32;
        // Store the center hexagon position for re-use later.
        let now = Local::now();
        // TODO: Read the config and if props are provided give them below as param
        let polar_clock = PolarClockState::new(None);
        Self {
            color,
            alpha,
            size_info,
            radius,
            polar_clock,
            moon_state: MoonPhaseState::new(radius),
            vertices: Default::default(),
            now,
            last_drawn_msecs: 0f32,
            x: coord.x,
            y: coord.y,
        }
    }

    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        let coords = super::gen_hex_grid_positions(size_info, self.radius);
        let center_idx = super::find_hexagon_grid_center_idx(&coords, size_info, self.radius);
        let coord = coords[center_idx];
        self.x = coord.x;
        self.y = coord.y;
        self.size_info = size_info;
        let now = Local::now();
        self.polar_clock.mark_as_dirty();
        self.polar_clock.tick(&now, self.x, self.y, self.radius, size_info, self.alpha);
        self.moon_state.mark_as_dirty();
        self.moon_state.tick(self.x, self.y, self.radius, size_info);
        self.update_opengl_vecs();
    }

    /// This is called regularly to potentially update the decoration vertices.
    pub fn tick(&mut self, time: f32) {
        self.now = Local::now();
        self.polar_clock.tick(&self.now, self.x, self.y, self.radius, self.size_info, self.alpha);
        self.moon_state.tick(self.x, self.y, self.radius, self.size_info);
        self.last_drawn_msecs = time;
        self.update_opengl_vecs();
    }

    /// Called after instantiation of the NannouDecoration, it will initialize the vertices for the
    /// decorations.
    pub fn update_opengl_vecs(&mut self) {
        // tracing::info!("NannouDecoration::update_opengl_vecs(size_info) {:?}, center_idx: {}, x:
        // {}, y:{}, radius: {}, coords: {:?}", self.size_info, center_idx, coord.x, coord.y,
        // self.radius, coords);
        self.vertices = self.gen_vertices();
    }

    // Transforms nannou::draw::Draw into xyzrgba vertices we can draw through our renderer.
    // Adapted from nannou_draw `render.rs` `update_draw_mesh()`, but each primitive is tessellated
    // into its own bevy Mesh instead of being uploaded through the bevy render app.
    pub fn gen_vertices_from_nannou_draw(draw: Draw, size_info: SizeInfo) -> Vec<NannouVertices> {
        let mut res = vec![];
        let draw_cmds: Vec<_> = draw.drain_commands().collect();
        let Ok(draw_state) = draw.state.read() else {
            return res;
        };
        let Ok(intermediary_state) = draw_state.try_read_intermediary_state() else {
            return res;
        };
        let mut fill_tessellator = FillTessellator::new();
        let mut stroke_tessellator = StrokeTessellator::new();
        // Keep track of context changes.
        let mut curr_ctxt = DrawContext::default();
        for cmd in draw_cmds {
            match cmd {
                DrawCommand::Context(ctxt) => curr_ctxt = ctxt,
                DrawCommand::Primitive(prim) => {
                    // Info required during rendering.
                    let ctxt = RenderContext {
                        intermediary_mesh: &intermediary_state.intermediary_mesh,
                        path_event_buffer: &intermediary_state.path_event_buffer,
                        path_points_vertex_buffer: &intermediary_state.path_points_vertex_buffer,
                        text_buffer: &intermediary_state.text_buffer,
                        theme: draw_state.theme(),
                        transform: &curr_ctxt.transform,
                        fill_tessellator: &mut fill_tessellator,
                        stroke_tessellator: &mut stroke_tessellator,
                        output_attachment_size: Vec2::new(size_info.width, size_info.height),
                        output_attachment_scale_factor: 1.,
                    };
                    let draw_array_mode = NannouDrawArrayMode::from(&prim);
                    let mut mesh = Mesh::init();
                    prim.render_primitive(ctxt, &mut mesh);
                    let vecs = mesh_to_xyzrgba_triangles(&mesh, size_info);
                    if !vecs.is_empty() {
                        res.push(NannouVertices { draw_array_mode, vecs });
                    }
                },
                // Shader models, background colors, instanced and indirect drawing are not used
                // by the decorations.
                _ => {},
            }
        }
        res
    }

    /// `gen_vertices` Returns the vertices for an tree created at center x,y with a
    /// specific radius
    pub fn gen_vertices(&self) -> Vec<NannouVertices> {
        // let x = self.x;
        // let y = self.y;
        // let x_60_degrees_offset = COS_60 * self.radius;
        // let y_60_degrees_offset = SIN_60 * self.radius;
        // let ellipse_color = LIGHTSKYBLUE.into_format::<f32>();
        // let ellipse_stroke_color =
        // rgba(ellipse_color.red, ellipse_color.green, ellipse_color.blue, 0.01f32);
        // draw.ellipse().x_y(x, y).radius(self.radius * 0.8).stroke(ellipse_stroke_color).rgba(
        // ellipse_color.red,
        // ellipse_color.green,
        // ellipse_color.blue,
        // self.alpha,
        // );
        // draw.tri()
        // .points(
        // [self.size_info.scale_x(x), self.size_info.scale_y(y)],
        // [
        // self.size_info.scale_x(x + x_60_degrees_offset),
        // self.size_info.scale_y(y + y_60_degrees_offset),
        // ],
        // [self.size_info.scale_x(x + x_60_degrees_offset), self.size_info.scale_y(y)],
        // )
        // .rotate(30f32)
        // .color(VIOLET);
        //
        let mut all_recs = self.polar_clock.day_of_year.vecs.clone();
        all_recs.append(&mut self.polar_clock.month_of_year.vecs.clone());
        all_recs.append(&mut self.polar_clock.day_of_month.vecs.clone());
        all_recs.append(&mut self.polar_clock.hour_of_day.vecs.clone());
        all_recs.append(&mut self.polar_clock.minute_of_hour.vecs.clone());
        all_recs.append(&mut self.polar_clock.seconds_with_millis_of_minute.vecs.clone());
        all_recs.append(&mut self.moon_state.vecs.clone());
        all_recs
    }
}

/// Expands the indexed triangle list of a tessellated mesh into xyzrgba vertices for
/// `glDrawArrays(GL_TRIANGLES)`, with positions scaled to NDC and colors back in sRGB.
fn mesh_to_xyzrgba_triangles(mesh: &Mesh, size_info: SizeInfo) -> Vec<f32> {
    let points = mesh.points();
    let colors = mesh.colors();
    let count = mesh.count_indices();
    let mut res = Vec::with_capacity(count * 7);
    for i in 0..count {
        let idx = mesh.get_index(i) as usize;
        let [x, y, _z] = points[idx];
        let [r, g, b, a] = colors[idx];
        let color = Color::LinearRgba(LinearRgba::new(r, g, b, a)).to_srgba();
        res.push(size_info.scale_x(x));
        res.push(size_info.scale_y(y));
        res.push(0.0);
        res.push(color.red);
        res.push(color.green);
        res.push(color.blue);
        res.push(color.alpha);
    }
    res
}

pub fn parse_svg_path() -> Vec<f32> {
    // tree is created by hand on some svg editor, let's make an SVG Path parser to create the
    // lines, this should be read from the config file
    let res = vec![];
    let _tree = "M 8 8 L 7 7 L 7 6 L 7 5 L 6 4 L 6 2 L 8 2 L 9 1 L 7 1 L 8 0 L 5 -1 L 5 1 L 2 -1 \
                 L 3 1 L 3 2 L 2 2 L 1 3 L 2 3 L 3 3 L 3 4 L 3 4 L 4 5 L 5 6 L 4 7 L 3 8";
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nannou_draw_tessellates_into_triangles() {
        let size_info = SizeInfo { width: 1000., height: 800., ..Default::default() };
        let mut decoration =
            NannouDecoration::new(Rgb { r: 25, g: 88, b: 167 }, 0.5, size_info, 100.);
        decoration.set_size_info(size_info);
        assert!(!decoration.vertices.is_empty());
        for vertices in &decoration.vertices {
            // Triangle list of xyzrgba vertices.
            assert!(!vertices.vecs.is_empty());
            assert_eq!(vertices.vecs.len() % (3 * 7), 0);
            for vertex in vertices.vecs.chunks_exact(7) {
                assert!((-1.5..=1.5).contains(&vertex[0]), "x out of range: {vertex:?}");
                assert!((-1.5..=1.5).contains(&vertex[1]), "y out of range: {vertex:?}");
                assert!(vertex[3..].iter().all(|c| (0.0..=1.0).contains(c)), "{vertex:?}");
            }
        }
        // The ellipse of the moon phase is tessellated as triangles.
        assert!(
            decoration
                .moon_state
                .vecs
                .iter()
                .any(|v| v.draw_array_mode == NannouDrawArrayMode::GlTriangles)
        );
    }
}
