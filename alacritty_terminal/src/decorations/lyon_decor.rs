//! Lyon-based decorations for Alacritty

use super::moon_phase::MoonPhaseState;
use super::PolarClockState;
use crate::term::SizeInfo;
use chrono::prelude::*;
use lyon::tessellation as tess;
use palette::rgb::{FromHexError, Rgb, Rgba, Srgb};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tess::geometry_builder::{simple_builder, VertexBuffers};
use tess::math::Point;
use tess::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct LyonDecoration {
    #[serde(deserialize_with = "from_str_serde")]
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
    pub vertices: Vec<Vec<f32>>,
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

fn from_str_serde<'de, D>(deserializer: D) -> Result<Rgb, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = if let Some(stripped) = s.strip_prefix("0x") { format!("#{}", stripped) } else { s };
    let color_convert: Result<Rgb<Srgb, u8>, FromHexError> = Rgb::<Srgb, u8>::from_str(&s);
    let color = color_convert.map_err(serde::de::Error::custom)?;
    Ok(Rgb::new(color.red as f32 / 255f32, color.green as f32 / 255f32, color.blue as f32 / 255f32))
}

fn local_now() -> DateTime<Local> {
    Local::now()
}

impl LyonDecoration {
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

    /// Called after instantiation of the LyonDecoration, it will initialize the vertices for the
    /// decorations.
    pub fn update_opengl_vecs(&mut self) {
        // tracing::info!("LyonDecoration::update_opengl_vecs(size_info) {:?}, center_idx: {}, x: {}, y:{}, radius: {}, coords: {:?}", self.size_info, center_idx, coord.x, coord.y, self.radius, coords);
        self.vertices = self.gen_vertices();
    }

    /// Transforms lyon paths into xyzrgba vertices we can draw through our renderer
    pub fn gen_vertices_from_lyon_path(
        path: &lyon::path::Path,
        size_info: SizeInfo,
        color: Rgba<f32>,
    ) -> Vec<f32> {
        // Create the destination vertex and index buffers.
        let mut buffers: VertexBuffers<Point, u16> = VertexBuffers::new();

        {
            let mut vertex_builder = simple_builder(&mut buffers);

            // Create the tessellator.
            let mut tessellator = StrokeTessellator::new();

            // Compute the tessellation.
            let result = tessellator.tessellate_path(
                path,
                &StrokeOptions::default().with_line_width(4.).with_tolerance(50.),
                &mut vertex_builder,
            );
            assert!(result.is_ok());
        }
        // No idea how gl Draw Elements work so let's build the payload by hand:
        let mut vertices: Vec<f32> = Vec::with_capacity(buffers.indices.len() * 7usize);
        for idx in buffers.indices {
            vertices.push(size_info.scale_x(buffers.vertices[idx as usize].x));
            vertices.push(size_info.scale_y(buffers.vertices[idx as usize].y));
            vertices.push(0.0); // z
            vertices.push(color.color.red);
            vertices.push(color.color.green);
            vertices.push(color.color.blue);
            vertices.push(color.alpha);
        }
        vertices
    }

    /// `gen_vertices` Returns the vertices for a polar clock created at center x,y with a
    /// specific radius
    pub fn gen_vertices(&self) -> Vec<Vec<f32>> {
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
        vec![
            self.polar_clock.day_of_year.vecs.clone(),
            self.polar_clock.month_of_year.vecs.clone(),
            self.polar_clock.day_of_month.vecs.clone(),
            self.polar_clock.hour_of_day.vecs.clone(),
            self.polar_clock.minute_of_hour.vecs.clone(),
            self.polar_clock.seconds_with_millis_of_minute.vecs.clone(),
            self.moon_state.vecs.clone(),
        ]
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
