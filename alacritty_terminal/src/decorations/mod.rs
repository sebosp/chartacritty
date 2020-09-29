use crate::charts::Value2D;
use crate::term::color::Rgb;
use crate::term::SizeInfo;
use serde::{Deserialize, Serialize};
//use log::*;

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
    /// An array of active decorations
    pub decorations: Vec<DecorationTypes>,
}
/// DecorationLines represents a line of x,y points.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DecorationLines {
    Hexagon(HexagonLineBackground),
}

/// DecorationPoints represents a line of x,y points.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DecorationPoints {
    Hexagon(HexagonPointBackground),
}

/// DecorationTriangles represents OpenGL Triangle Triangle of x,y points.
/// The usize represents the number of coordinates that make up one fan
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DecorationTriangles {
    Hexagon(HexagonTriangleBackground),
}

/// DecorationGLPrimitives Allows grouping of
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DecorationTypes {
    Lines(DecorationLines),
    Triangles(DecorationTriangles), // Number of triangles per turn
    Points(DecorationPoints),
    None,
}

impl Default for DecorationTypes {
    fn default() -> Self {
        DecorationTypes::None
    }
}
pub fn create_hexagon_line(
    color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    let hexagon_line_background = HexagonLineBackground::new(color, alpha, size_info, radius);
    //hexagon_line_background.update_opengl_vecs();
    DecorationTypes::Lines(DecorationLines::Hexagon(hexagon_line_background))
}

pub fn create_hexagon_triangles(
    vertex_color: Rgb,
    center_color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    // Each vertex has 6 data points, x, y, r, g, b, a
    let mut hexagon_fan_background =
        HexagonTriangleBackground::new(vertex_color, center_color, alpha, size_info, radius);
    hexagon_fan_background.update_opengl_vecs();
    DecorationTypes::Triangles(DecorationTriangles::Hexagon(hexagon_fan_background))
}

pub fn create_hexagon_points(
    color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    let mut hexagon_point_bakcground = HexagonPointBackground::new(color, alpha, size_info, radius);
    hexagon_point_bakcground.update_opengl_vecs();
    DecorationTypes::Points(DecorationPoints::Hexagon(hexagon_point_bakcground))
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonPointBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,
    pub alpha: f32,
    size_info: SizeInfo,
    radius: f32,
    /// Now and then, certain points will be chosen to be moved horizontally
    chosen_vertices: Vec<usize>,
    /// Every these many seconds, chose new points to move
    next_update_interval: u64,
    pub vecs: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonLineBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,
    pub alpha: f32,
    size_info: SizeInfo,
    radius: f32,
    pub vecs: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonTriangleBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub vertex_color: Rgb,
    pub center_color: Rgb,
    pub alpha: f32,
    size_info: SizeInfo,
    radius: f32,
    pub vecs: Vec<f32>,
}

