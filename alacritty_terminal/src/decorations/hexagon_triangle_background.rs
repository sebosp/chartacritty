//! Hexagon Triangle Background decoration

use crate::term::color::Rgb;
use crate::term::SizeInfo;
use noise::Perlin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HexagonTriangleBackground {
    pub vertex_color: Rgb,
    pub center_color: Rgb,
    pub alpha: f32,
    #[serde(default)]
    pub size_info: SizeInfo,
    radius: f32,
    #[serde(default)]
    pub vecs: Vec<f32>,
    #[serde(skip)]
    pub noise: noise::Perlin,
}

impl PartialEq for HexagonTriangleBackground {
    fn eq(&self, rhs: &Self) -> bool {
        self.vertex_color == rhs.vertex_color
            && self.center_color == rhs.center_color
            && self.alpha == rhs.alpha
            && self.size_info == rhs.size_info
            && self.radius == rhs.radius
            && self.vecs == rhs.vecs
    }
}

impl HexagonTriangleBackground {
    pub fn new(
        vertex_color: Rgb,
        center_color: Rgb,
        alpha: f32,
        size_info: SizeInfo,
        radius: f32,
    ) -> Self {
        let noise = Default::default();
        HexagonTriangleBackground {
            vertex_color,
            center_color,
            alpha,
            size_info,
            radius,
            vecs: vec![],
            noise,
        }
    }

    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        self.size_info = size_info;
        self.update_opengl_vecs();
    }

    pub fn update_opengl_vecs(&mut self) {
        let mut res = vec![];
        // To avoid colliding with the HexagonLines, the inner triangles ocupy a radius a bit
        // smaller
        let inner_hexagon_radius_percent = 0.92f32; // XXX: Maybe this can be a field?
        let coords = super::gen_hex_grid_positions(self.size_info, self.radius);
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
            0.0f32, // x
            0.0f32, // y
            0.0f32, // z
            <f32 as From<_>>::from(self.vertex_color.r) / 255.,
            <f32 as From<_>>::from(self.vertex_color.g) / 255.,
            <f32 as From<_>>::from(self.vertex_color.b) / 255.,
            0.0f32, // a
        ];
        let sides = vec![
            0f32, // x
            0f32, // y
            0f32, // z
            <f32 as From<_>>::from(self.vertex_color.r) / 255.,
            <f32 as From<_>>::from(self.vertex_color.g) / 255.,
            <f32 as From<_>>::from(self.vertex_color.b) / 255.,
            self.alpha,
        ];
        let mut east = sides.clone();
        let mut northeast = sides.clone();
        let mut northwest = sides.clone();
        let mut west = sides.clone();
        let mut southwest = sides.clone();
        let mut southeast = sides;
        for coord in coords.iter() {
            // The first pair of coordinates are the center of the hexagon
            center[0] = self.size_info.scale_x(coord.x);
            center[1] = self.size_info.scale_y(coord.y);
            let hexagon_vertices = super::gen_2d_hexagon_vertices(
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

    pub fn tick(&mut self, time: f32) {
        let sn = time.cos() as f64 * 0.01;
        // Iterate over xyzrgba
        for (idx, chunk) in self.vecs.chunks_exact_mut(7).enumerate() {
            if idx % 3 != 0 {
                let chunk_z = chunk[2] as f64;
                chunk[2] += self.noise.get([sn * chunk_z, 0.0, 1.0]) as f32;
            }
        }
    }
}
