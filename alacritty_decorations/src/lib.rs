use alacritty_charts::Value2D;
use alacritty_common::Rgb;
use alacritty_common::SizeInfo;
use log::*;

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

/// DecorationLines represents a line of x,y points.
pub enum DecorationLines {
    Hexagon(HexagonLineBackground),
}

/// DecorationPoints represents a line of x,y points.
pub enum DecorationPoints {
    Hexagon(HexagonPointBackground),
}

/// DecorationFans represents OpenGL Triangle Fan of x,y points.
/// The usize represents the number of coordinates that make up one fan
pub enum DecorationFans {
    Hexagon((HexagonFanBackground, usize)),
}

/// DecorationGLPrimitives Allows grouping of
pub enum DecorationTypes {
    Lines(DecorationLines),
    Fans(DecorationFans), // Number of triangles per turn
    Points(DecorationPoints),
}

pub fn create_hexagon_line(
    color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    DecorationTypes::Lines(DecorationLines::Hexagon(HexagonLineBackground::new(
        color, alpha, size_info, radius,
    )))
}

pub fn create_hexagon_fan(
    vertex_color: Rgb,
    center_color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    let num_vertices: usize = 7usize * 2usize; // 6 vertices plus the center for the hexagon fan.
    DecorationTypes::Fans(DecorationFans::Hexagon((
        HexagonFanBackground::new(vertex_color, center_color, alpha, size_info, radius),
        num_vertices,
    )))
}

pub fn create_hexagon_points(
    color: Rgb,
    alpha: f32,
    size_info: SizeInfo,
    radius: f32,
) -> DecorationTypes {
    DecorationTypes::Points(DecorationPoints::Hexagon(HexagonPointBackground::new(
        color, alpha, size_info, radius,
    )))
}

/// `gen_hexagon_vertices` Returns the vertices for an hexagon created at center x,y with a
/// specific radius
pub fn gen_hexagon_vertices(size_info: SizeInfo, x: f32, y: f32, radius: f32) -> Vec<f32> {
    let x_60_degrees_offset = COS_60 * radius;
    let y_60_degrees_offset = SIN_60 * radius;
    // Order of vertices:
    //    3------2
    //   /        \
    //  /          \
    // 4            1
    //  \          /
    //   \        /
    //    5------6
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

pub struct HexagonPointBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,
    pub alpha: f32,
    size_info: SizeInfo,
    radius: f32,
    pub vecs: Vec<f32>,
}
pub struct HexagonLineBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,
    pub alpha: f32,
    size_info: SizeInfo,
    radius: f32,
    pub vecs: Vec<f32>,
}

pub struct HexagonFanBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub vertex_color: Rgb,
    pub center_color: Rgb,
    pub alpha: f32,
    size_info: SizeInfo,
    radius: f32,
    pub vecs: Vec<f32>,
}

impl HexagonFanBackground {
    pub fn new(
        vertex_color: Rgb,
        center_color: Rgb,
        alpha: f32,
        size_info: SizeInfo,
        radius: f32,
    ) -> Self {
        HexagonFanBackground {
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
        let mut hexagons = vec![];
        // To avoid colliding with the HexagonLines, the fans ocupy a radius a bit smaller
        let inner_hexagon_radius_percent = 0.92f32; // XXX: Maybe this can be a field?
        let coords = background_fill_hexagon_positions(self.size_info, self.radius);
        for coord in coords {
            hexagons.push(self.size_info.scale_x(coord.x));
            hexagons.push(self.size_info.scale_y(coord.y));
            hexagons.append(&mut gen_hexagon_vertices(
                self.size_info,
                coord.x,
                coord.y,
                self.radius * inner_hexagon_radius_percent,
            ));
        }
        self.vecs = hexagons;
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
    while current_x_position <= size.width {
        let current_y_position = 0f32;
        let mut temp_y = current_y_position;
        if half_offset {
            // shift the y position in alternate fashion that the positions look like:
            // x   x   x   x
            //   x   x   x
            temp_y += y_offset;
        }
        while temp_y <= size.height {
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
        // What is returned:
        hexagons
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
