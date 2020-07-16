use alacritty_charts::ChartSizeInfo;
use log::debug;
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
}

impl Decoration for HexagonGridBackground {
    fn render(self) -> Vec<f32> {
        let radius = 100f32; // 100 pixels
        let angle = 60.0f32; // Hexagon degrees
        let x_offset = angle.to_radians().cos() * radius;
        let y_offset = angle.to_radians().sin() * radius;
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
                let mut current_hexagon = vec![
                    // Mid right:
                    self.size_info.scale_x(current_x_position + radius),
                    self.size_info.scale_y(self.size_info.term_size.height as f64, temp_y as f64),
                    // Top right:
                    self.size_info.scale_x(current_x_position + x_offset),
                    self.size_info.scale_y(
                        self.size_info.term_size.height as f64,
                        (temp_y + y_offset) as f64,
                    ),
                    // Top left
                    self.size_info.scale_x(current_x_position - x_offset),
                    self.size_info.scale_y(
                        self.size_info.term_size.height as f64,
                        (temp_y + y_offset) as f64,
                    ),
                    // Mid left:
                    self.size_info.scale_x(current_x_position - radius),
                    self.size_info.scale_y(self.size_info.term_size.height as f64, temp_y as f64),
                    // Bottom left
                    self.size_info.scale_x(current_x_position - x_offset),
                    self.size_info.scale_y(
                        self.size_info.term_size.height as f64,
                        (temp_y - y_offset) as f64,
                    ),
                    // Bottom Right
                    self.size_info.scale_x(current_x_position + x_offset),
                    self.size_info.scale_y(
                        self.size_info.term_size.height as f64,
                        (temp_y - y_offset) as f64,
                    ),
                ];
                opengl_data.append(&mut current_hexagon);
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