impl HexagonTriangleBackground {
    pub fn new(
        vertex_color: Rgb,
        center_color: Rgb,
        alpha: f32,
        size_info: SizeInfo,
        radius: f32,
    ) -> Self {
        HexagonTriangleBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            vertex_color,
            center_color,
            alpha,
            size_info,
            radius,
            vecs: vec![],
        }
    }
    pub fn update_opengl_vecs(&mut self) {
        let mut res = vec![];
        // To avoid colliding with the HexagonLines, the fans ocupy a radius a bit smaller
        let inner_hexagon_radius_percent = 0.92f32; // XXX: Maybe this can be a field?
        let coords = background_fill_hexagon_positions(self.size_info, self.radius);
        // TODO: The alpha should be calculated inside the shaders
        //          N
        //      3-------2
        //     /         \
        //    /           \
        // W 4      0      1 E
        //    \           /
        //     \         /
        //      5-------6
        //          S
        let mut center = vec![
            0f32, // x
            0f32, // y
            f32::from(self.center_color.r) / 255.,
            f32::from(self.center_color.g) / 255.,
            f32::from(self.center_color.b) / 255.,
            0.0f32,
        ];
        let sides = vec![
            0f32, // x
            0f32, // y
            f32::from(self.vertex_color.r) / 255.,
            f32::from(self.vertex_color.g) / 255.,
            f32::from(self.vertex_color.b) / 255.,
            self.alpha,
        ];
        let mut east = sides.clone();
        let mut northeast = sides.clone();
        let mut northwest = sides.clone();
        let mut west = sides.clone();
        let mut southwest = sides.clone();
        let mut southeast = sides;
        for coord in coords {
            // The first pair of coordinates are the center of the hexagon
            center[0] = self.size_info.scale_x(coord.x);
            center[1] = self.size_info.scale_y(coord.y);
            let hexagon_vertices = gen_hexagon_vertices(
                self.size_info,
                coord.x,
                coord.y,
                self.radius * inner_hexagon_radius_percent,
            );
            // Overwrite the positions
            east[0] = hexagon_vertices[0];
            east[1] = hexagon_vertices[1];
            northeast[0] = hexagon_vertices[2];
            northeast[1] = hexagon_vertices[3];
            northwest[0] = hexagon_vertices[4];
            northwest[1] = hexagon_vertices[5];
            west[0] = hexagon_vertices[6];
            west[1] = hexagon_vertices[7];
            southwest[0] = hexagon_vertices[8];
            southwest[1] = hexagon_vertices[9];
            southeast[0] = hexagon_vertices[10];
            southeast[1] = hexagon_vertices[11];
            // 0, 1, 2, // North-East triangle
            res.append(&mut center.clone());
            res.append(&mut east.clone());
            res.append(&mut northeast.clone());
            // 0, 2, 3, North triangle
            res.append(&mut center.clone());
            res.append(&mut northeast.clone());
            res.append(&mut northwest.clone());
            // 0, 3, 4, North-West triangle
            res.append(&mut center.clone());
            res.append(&mut northwest.clone());
            res.append(&mut west.clone());
            // 0, 4, 5, South-West triangle
            res.append(&mut center.clone());
            res.append(&mut west.clone());
            res.append(&mut southwest.clone());
            // 0, 5, 6, South triangle
            res.append(&mut center.clone());
            res.append(&mut southwest.clone());
            res.append(&mut southeast.clone());
            // 0, 6, 1, South-East triangle
            res.append(&mut center.clone());
            res.append(&mut southeast.clone());
            res.append(&mut east.clone());
        }
        self.vecs = res;
    }
}
impl HexagonLineBackground {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        HexagonLineBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            color,
            alpha,
            size_info,
            radius,
            vecs: vec![],
        }
    }
    pub fn update_opengl_vecs(&mut self) {
        let mut hexagons = vec![];
        let coords = background_fill_hexagon_positions(self.size_info, self.radius);
        for coord in coords {
            hexagons.append(&mut gen_hexagon_vertices(
                self.size_info,
                coord.x,
                coord.y,
                self.radius,
            ));
        }
        self.vecs = hexagons;
    }
}

impl HexagonPointBackground {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        HexagonPointBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            color,
            alpha,
            size_info,
            radius,
            vecs: vec![],
            chosen_vertices: vec![],
            next_update_interval: 15u64,
        }
    }
    pub fn update_opengl_vecs(&mut self) {
        let mut hexagons = vec![];
        let coords = background_fill_hexagon_positions(self.size_info, self.radius);
        for coord in coords {
            hexagons.append(&mut gen_hexagon_vertices(
                self.size_info,
                coord.x,
                coord.y,
                self.radius,
            ));
        }
        self.vecs = hexagons;
    }
}

/// Creates a vector with x,y coordinates in which new hexagons can be drawn
fn background_fill_hexagon_positions(size: SizeInfo, radius: f32) -> Vec<Value2D> {
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
    let mut half_offset = true; // When true, we will add half radius to Y to make sure the hexagons do not overlap
    let mut res = vec![];
    while current_x_position <= (size.width + radius * 2f32) {
        let current_y_position = 0f32;
        let mut temp_y = current_y_position;
        if half_offset {
            // shift the y position in alternate fashion that the positions look like:
            //   x   x   x
            // x   x   x   x
            temp_y -= y_offset;
        }
        while temp_y <= (size.height + radius * 2f32) {
            res.push(Value2D { x: current_x_position, y: temp_y });
            temp_y += y_offset * 2f32;
        }
        half_offset = !half_offset;
        current_x_position += x_offset * 3f32;
    }
    res
}

impl Decoration for HexagonLineBackground {
    fn render(self) -> Vec<f32> {
        let mut hexagons: Vec<f32> = vec![];
        // Let's create an adjusted version of the values that is slightly less than the actual
        // position
        let coords = background_fill_hexagon_positions(self.size_info, self.radius);
        for coord in coords {
            hexagons.append(&mut gen_hexagon_vertices(
                self.size_info,
                coord.x,
                coord.y,
                self.radius,
            ));
        }
        hexagons
    }
}
