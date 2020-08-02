use alacritty_charts::ChartSizeInfo;
use alacritty_charts::Value2D;
use log::*;
pub trait Decoration {
    fn render(self) -> Vec<f32>;
    // fn load_vertex_shader(path: &str) -> bool {
    // include_str!(path)
    // }
    // fn load_fragment_shader(path: &str) -> bool{
    // include_str!(path)
    // }
}

// TODO: Add an array to the renderer mode for new decorations
pub struct HexagonLineBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    size_info: ChartSizeInfo,
    radius: f32,
}

impl HexagonLineBackground {
    pub fn new(size_info: ChartSizeInfo, radius: f32) -> Self {
        HexagonLineBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            size_info,
            radius,
        }
    }

    pub fn create_hexagon_fan(
        &self,
        x: f32,
        y: f32,
        radius: f32,
        x_60_degrees_offset: f32,
        y_60_degrees_offset: f32,
    ) -> Vec<f32> {
        let mut res = vec![
            // Center, to be used for triangle fan
            self.size_info.scale_x(x),
            self.size_info.scale_y(self.size_info.term_size.height as f64, y as f64),
        ];
        res.append(&mut self.create_hexagon(
            x,
            y,
            radius,
            x_60_degrees_offset,
            y_60_degrees_offset,
        ));
        res
    }
    pub fn create_hexagon(
        &self,
        x: f32,
        y: f32,
        radius: f32,
        x_60_degrees_offset: f32,
        y_60_degrees_offset: f32,
    ) -> Vec<f32> {
        vec![
            // Mid right:
            self.size_info.scale_x(x + radius),
            self.size_info.scale_y(self.size_info.term_size.height as f64, y as f64),
            // Top right:
            self.size_info.scale_x(x + x_60_degrees_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (y + y_60_degrees_offset) as f64),
            // Top left
            self.size_info.scale_x(x - x_60_degrees_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (y + y_60_degrees_offset) as f64),
            // Mid left:
            self.size_info.scale_x(x - radius),
            self.size_info.scale_y(self.size_info.term_size.height as f64, y as f64),
            // Bottom left
            self.size_info.scale_x(x - x_60_degrees_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (y - y_60_degrees_offset) as f64),
            // Bottom Right
            self.size_info.scale_x(x + x_60_degrees_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (y - y_60_degrees_offset) as f64),
        ]
    }
}

/// Creates a vector with x,y coordinates in which new hexagons can be drawn
/// The offsets (x,y) to the first 60 degrees point are alse returned as the hexagon is pretty
/// symmetric, this should probably be changed...
fn background_fill_hexagon_positions(size: ChartSizeInfo, radius: f32) -> (Value2D, Vec<Value2D>) {
    // We only care for the 60 degrees X,Y, the rest we can calculate from this distance.
    // For the degrees at 0, X is the radius, and Y is 0.
    // let angle = 60.0f32; // Hexagon degrees
    // let cos_60 =  angle.to_radians().cos();
    // let sin_60 =  angle.to_radians().sin();
    // let x_offset = angle.to_radians().cos() * radius;
    // let y_offset = angle.to_radians().sin() * radius;
    let cos_60 = 0.49999997f32;
    let sin_60 = 0.86602545f32;
    let x_offset = cos_60 * radius;
    let y_offset = sin_60 * radius;
    let mut current_x_position = 0f32;
    let mut half_offset = true; // When true, we will add half radius to Y to make sure the hexagons do not overlap
    let mut res = vec![];
    while current_x_position <= size.term_size.width {
        let current_y_position = 0f32;
        let mut temp_y = current_y_position;
        if half_offset {
            temp_y += y_offset;
        }
        while temp_y <= size.term_size.height {
            res.push(Value2D { x: current_x_position, y: temp_y });
            temp_y += y_offset * 2f32;
        }
        half_offset = !half_offset;
        current_x_position += x_offset * 3f32;
    }
    (Value2D { x: x_offset, y: y_offset }, res)
}

impl Decoration for HexagonLineBackground {
    fn render(self) -> Vec<f32> {
        let mut outer_hexagons: Vec<f32> = vec![];
        let mut inner_hexagons: Vec<f32> = vec![];
        // Let's create an adjusted version of the values that is slightly less than the actual
        // position
        let inner_hexagon_radius_percent = 0.92f32;
        let adjusted_radius = self.radius * inner_hexagon_radius_percent;
        let (offsets, coords) = background_fill_hexagon_positions(self.size_info, self.radius);
        for coord in coords {
            // Inner hexagon:
            inner_hexagons.append(&mut self.create_hexagon_fan(
                coord.x,
                coord.y,
                adjusted_radius,
                coord.x * inner_hexagon_radius_percent,
                coord.x * inner_hexagon_radius_percent,
            ));
            // Outer radius
            outer_hexagons.append(&mut self.create_hexagon(
                coord.x,
                coord.y,
                self.radius,
                x_offset,
                y_offset,
            ));
        }
        outer_hexagons.append(&mut inner_hexagons);
        // What is returned:
        // First, the outer(bigger hexagons whos vertices touch the other outer hexagons
        // Then the inner hexagons that are slightly less and:
        // TODO: should in the future become triangle strips and the closer they get to the center
        // the darker.
        outer_hexagons
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
