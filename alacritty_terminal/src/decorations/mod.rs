//! Decorations for the Alacritty Terminal.
//!
pub use self::nannou::NannouDecoration;
pub use self::nannou::NannouDrawArrayMode;
use crate::charts::Value2D;
use crate::term::SizeInfo;
pub use hexagon_line_background::HexagonLineBackground;
pub use hexagon_point_background::HexagonPointBackground;
pub use hexagon_triangle_background::HexagonTriangleBackground;
use log::*;
pub use polar_clock::PolarClockState;
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub mod hexagon_line_background;
pub mod hexagon_point_background;
pub mod hexagon_triangle_background;
pub mod nannou;
pub mod polar_clock;

// TODO: Use const init that calculates these magic numbers at compile time
const COS_60: f32 = 0.49999997f32;
const SIN_60: f32 = 0.86602545f32;

pub trait Decoration {
    fn render(self) -> Vec<f32>;
    // fn load_vertex_shader(path: &str) -> bool {
    // include_str!(path)
    // }
    // fn load_fragment_shader(path: &str) -> bool{
    // include_str!(path)
    // }
}

/// `DecorationsConfig` contains a vector of decorations and their properties
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct DecorationsConfig {
    /// An array of active decorators
    pub decorators: Vec<DecorationTypes>,

    /// The time at which config was initialized
    #[serde(skip)]
    init_start: Option<Instant>,
}

impl DecorationsConfig {
    /// `set_size_info` iterates over the enabled decorations and calls the resize method for any
    /// registered decorators
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        debug!("DecorationsConfig::set_size_info()");
        for decor in self.decorators.iter_mut() {
            debug!("DecorationsConfig:: iter_mut: {:?}", decor);
            decor.set_size_info(size_info);
        }
    }

    /// `from_optional_decor_config` transforms an optional DecorationsConfig into an
    /// DecorationsConfig with resized vector items
    pub fn optional_decor_to_sized(config_decorations: Option<Self>, size_info: SizeInfo) -> Self {
        match config_decorations {
            Some(mut decors) => {
                decors.set_size_info(size_info);
                decors
            },
            None => {
                info!("No decorations to size");
                DecorationsConfig::default()
            },
        }
    }

    /// `tick` calls the underlying decorators to update decorations that depend on time
    /// such as animations
    pub fn tick(&mut self) {
        let mut time_ms = 0.0f32;
        if let Some(val) = self.init_start {
            let elapsed = val.elapsed();
            time_ms = elapsed.as_secs_f32() + elapsed.subsec_millis() as f32 / 1000f32;
        }
        for decor in self.decorators.iter_mut() {
            decor.tick(time_ms);
        }
    }

    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self) {
        let curr_time = Instant::now();
        self.init_start = Some(curr_time);
        for decor in self.decorators.iter_mut() {
            decor.init_timers(curr_time);
        }
    }
}

// TODO: Maybe we can change the <Type>(Decor<Type>) to simply Decor<Type>
/// DecorationTypes Groups available decorations
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "props")]
pub enum DecorationTypes {
    Lines(DecorationLines),
    Triangles(DecorationTriangles),
    Points(DecorationPoints),
    None,
}
impl Default for DecorationTypes {
    fn default() -> Self {
        DecorationTypes::None
    }
}

impl DecorationTypes {
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        info!("Updating Triangle decorations");
        match self {
            DecorationTypes::Triangles(ref mut hexagon_triangles) => {
                hexagon_triangles.set_size_info(size_info);
            },
            DecorationTypes::Points(ref mut hexagon_points) => {
                hexagon_points.set_size_info(size_info);
            },
            DecorationTypes::Lines(ref mut hexagon_lines) => {
                hexagon_lines.set_size_info(size_info);
            },
            DecorationTypes::None => {
                unreachable!("Attempting to update decorations on None variant");
            },
        }
    }

    /// `tick` is called every time there is a draw request for the terminal
    pub fn tick(&mut self, time: f32) {
        match self {
            DecorationTypes::Points(ref mut hexagon_points) => hexagon_points.tick(time),
            DecorationTypes::Triangles(ref mut tris) => tris.tick(time),
            _ => {},
        }
    }

    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self, time: Instant) {
        if let DecorationTypes::Points(ref mut hexagon_points) = self {
            hexagon_points.init_timers(time);
        }
    }
}

