//! Nannou-based decorations for Alacritty

use chrono::prelude::*;
use chrono::NaiveDate;
use lyon::tessellation::{FillTessellator, StrokeTessellator};
use nannou::draw;
use crate::term::SizeInfo;
use serde::{Deserialize, Serialize};
pub use nannou::draw::primitive::Primitive;
use nannou::draw::renderer::{GlyphCache, RenderPrimitive};
pub use nannou::draw::State;
use nannou::glam::Vec2;
use nannou::prelude::*;
use super::PolarClockState;

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
    pub polar_clock: PolarClockState,
    #[serde(default)]
    pub vertices: Vec<NannouVertices>,
    #[serde(default = "local_now")]
    pub now: DateTime<Local>,
    #[serde(default)]
    pub coord_x: f32,
    #[serde(default)]
    pub coord_y: f32,
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
        .build()
        .into();
    let pixel_buffer = vec![0u8; w as usize * h as usize];
    let requires_upload = false;
    GlyphCache { cache, pixel_buffer, requires_upload }
}

impl NannouDecoration {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        let polar_clock = PolarClockState::new();
        Self {
            color,
            alpha,
            size_info,
            radius,
            polar_clock,
            vertices: Default::default(),
            now: Local::now(),
            last_drawn_msecs: 0f32,
            coord_x: 0f32,
            coord_y: 0f32,
        }
    }

    pub fn tick(&mut self, time: f32) {
        if time.floor() != self.last_drawn_msecs.floor() {
            self.now = Local::now();
            self.vertices = self.gen_vertices();
        }
    }

    pub fn update_opengl_vecs(&mut self) {
        let coords = super::gen_hex_grid_positions(self.size_info, self.radius);
        let center_idx = super::find_hexagon_grid_center_idx(&coords, self.size_info, self.radius);
        let coord = coords[center_idx];
        self.now = Local::now();
        // self.alpha = 0.7f32;
        // Store the center hexagon position for re-use later.
        self.coord_x = coord.x;
        self.coord_y = coord.y;
        // tracing::info!("NannouDecoration::update_opengl_vecs(size_info) {:?}, center_idx: {}, x: {}, y:{}, radius: {}, coords: {:?}", self.size_info, center_idx, coord.x, coord.y, self.radius, coords);
        self.vertices = self.gen_vertices();
    }

    fn draw_arc_path(&self, draw: &draw::Draw, arc_radius: f32, end_angle: f32, arc_color: Rgba<f32>, stroke_weight: f32) {
        draw.path()
            .stroke()
            .stroke_weight(stroke_weight)
            .color(arc_color)
            .caps_round()
            .events(build_time_arc(self.coord_x, self.coord_y, arc_radius, end_angle).iter());
    }

    fn draw_month_of_year_arc_vertices(&self) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        let month_in_year_angle = 360f32 * self.now.month() as f32 / 12f32;
        let month_in_year_rgb = LIGHTBLUE.into_format::<f32>();
        let month_in_year_rgba = rgba(month_in_year_rgb.red, month_in_year_rgb.green, month_in_year_rgb.blue, self.alpha * POLAR_CLOCK_MONTH_OF_YEAR_ALPHA_MULTIPLIER);
        self.draw_arc_path(&draw, self.radius * POLAR_CLOCK_MONTH_OF_YEAR_RADIUS_MULTIPLIER, month_in_year_angle ,month_in_year_rgba, 8.);
        self.gen_vertices_from_nannou_draw(draw)
    }

    fn draw_day_of_month_arc_vertices(&self) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        let first_day_of_next_year = NaiveDate::from_ymd_opt(self.now.year() + 1, 1, 1).unwrap();
        let first_day_of_next_month = if self.now.month() == 12 {
            first_day_of_next_year
        } else {
            NaiveDate::from_ymd_opt(self.now.year(), self.now.month() + 1, 1).unwrap()
        };
        let first_day_of_month =
            NaiveDate::from_ymd_opt(self.now.year(), self.now.month(), 1).unwrap();
        let days_in_month =
            first_day_of_next_month.signed_duration_since(first_day_of_month).num_days();
        let day_in_month_angle = 360f32 * self.now.day() as f32 / days_in_month as f32;
        let day_in_month_rgb = GRAY.into_format::<f32>();
        let day_in_month_rgba = rgba(day_in_month_rgb.red, day_in_month_rgb.green, day_in_month_rgb.blue, self.alpha * POLAR_CLOCK_DAY_OF_MONTH_ALPHA_MULTIPLIER);
        self.draw_arc_path(&draw, self.radius * POLAR_CLOCK_DAY_OF_MONTH_RADIUS_MULTIPLIER, day_in_month_angle, day_in_month_rgba, 8.);
        self.gen_vertices_from_nannou_draw(draw)
    }

    fn draw_hour_of_day_arc_vertices(&self) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        let hour_in_day_angle = 360f32 * self.now.hour() as f32 / 24f32;
        let (hour_in_day_rgb, hour_in_day_alpha) = if self.now.hour() >= 9 && self.now.hour() < 17
        {
            (LIGHTBLUE.into_format::<f32>(), self.alpha * POLAR_CLOCK_WORKHOUR_OF_DAY_ALPHA_MULTIPLIER)
        } else {
            (DARKRED.into_format::<f32>(), self.alpha * POLAR_CLOCK_NONWORKHOUR_OF_DAY_ALPHA_MULTIPLIER)
        };
        let hour_in_day_rgba = rgba(hour_in_day_rgb.red, hour_in_day_rgb.green, hour_in_day_rgb.blue, hour_in_day_alpha);
        self.draw_arc_path(&draw, self.radius * POLAR_CLOCK_HOUR_OF_DAY_RADIUS_MULTIPLIER, hour_in_day_angle, hour_in_day_rgba, 8.);
        self.gen_vertices_from_nannou_draw(draw)
    }

    fn draw_minute_of_hour_arc_vertices(&self) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        let minute_in_hour_angle = 360f32 * self.now.minute() as f32 / 60f32;
        let minute_in_hour_rgb = GRAY.into_format::<f32>();
        let minute_in_hour_rgba = rgba(minute_in_hour_rgb.red, minute_in_hour_rgb.green, minute_in_hour_rgb.blue, self.alpha * POLAR_CLOCK_MINUTE_OF_HOUR_ALPHA_MULTIPLIER);
        self.draw_arc_path(&draw, self.radius * POLAR_CLOCK_MINUTE_OF_HOUR_RADIUS_MULTIPLIER, minute_in_hour_angle, minute_in_hour_rgba, 8.);
        self.gen_vertices_from_nannou_draw(draw)
    }

    fn draw_second_of_minute_arc_vertices(&self) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        let second_in_minute_angle = 360f32
            * (self.now.second() as f32 * 1000f32
                + (self.now.nanosecond() as f32 / 1_000_000f32).floor())
            / 60_000f32;
        let second_in_minute_rgb = AQUA.into_format::<f32>();
        let second_in_minute_rgba = rgba(second_in_minute_rgb.red, second_in_minute_rgb.green, second_in_minute_rgb.blue, self.alpha * POLAR_CLOCK_SECOND_OF_MINUTE_ALPHA_MULTIPLIER);
        self.draw_arc_path(&draw, self.radius * POLAR_CLOCK_SECOND_OF_MINUTE_RADIUS_MULTIPLIER, second_in_minute_angle, second_in_minute_rgba, 8.);
        self.gen_vertices_from_nannou_draw(draw)
    }

    // Transforms nannou::draw::Draw into xyrgba vertices we can draw through our renderer
    fn gen_vertices_from_nannou_draw(draw: draw::Draw, size_info: SizeInfo) -> Vec<NannouVertices> {
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
                        intermediary_mesh: &intermediary_state.intermediary_mesh(),
                        path_event_buffer: &intermediary_state.path_event_buffer(),
                        path_points_colored_buffer: &intermediary_state
                            .path_points_colored_buffer(),
                        path_points_textured_buffer: &intermediary_state
                            .path_points_textured_buffer(),
                        text_buffer: &intermediary_state.text_buffer(),
                        theme: &draw_state.theme(),
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
        /*let x = self.coord_x;
        let y = self.coord_y;
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

        draw.ellipse()
            .x_y(x + x_60_degrees_offset, y + y_60_degrees_offset)
            .radius(self.radius * 0.4)
            .stroke(ellipse_stroke_color)
            .rgba(ellipse_color.red, ellipse_color.green, ellipse_color.blue, self.alpha * 0.10);
        */
        let mut all_recs = vec![];
        all_recs.append(&mut self.draw_day_of_year_arc_vertices());
        all_recs.append(&mut self.draw_month_of_year_arc_vertices());
        all_recs.append(&mut self.draw_day_of_month_arc_vertices());
        all_recs.append(&mut self.draw_hour_of_day_arc_vertices());
        all_recs.append(&mut self.draw_minute_of_hour_arc_vertices());
        all_recs.append(&mut self.draw_second_of_minute_arc_vertices());
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

