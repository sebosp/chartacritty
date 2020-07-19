use alacritty_charts::ChartSizeInfo;
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
pub struct HexagonGridBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    size_info: ChartSizeInfo,
}

impl HexagonGridBackground {
    pub fn new(size_info: ChartSizeInfo) -> Self {
        HexagonGridBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            size_info,
        }
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

impl Decoration for HexagonGridBackground {
    fn render(self) -> Vec<f32> {
        let radius = 100f32; // 100 pixels

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
        // Let's create an adjusted version of the values that is slightly less than the actual
        // position
        let adjusted_radius = radius * 0.92;
        let adjusted_x_offset = x_offset * 0.92;
        let adjusted_y_offset = y_offset * 0.92;
        let mut current_x_position = 0f32;
        let mut half_offset = true; // When true, we will add half radius to Y to make sure the hexagons do not overlap
        let mut opengl_data: Vec<f32> = vec![];
        while current_x_position <= self.size_info.term_size.width {
            let current_y_position = 0f32;
            let mut temp_y = current_y_position;
            if half_offset {
                temp_y += y_offset;
            }
            while temp_y <= self.size_info.term_size.height {
                // Inner hexagon:
                opengl_data.append(&mut self.create_hexagon(
                    current_x_position,
                    temp_y,
                    adjusted_radius,
                    adjusted_x_offset,
                    adjusted_y_offset,
                ));
                // Outer radius
                opengl_data.append(&mut self.create_hexagon(
                    current_x_position,
                    temp_y,
                    radius,
                    x_offset,
                    y_offset,
                ));
                temp_y += y_offset * 2f32;
            }
            half_offset = !half_offset;
            current_x_position += x_offset * 3f32;
        }
        opengl_data
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