/// DecorationLines represents lines of x,y points.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "props")]
pub enum DecorationLines {
    Hexagon(HexagonLineBackground),
}

impl DecorationLines {
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationLines::Hexagon(ref mut hex_lines) => {
                hex_lines.size_info = size_info;
                hex_lines.update_opengl_vecs();
            },
        }
    }
}

/// DecorationPoints represents sets of x,y points.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "props")]
pub enum DecorationPoints {
    Hexagon(HexagonPointBackground),
}

impl DecorationPoints {
    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self, time: Instant) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.init_timers(time);
            },
        }
    }

    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.size_info = size_info;
                hex_points.update_opengl_vecs();
                hex_points.choose_random_vertices();
            },
        }
    }

    pub fn tick(&mut self, time: f32) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.tick(time);
            },
        }
    }
}

/// DecorationTriangles represents sets of triangle of x,y,r,g,b,a properties
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "props")]
pub enum DecorationTriangles {
    Hexagon(HexagonTriangleBackground),
    Nannou(NannouDecoration),
}

impl DecorationTriangles {
    // TODO: Maybe make it a trait?
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationTriangles::Hexagon(ref mut hex_triangles) => {
                hex_triangles.size_info = size_info;
                hex_triangles.update_opengl_vecs();
            },
            DecorationTriangles::Nannou(ref mut nannou_triangles) => {
                nannou_triangles.set_size_info(size_info);
            },
        }
    }

    pub fn tick(&mut self, time: f32) {
        match self {
            DecorationTriangles::Nannou(ref mut nannou) => {
                nannou.tick(time);
            },
            _ => {},
        }
    }
}

/// `gen_hexagon_vertices` Returns the vertices for an hexagon created at center x,y with a
/// specific radius
pub fn gen_hexagon_vertices(size_info: SizeInfo, x: f32, y: f32, radius: f32) -> Vec<f32> {
    let x_60_degrees_offset = COS_60 * radius;
    let y_60_degrees_offset = SIN_60 * radius;
    // Order of vertices:
    //    3-------2
    //   /         \
    //  /           \
    // 4             1
    //  \           /
    //   \         /
    //    5-------6
    vec![
        // Mid right:
        size_info.scale_x(x + radius),
        size_info.scale_y(y),
        // Top right:
        size_info.scale_x(x + x_60_degrees_offset),
        size_info.scale_y(y + y_60_degrees_offset),
        // Top left
        size_info.scale_x(x - x_60_degrees_offset),
        size_info.scale_y(y + y_60_degrees_offset),
        // Mid left:
        size_info.scale_x(x - radius),
        size_info.scale_y(y),
        // Bottom left
        size_info.scale_x(x - x_60_degrees_offset),
        size_info.scale_y(y - y_60_degrees_offset),
        // Bottom Right
        size_info.scale_x(x + x_60_degrees_offset),
        size_info.scale_y(y - y_60_degrees_offset),
    ]
}

