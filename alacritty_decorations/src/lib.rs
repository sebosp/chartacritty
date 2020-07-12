use alacritty_charts::ChartSizeInfo;
#[macro_use]
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
        let radius = 100f32; // 500 pixels
        let angle = 60.0f32;
        let x_offset = angle.to_radians().cos() * radius;
        let y_offset = angle.to_radians().sin() * radius;
        let r_center_x = self.size_info.term_size.width / 2f32;
        let r_center_y = self.size_info.term_size.height / 2f32;
        debug!("r_center_x : {}, r_center_y: {}", r_center_x, r_center_y);
        debug!("x_offset : {}, y_offset: {}", x_offset, y_offset);
        let opengl_data = vec![
            // Mid right:
            self.size_info.scale_x(r_center_x + radius),
            self.size_info.scale_y(self.size_info.term_size.height as f64, r_center_y as f64),
            // Top right:
            self.size_info.scale_x(r_center_x + x_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (r_center_y + y_offset) as f64),
            // Top left
            self.size_info.scale_x(r_center_x - x_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (r_center_y + y_offset) as f64),
            // Mid left:
            self.size_info.scale_x(r_center_x - radius),
            self.size_info.scale_y(self.size_info.term_size.height as f64, r_center_y as f64),
            // Bottom left
            self.size_info.scale_x(r_center_x - x_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (r_center_y - y_offset) as f64),
            // Bottom Right
            self.size_info.scale_x(r_center_x + x_offset),
            self.size_info
                .scale_y(self.size_info.term_size.height as f64, (r_center_y - y_offset) as f64),
        ];
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
