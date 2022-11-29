//! Nannou-based decorations for Alacritty

use super::PolarClockState;
use crate::term::color::Rgb;
use crate::term::SizeInfo;
use chrono::prelude::*;
use lyon::tessellation::{FillTessellator, StrokeTessellator};
use nannou::draw;
pub use nannou::draw::primitive::Primitive;
use nannou::draw::renderer::{GlyphCache, RenderPrimitive};
pub use nannou::draw::State;
use nannou::glam::Vec2;
use serde::{Deserialize, Serialize};
use super::moon_phase::MoonPhaseState;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum NannouDrawArrayMode {
    Points,
    LineStrip,
    LineLoop,
    GlTriangles,
}

impl Default for NannouDrawArrayMode {
    fn default() -> Self {
        Self::LineStrip
    }
}

impl From<nannou::draw::primitive::Primitive> for NannouDrawArrayMode {
    fn from(src: nannou::draw::primitive::Primitive) -> Self {
        match src {
            nannou::draw::primitive::Primitive::Mesh(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Tri(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Polygon(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Ellipse(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Quad(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Rect(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Line(_) => NannouDrawArrayMode::LineStrip,
            nannou::draw::primitive::Primitive::Text(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Texture(_) => NannouDrawArrayMode::GlTriangles,
            nannou::draw::primitive::Primitive::Path(_) => NannouDrawArrayMode::LineStrip,
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

// TODO: Move this somewhere sensical...
fn new_glyph_cache() -> GlyphCache {
    let size = nannou::draw::Renderer::DEFAULT_GLYPH_CACHE_SIZE;
    let scale_tolerance = nannou::draw::Renderer::DEFAULT_GLYPH_CACHE_SCALE_TOLERANCE;
    let position_tolerance = nannou::draw::Renderer::DEFAULT_GLYPH_CACHE_POSITION_TOLERANCE;
    let [w, h] = size;
    let cache = nannou::text::GlyphCache::builder()
        .dimensions(w, h)
        .scale_tolerance(scale_tolerance)
        .position_tolerance(position_tolerance)
        .build();
    let pixel_buffer = vec![0u8; w as usize * h as usize];
    let requires_upload = false;
    GlyphCache { cache, pixel_buffer, requires_upload }
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
        // tracing::info!("NannouDecoration::update_opengl_vecs(size_info) {:?}, center_idx: {}, x: {}, y:{}, radius: {}, coords: {:?}", self.size_info, center_idx, coord.x, coord.y, self.radius, coords);
        self.vertices = self.gen_vertices();
    }

    // Transforms nannou::draw::Draw into xyrgba vertices we can draw through our renderer
    pub fn gen_vertices_from_nannou_draw(
        draw: draw::Draw,
        size_info: SizeInfo,
    ) -> Vec<NannouVertices> {
        let mut res = vec![];
        draw.finish_remaining_drawings();
        // Trying to adapt nannou crate nannou/src/draw/renderer/mod.rs `fill()` function
        let mut mesh = draw::Mesh::default();
        // Construct the glyph cache.
        let mut glyph_cache = new_glyph_cache();
        let mut fill_tessellator = FillTessellator::new();
        let mut stroke_tessellator = StrokeTessellator::new();
        // Keep track of context changes.
        let mut curr_ctxt = draw::Context::default();
        let draw_cmds: Vec<_> = draw.drain_commands().collect();
        let draw_state = draw.state();
        let intermediary_state = draw_state.intermediary_state();
        let scale_factor = 1.;
        for cmd in draw_cmds {
            match cmd {
                draw::DrawCommand::Context(ctxt) => curr_ctxt = ctxt,
                draw::DrawCommand::Primitive(prim) => {
                    // Info required during rendering.
                    let ctxt = draw::renderer::RenderContext {
                        intermediary_mesh: intermediary_state.intermediary_mesh(),
                        path_event_buffer: intermediary_state.path_event_buffer(),
                        path_points_colored_buffer: intermediary_state
                            .path_points_colored_buffer(),
                        path_points_textured_buffer: intermediary_state
                            .path_points_textured_buffer(),
                        text_buffer: intermediary_state.text_buffer(),
                        theme: draw_state.theme(),
                        transform: &curr_ctxt.transform,
                        fill_tessellator: &mut fill_tessellator,
                        stroke_tessellator: &mut stroke_tessellator,
                        glyph_cache: &mut glyph_cache,
                        output_attachment_size: Vec2::new(2., 2.),
                        output_attachment_scale_factor: scale_factor,
                    };

                    let draw_array_mode = prim.clone().into();
                    // Render the primitive.
                    let _render = prim.render_primitive(ctxt, &mut mesh);
                    let mut primitive_render_vecs = vec![];
                    for vx in mesh.vertices() {
                        primitive_render_vecs.push(size_info.scale_x(vx.x));
                        primitive_render_vecs.push(size_info.scale_y(vx.y));
                        primitive_render_vecs.push(0.0);
                        primitive_render_vecs.push(vx.color.red);
                        primitive_render_vecs.push(vx.color.green);
                        primitive_render_vecs.push(vx.color.blue);
                        primitive_render_vecs.push(vx.color.alpha);
                    }
                    res.push(NannouVertices { draw_array_mode, vecs: primitive_render_vecs });
                },
            }
        }
        res
    }

    /// `gen_vertices` Returns the vertices for an tree created at center x,y with a
    /// specific radius
    pub fn gen_vertices(&self) -> Vec<NannouVertices> {
        /*let x = self.x;
        let y = self.y;
        let x_60_degrees_offset = COS_60 * self.radius;
        let y_60_degrees_offset = SIN_60 * self.radius;
        let ellipse_color = LIGHTSKYBLUE.into_format::<f32>();
        let ellipse_stroke_color =
            rgba(ellipse_color.red, ellipse_color.green, ellipse_color.blue, 0.01f32);
        draw.ellipse().x_y(x, y).radius(self.radius * 0.8).stroke(ellipse_stroke_color).rgba(
            ellipse_color.red,
            ellipse_color.green,
            ellipse_color.blue,
            self.alpha,
        );
        draw.tri()
            .points(
                [self.size_info.scale_x(x), self.size_info.scale_y(y)],
                [
                    self.size_info.scale_x(x + x_60_degrees_offset),
                    self.size_info.scale_y(y + y_60_degrees_offset),
                ],
                [self.size_info.scale_x(x + x_60_degrees_offset), self.size_info.scale_y(y)],
            )
            .rotate(30f32)
            .color(VIOLET);

        */
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

pub fn parse_svg_path() -> Vec<f32> {
    // tree is created by hand on some svg editor, let's make an SVG Path parser to create the
    // lines, this should be read from the config file
    let res = vec![];
    let _tree = "M 8 8 L 7 7 L 7 6 L 7 5 L 6 4 L 6 2 L 8 2 L 9 1 L 7 1 L 8 0 L 5 -1 L 5 1 L 2 -1 \
                 L 3 1 L 3 2 L 2 2 L 1 3 L 2 3 L 3 3 L 3 4 L 3 4 L 4 5 L 5 6 L 4 7 L 3 8";
    res
}