/// Creates a vector with x,y coordinates in which new hexagons can be drawn
fn gen_hex_grid_positions(size: SizeInfo, radius: f32) -> Vec<Value2D> {
    // We only care for the 60 degrees X,Y, the rest we can calculate from this distance.
    // For the degrees at 0, X is the radius, and Y is 0.
    // let angle = 60.0f32; // Hexagon degrees
    // let cos_60 =  angle.to_radians().cos();
    // let sin_60 =  angle.to_radians().sin();
    // let x_offset = angle.to_radians().cos() * radius;
    // let y_offset = angle.to_radians().sin() * radius;
    let x_offset = COS_60 * radius;
    let y_offset = SIN_60 * radius;
    let mut current_x_position = 0f32;
    // When true, we will add half radius to Y to make sure the hexagons do not overlap
    let mut half_offset = true;
    let mut res = vec![];
    while current_x_position < (size.width + x_offset) {
        let current_y_position = 0f32;
        let mut temp_y = current_y_position;
        while temp_y <= (size.height + y_offset) {
            res.push(Value2D {
                x: current_x_position,
                // shift the y position in alternate fashion that the positions look like:
                // x   x   x   x
                //   x   x   x
                y: match half_offset {
                    true => temp_y + y_offset,
                    false => temp_y,
                },
            });
            temp_y += y_offset * 2f32;
        }
        half_offset = !half_offset;
        // Advance by the diameter (2 * radius) + 1 radius so that we find the next center of the
        // adjacent hexagon.
        current_x_position += x_offset * 3f32;
    }
    res
}

/// There is a background of hexagons, a sort of grid, this function finds the center-most in the
/// array of hexagons, they are organized top to bottom, then interleaved a bit to avoid vertex
/// overlapping.
fn find_hexagon_grid_center_idx(coords: &[Value2D], size_info: SizeInfo, radius: f32) -> usize {
    // We need to find the center hexagon.
    let hex_height = SIN_60 * radius * 2.;
    // We'll draw half a hexagon more than needed so that we can interleave them while having the
    // same number of vertical hexagons and let us calculate centers/etc easily.
    let total_height = size_info.height + hex_height / 2.;
    // total number of hexagons vertically, in the grid, the number of them in Y, ceil because
    // hexagons may not be shown partially depending on the terminal size
    let y_hex_n = (total_height / hex_height).ceil() as usize;
    // total number of hexagons horizontally, in the grid, the number of them in X
    let x_hex_n = (coords.len() / y_hex_n) as usize;
    let mut center_idx =
        y_hex_n * (x_hex_n as f32 / 2.).floor() as usize + (y_hex_n as f32 / 2.).floor() as usize;
    if (x_hex_n as f32 / 2.) as usize % 2usize == 0usize {
        // When we are in an even-numbered column, we'll choose a hexagon that is one unit below
        // the pre-calculated. This is because the y-position of the hexagons varies depending on
        // the odd/even column
        center_idx = (center_idx - 1) % coords.len();
    }
    // tracing::info!("NannouDecoration::update_opengl_vecs(size_info) size_info.height: {}, total_height: {total_height}, hex_height: {hex_height}, y_hex_n: {y_hex_n}, x_hex_n: {x_hex_n}, coords.len(): {}, center_idx: {center_idx}, coords: {coords:?}", size_info.height, coords.len());
    // ((x_hex_n as f32 / 2.).floor() * y_hex_n as f32 + (y_hex_n as f32 / 2.).floor()) as usize
    center_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_finds_center_idx() {
        let mut size = SizeInfo::default();
        size.width = 100.;
        size.height = 100.;
        let radius = 10.;
        let hex_radius_y = SIN_60 * radius;
        let hex_radius_x = COS_60 * radius;
        let hex_diameter_y = hex_radius_y * 2.;
        let total_height = size.height + hex_radius_y;
        let total_width = size.width + hex_radius_x;
        // The hexagons are laid vertically by their
        let y_hex_n = (total_height / hex_diameter_y).ceil() as usize;
        // The X position is interleaved by 3 x the radius (x axis) and half a y + radius
        let x_hex_n = ((total_width + hex_radius_x) / (hex_radius_x * 3.)).ceil() as usize;
        let hex_coords = gen_hex_grid_positions(size, radius);
        // Hexagons are laid vertically by increments of (sin(60) * diameter) along the Y axis.
        assert_eq!(y_hex_n, 7);
        assert_eq!(x_hex_n, 8);
        assert_eq!(hex_coords.len(), 56);
    }
}
