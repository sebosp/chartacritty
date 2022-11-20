//! Hexagon Point Background decoration
//! TODO: Use nannou
use crate::term::SizeInfo;
use crate::term::color::Rgb;
use log::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonPointBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,

    pub alpha: f32,

    #[serde(default)]
    size_info: SizeInfo,

    radius: f32,

    #[serde(default)]
    pub animated: bool,

    /// Now and then, certain points will be chosen to be moved horizontally
    #[serde(default)]
    chosen_vertices: Vec<usize>,

    /// Every these many seconds, chose new points to move
    #[serde(default)]
    update_interval_s: i32,

    /// At which epoch ms in time the point animation should start
    #[serde(default)]
    start_animation_ms: f32,

    /// The duration of the animation
    #[serde(default)]
    animation_duration_ms: f32,

    /// The horizontal distance that should be covered during the animation time
    #[serde(default)]
    animation_offset: f32,

    /// The next epoch in which the horizontal move is active
    #[serde(default)]
    next_update_epoch: f32,

    /// The OpenGL representation of the dots for a buffer array object
    #[serde(default)]
    pub vecs: Vec<f32>,
}

impl Default for HexagonPointBackground {
    fn default() -> Self {
        let epoch = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let start_animation_ms = epoch.as_secs_f32() + epoch.subsec_millis() as f32 / 1000f32;
        let animation_duration_ms = 2000f32;
        let mut res = HexagonPointBackground {
            color: Rgb { r: 25, g: 88, b: 167 },
            alpha: 0.4f32,
            size_info: SizeInfo::default(),
            radius: 100f32,
            chosen_vertices: vec![],
            update_interval_s: 15i32,
            start_animation_ms,
            animation_duration_ms,
            animation_offset: 0.0f32,
            next_update_epoch: start_animation_ms + animation_duration_ms,
            vecs: vec![],
            animated: true,
        };
        res.update_opengl_vecs();
        res.choose_random_vertices();
        res.init_timers(Instant::now());
        res
    }
}

impl HexagonPointBackground {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        info!("HexagonPointBackground::new()");
        let mut res = HexagonPointBackground {
            // shader_fragment_path: String::from("Unimplemented"),
            // shader_vertex_path: String::from("Unimplemented"),
            color,
            alpha,
            size_info,
            radius,
            vecs: vec![],
            chosen_vertices: vec![],
            update_interval_s: 0i32,
            start_animation_ms: 0.0f32,
            animation_duration_ms: 0.0f32,
            animation_offset: 0f32, // This is calculated on the `update_opengl_vecs` function
            next_update_epoch: 0.0,
            animated: true,
        };
        res.update_opengl_vecs();
        res.choose_random_vertices();
        res.init_timers(Instant::now());
        res
    }

    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self, time: Instant) {
        info!("HexagonPointBackground::init_timers()");
        self.update_interval_s = 15i32;
        self.animation_duration_ms = 2000f32;
        let elapsed = time.elapsed();
        let curr_secs = elapsed.as_secs_f32() + elapsed.subsec_millis() as f32 / 1000f32;
        self.start_animation_ms = (curr_secs / self.update_interval_s as f32).floor();
        self.next_update_epoch = 0.0f32 + (self.update_interval_s as f32);
    }

    /// `choose_random_vertices` should be called once a new animation should take place,
    /// it selects new vertices to animate from the hexagons
    pub fn choose_random_vertices(&mut self) {
        // SEB TODO: There seems to be bug where it hangs in this function after 1 or two
        // minutes...
        // Of the six vertices of x,y values, we only care about one of them, the top left.
        let total_hexagons = self.vecs.len() / 6usize / 2usize;
        // Let's animate 1/5 of the top-left hexagons
        let random_vertices_to_choose = (total_hexagons / 5usize) as usize;
        info!(
            "HexagonPointBackground::choose_random_vertices INIT. Total hexagons: {}, \
             random_vertices_to_choose: {}",
            total_hexagons, random_vertices_to_choose
        );
        // Testing, TODO: remove
        // self.chosen_vertices = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
        // 18]; return;
        let mut rng = rand::thread_rng();
        let mut current_vertex = 0;
        while current_vertex <= random_vertices_to_choose {
            let new_vertex = rng.gen_range(0, total_hexagons);
            if self.chosen_vertices.contains(&new_vertex) {
                continue;
            }
            if self.chosen_vertices.len() < current_vertex {
                self.chosen_vertices[current_vertex] = new_vertex;
            } else {
                self.chosen_vertices.push(new_vertex);
            }
            current_vertex += 1;
        }
        info!("HexagonPointBackground::choose_random_vertices DONE");
    }

    pub fn update_opengl_vecs(&mut self) {
        let mut hexagons = vec![];
        let coords = gen_hex_grid_positions(self.size_info, self.radius);
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
        if !self.animated {
            return;
        }
        // The time is received as seconds.millis, let's transform all to ms
        let time_ms = time * 1000f32;
        info!(
            "tick time: {}, as f32: {}, start_animation_ms: {}, animation_duration_ms: {}, \
             animation_offset: {}, update_interval_s: {}, next_update_epoch: {}",
            time,
            time as f32,
            self.start_animation_ms,
            self.animation_duration_ms,
            self.animation_offset,
            self.update_interval_s,
            self.next_update_epoch
        );
        if time_ms > self.start_animation_ms
            && time_ms < self.start_animation_ms + self.animation_duration_ms
        {
            let current_animation_ms = time_ms - self.start_animation_ms;
            // Given this much time, the animation should have added this much offset
            let current_ms_x_offset = (current_animation_ms as f32
                / self.animation_duration_ms as f32)
                * self.animation_offset;
            info!("tick in range of animation, x_offset should be: {}", current_ms_x_offset);
            for curr_vertex in &self.chosen_vertices {
                // This vertex is static, so we can use it as a start
                let bottom_left_vertex_offset_idx = (curr_vertex * 6usize * 2usize) + 8usize;
                // This is the vertex we will move horizontally
                let top_left_vertex_offset_idx = (curr_vertex * 6usize * 2usize) + 4usize;
                if top_left_vertex_offset_idx > self.vecs.len()
                    || bottom_left_vertex_offset_idx > self.vecs.len()
                {
                    warn!("The number of hexagons may have been decreased on window resize");
                } else {
                    self.vecs[top_left_vertex_offset_idx] =
                        self.vecs[bottom_left_vertex_offset_idx] + current_ms_x_offset;
                }
            }
        } else if time_ms > self.next_update_epoch {
            info!("tick to update next animation");
            // Schedule the next update to be in the future
            self.next_update_epoch += self.update_interval_s as f32 * 1000f32;
            // The animation is over, we can reset the position of the chosen vertices
            for curr_vertex in &self.chosen_vertices {
                // This vertex is static, so we can use it as a start
                let bottom_left_vertex_offset_idx = (curr_vertex * 6usize * 2usize) + 8usize;
                // This is the vertex we will move horizontally
                let top_left_vertex_offset_idx = (curr_vertex * 6usize * 2usize) + 4usize;
                self.vecs[top_left_vertex_offset_idx] = self.vecs[bottom_left_vertex_offset_idx];
            }
            self.start_animation_ms += self.update_interval_s as f32 * 1000f32;
            self.choose_random_vertices();
        }
    }
}

