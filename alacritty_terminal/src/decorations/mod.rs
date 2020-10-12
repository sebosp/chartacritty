use crate::charts::Value2D;
use crate::term::color::Rgb;
use crate::term::SizeInfo;
use log::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::UNIX_EPOCH;

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
    /// An array of active decorations
    pub decorations: Vec<DecorationTypes>,
}

impl DecorationsConfig {
    /// `set_size_info` iterates over the enabled decorations and calls the resize method for any
    /// registered decorations
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        info!("Sizing decorations");
        for decor in self.decorations.iter_mut() {
            decor.set_size_info(size_info);
        }
    }
    /// `to_sized_decor_vec` transforms an optional DecorationsConfig into an
    /// DecorationsConfig with resized vector items
    pub fn to_sized_decor_vec(config_decorations: Option<Self>, size_info: SizeInfo) -> Self {
        match config_decorations {
            Some(mut decors) => {
                decors.set_size_info(size_info);
                decors
            }
            None => {
                info!("No decorations to size");
                DecorationsConfig::default()
            }
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
            }
            DecorationTypes::Points(ref mut hexagon_points) => {
                hexagon_points.set_size_info(size_info);
            }
            DecorationTypes::Lines(ref mut hexagon_lines) => {
                hexagon_lines.set_size_info(size_info);
            }
            DecorationTypes::None => {
                unreachable!("Attempting to update decorations on None variant");
            }
        }
    }
    /// `tick` is called every time there is a draw request for the terminal
    pub fn tick(&mut self, time: f32) {
        match self {
            DecorationTypes::Points(ref mut hexagon_points) => {
                hexagon_points.tick(time);
            }
            _ => {}
        }
    }
}

/// DecorationLines represents lines of x,y points.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DecorationLines {
    Hexagon(HexagonLineBackground),
}

impl DecorationLines {
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationLines::Hexagon(ref mut hex_lines) => {
                hex_lines.size_info = size_info;
                hex_lines.update_opengl_vecs();
            }
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
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.size_info = size_info;
                hex_points.update_opengl_vecs();
            }
        }
    }
    pub fn tick(&mut self, time: f32) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.tick(time);
            }
        }
    }
}

/// DecorationTriangles represents sets of triangle of x,y,r,g,b,a properties
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "props")]
pub enum DecorationTriangles {
    Hexagon(HexagonTriangleBackground),
}

impl DecorationTriangles {
    // TODO: Maybe make it a trait?
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationTriangles::Hexagon(ref mut hex_triangles) => {
                hex_triangles.size_info = size_info;
                hex_triangles.update_opengl_vecs();
            }
        }
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
    let mut hexagon_triangles_background =
        HexagonTriangleBackground::new(vertex_color, center_color, alpha, size_info, radius);
    hexagon_triangles_background.update_opengl_vecs();
    DecorationTypes::Triangles(DecorationTriangles::Hexagon(hexagon_triangles_background))
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
    update_interval: usize,
    /// At which epoch ms in time the point animation should start
    start_animation_ms: f32,
    /// The duration of the animation
    animation_duration_ms: f32,
    /// The horizontal distance that should be covered during the animation time
    animation_offset: f32,
    /// The next epoch in which the horizontal move is active
    next_update_epoch: u64,
    pub vecs: Vec<f32>,
}

pub fn create_hexagon_points(
    color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    let mut hexagon_point_background = HexagonPointBackground::new(color, alpha, size_info, radius);
    hexagon_point_background.update_opengl_vecs();
    DecorationTypes::Points(DecorationPoints::Hexagon(hexagon_point_background))
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
        // To avoid colliding with the HexagonLines, the inner triangles ocupy a radius a bit smaller
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
impl HexagonPointBackground {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        let update_interval = 15usize;
        let epoch = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let start_animation_ms = epoch.as_secs_f32() + epoch.subsec_millis() as f32 / 1000f32;
        let animation_duration_ms = 2000f32;
        HexagonPointBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            color,
            alpha,
            size_info,
            radius,
            vecs: vec![],
            chosen_vertices: vec![],
            update_interval,
            start_animation_ms,
            animation_duration_ms,
            animation_offset: 0f32, // SEB TODO: Calculate on top of the hexagon
            next_update_epoch: epoch.as_secs() + (update_interval as u64),
        }
    }
    pub fn choose_random_vertices(&mut self) {
        let random_vertices_to_choose = 5; // SEB TODO: unhardcode later

        // Of the six vertices, we only care about one of them, the top left.
        let total_hexagon_vertices = self.vecs.len() / 6usize;
        let mut rng = rand::thread_rng();
        let current_vertex = 0;
        while current_vertex <= random_vertices_to_choose {
            let new_vertex = usize::from(rng.gen_range(0, total_hexagon_vertices));
            if self.chosen_vertices.len() < current_vertex {
                self.chosen_vertices.push(new_vertex);
            } else {
                self.chosen_vertices[current_vertex] = new_vertex;
            }
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
        let hexagon_top_left_x = self.vecs[4];
        let hexagon_top_right_x = self.vecs[2];
        self.animation_offset = (hexagon_top_right_x - hexagon_top_left_x).abs();
    }
    pub fn tick(&mut self, time: f32) {
        if time > self.start_animation_ms
            && time < self.start_animation_ms + self.animation_duration_ms
        {
            let current_animation_ms = time - self.start_animation_ms;
            // Given this much time, the animation should have added this much offset
            let current_ms_x_offset = (current_animation_ms as f32
                / self.animation_duration_ms as f32)
                * self.animation_offset;
        }
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
