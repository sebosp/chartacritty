//! Hexagon Line Background Decorations
use crate::term::color::Rgb;
use crate::term::SizeInfo;
use serde::{Deserialize, Serialize};
use super::Decoration;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonLineBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,
    pub alpha: f32,
    #[serde(default)]
    pub size_info: SizeInfo,
    radius: f32,
    #[serde(default)]
    pub vecs: Vec<f32>,
}

impl HexagonLineBackground {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        Self {
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
        let coords = super::gen_hex_grid_positions(self.size_info, self.radius);
        for coord in coords {
            hexagons.append(&mut super::gen_2d_hexagon_vertices(
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
        // TODO: Why is this not the same as Self::update_opengl_vecs ?
        let mut hexagons: Vec<f32> = vec![];
        // Let's create an adjusted version of the values that is slightly less than the actual
        // position
        let coords = super::gen_hex_grid_positions(self.size_info, self.radius);
        for coord in coords {
            hexagons.append(&mut super::gen_2d_hexagon_vertices(
                self.size_info,
                coord.x,
                coord.y,
                self.radius,
            ));
        }
        hexagons
    }
}

