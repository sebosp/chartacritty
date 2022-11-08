use crate::charts::Value2D;
use crate::term::color::Rgb;
use crate::term::SizeInfo;
use log::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::time::UNIX_EPOCH;
use nannou::draw;
use nannou::glam::Vec2;
use nannou::prelude::*;
use nannou::draw::renderer::{RenderPrimitive, GlyphCache};
use lyon::tessellation::{FillTessellator, StrokeTessellator};

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
    /// An array of active decorators
    pub decorators: Vec<DecorationTypes>,

    /// The time at which config was initialized
    #[serde(skip)]
    init_start: Option<Instant>,
}

impl DecorationsConfig {
    /// `set_size_info` iterates over the enabled decorations and calls the resize method for any
    /// registered decorators
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        info!("DecorationsConfig::set_size_info()");
        for decor in self.decorators.iter_mut() {
            info!("DecorationsConfig:: iter_mut: {:?}", decor);
            decor.set_size_info(size_info);
        }
    }

    /// `from_optional_decor_config` transforms an optional DecorationsConfig into an
    /// DecorationsConfig with resized vector items
    pub fn optional_decor_to_sized(config_decorations: Option<Self>, size_info: SizeInfo) -> Self {
        match config_decorations {
            Some(mut decors) => {
                decors.set_size_info(size_info);
                decors
            },
            None => {
                info!("No decorations to size");
                DecorationsConfig::default()
            },
        }
    }

    /// `tick` calls the underlying decorators to update decorations that depend on time
    /// such as animations
    pub fn tick(&mut self) {
        let mut time_ms = 0.0f32;
        if let Some(val) = self.init_start {
            let elapsed = val.elapsed();
            time_ms = elapsed.as_secs_f32() + elapsed.subsec_millis() as f32 / 1000f32;
        }
        for decor in self.decorators.iter_mut() {
            decor.tick(time_ms);
        }
    }

    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self) {
        let curr_time = Instant::now();
        self.init_start = Some(curr_time);
        for decor in self.decorators.iter_mut() {
            decor.init_timers(curr_time);
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
            },
            DecorationTypes::Points(ref mut hexagon_points) => {
                hexagon_points.set_size_info(size_info);
            },
            DecorationTypes::Lines(ref mut hexagon_lines) => {
                hexagon_lines.set_size_info(size_info);
            },
            DecorationTypes::None => {
                unreachable!("Attempting to update decorations on None variant");
            },
        }
    }

    /// `tick` is called every time there is a draw request for the terminal
    pub fn tick(&mut self, time: f32) {
        if let DecorationTypes::Points(ref mut hexagon_points) = self {
            hexagon_points.tick(time);
        }
    }

    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self, time: Instant) {
        if let DecorationTypes::Points(ref mut hexagon_points) = self {
            hexagon_points.init_timers(time);
        }
    }
}

/// DecorationLines represents lines of x,y points.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", content = "props")]
pub enum DecorationLines {
    Hexagon(HexagonLineBackground),
    TreeSilhoutte(TreeSilhoutteLineBackground),
}

impl DecorationLines {
    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationLines::Hexagon(ref mut hex_lines) => {
                hex_lines.size_info = size_info;
                hex_lines.update_opengl_vecs();
            },
            DecorationLines::TreeSilhoutte(ref mut tree_lines) => {
                tree_lines.size_info = size_info;
                tree_lines.update_opengl_vecs();
            },
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
    /// `init_timers` will initialize times/epochs in the animation to some chosen defaults
    pub fn init_timers(&mut self, time: Instant) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.init_timers(time);
            },
        }
    }

    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.size_info = size_info;
                hex_points.update_opengl_vecs();
                hex_points.choose_random_vertices();
            },
        }
    }

    pub fn tick(&mut self, time: f32) {
        match self {
            DecorationPoints::Hexagon(ref mut hex_points) => {
                hex_points.tick(time);
            },
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
            },
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
    // hexagon_line_background.update_opengl_vecs();
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct TreeSilhoutteLineBackground {
    pub color: Rgb,
    pub alpha: f32,
    #[serde(default)]
    size_info: SizeInfo,
    radius: f32,
    #[serde(default)]
    pub vecs: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonLineBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub color: Rgb,
    pub alpha: f32,
    #[serde(default)]
    size_info: SizeInfo,
    radius: f32,
    #[serde(default)]
    pub vecs: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HexagonTriangleBackground {
    // shader_vertex_path: String,
    // shader_fragment_path: String,
    pub vertex_color: Rgb,
    pub center_color: Rgb,
    pub alpha: f32,
    #[serde(default)]
    size_info: SizeInfo,
    radius: f32,
    #[serde(default)]
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
        // To avoid colliding with the HexagonLines, the inner triangles ocupy a radius a bit
        // smaller
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
            <f32 as From<_>>::from(self.center_color.r) / 255.,
            <f32 as From<_>>::from(self.center_color.g) / 255.,
            <f32 as From<_>>::from(self.center_color.b) / 255.,
            0.0f32,
        ];
        let sides = vec![
            0f32, // x
            0f32, // y
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

// TODO: Move this somewhere sensical...
fn new_glyph_cache() -> GlyphCache {
    let size = nannou::draw::Renderer::DEFAULT_GLYPH_CACHE_SIZE;
    let scale_tolerance = nannou::draw::Renderer::DEFAULT_GLYPH_CACHE_SCALE_TOLERANCE;
    let position_tolerance = nannou::draw::Renderer::DEFAULT_GLYPH_CACHE_POSITION_TOLERANCE;
    let [w, h] = size;
    let cache = nannou::text::GlyphCache::builder()
        .dimensions(w, h)
        .scale_tolerance(scale_tolerance)
        .position_tolerance(position_tolerance)
        .build()
        .into();
    let pixel_buffer = vec![0u8; w as usize * h as usize];
    let requires_upload = false;
    GlyphCache {
        cache,
        pixel_buffer,
        requires_upload,
    }
}

impl TreeSilhoutteLineBackground {
    pub fn new(color: Rgb, alpha: f32, size_info: SizeInfo, radius: f32) -> Self {
        Self { color, alpha, size_info, radius, vecs: vec![] }
    }

    pub fn update_opengl_vecs(&mut self) {
        let coords = background_fill_hexagon_positions(self.size_info, self.radius);
        // We need to find the center hexagon. Which should be in the middle of the array
        let center_idx: usize = coords.len() / 2;
        let coord = coords[center_idx];
        self.vecs = self.gen_tree_vertices(coord.x, coord.y);
    }

    /// `gen_tree_vertices` Returns the vertices for an tree created at center x,y with a
    /// specific radius
    pub fn gen_tree_vertices(&self, x: f32, y: f32) -> Vec<f32> {
        let x_60_degrees_offset = COS_60 * self.radius;
        let y_60_degrees_offset = SIN_60 * self.radius;
        let mut draw = draw::Draw::default();
        let mut mesh = draw::Mesh::default();
        draw.ellipse()
            .x_y(0., 0.)
        .radius(self.radius)
        .color(RED);
        /*draw.tri()
            .points(
                [
            self.size_info.scale_x(x),
            self.size_info.scale_y(y),
                ],
                [
            self.size_info.scale_x(x + x_60_degrees_offset),
            self.size_info.scale_y(y + y_60_degrees_offset),
                ],
                [
            self.size_info.scale_x(x + x_60_degrees_offset),
            self.size_info.scale_y(y),
                ]
            )
            .rotate(30f32)
            .color(VIOLET);*/
        draw.finish_remaining_drawings();
        // Trying to adapt nannou crate nannou/src/draw/renderer/mod.rs `fill()` function
        // Construct the glyph cache.
        let mut glyph_cache = new_glyph_cache();

        let mut fill_tessellator = FillTessellator::new();
        let mut stroke_tessellator = StrokeTessellator::new();
        // Keep track of context changes.
        let curr_ctxt = draw::Context::default();
        let draw_cmds: Vec<_> = draw.drain_commands().collect();
        let scale_factor = 1.;
        for cmd in draw_cmds {
            match cmd {
                draw::DrawCommand::Primitive(prim) => {
                    // Info required during rendering.
                    let ctxt = draw::renderer::RenderContext {
                        intermediary_mesh: &Default::default(),
                        path_event_buffer: &[],
                        path_points_colored_buffer: &[],
                        path_points_textured_buffer: &[],
                        text_buffer: Default::default(),
                        theme: &Default::default(),
                        transform: &curr_ctxt.transform,
                        fill_tessellator: &mut fill_tessellator,
                        stroke_tessellator: &mut stroke_tessellator,
                        glyph_cache: &mut glyph_cache,
                        output_attachment_size: Vec2::new(2., 2.),
                        output_attachment_scale_factor: scale_factor,
                    };

                    // Render the primitive.
                    let _render = prim.render_primitive(ctxt, &mut mesh);
                    let mut res = vec![];
                    for vx in mesh.vertices() {
                        res.push(self.size_info.scale_x(vx.x));
                        res.push(self.size_info.scale_y(vx.y));
                    }
                    tracing::warn!("nannou output: {:?}", res);
                    return res;
                }
                unhandled @ _ => {
                    tracing::warn!("Unknown DrawCommand: {:?}", unhandled);
                }
            }
        }
        // Order of vertices:
        //    3-------2
        //   /         \
        //  /           \
        // 4      .      1
        //  \           /
        //   \         /
        //    5-------6
        // |     width   |
        // Mid right - mid left
        vec![
            // Bottom Right
            self.size_info.scale_x(x + x_60_degrees_offset),
            self.size_info.scale_y(y - y_60_degrees_offset),
            // Lower bottom Right trunk
            self.size_info.scale_x(x + x_60_degrees_offset - (x_60_degrees_offset / 6f32)),
            self.size_info.scale_y(y - y_60_degrees_offset + (y_60_degrees_offset / 4f32)),
            // Mid-Lower bottom Right trunk
            self.size_info.scale_x(x + x_60_degrees_offset - (x_60_degrees_offset / 4f32)),
            self.size_info.scale_y(y - y_60_degrees_offset + (y_60_degrees_offset / 2f32)),
            /* Mid right:
            size_info.scale_x(x + self.radius),
            size_info.scale_y(y),
            // Top right:
            size_info.scale_x(x + x_60_degrees_offset),
            size_info.scale_y(y + y_60_degrees_offset),
            // Top left
            size_info.scale_x(x - x_60_degrees_offset),
            size_info.scale_y(y + y_60_degrees_offset),
            // Mid left:
            size_info.scale_x(x - self.radius),
            size_info.scale_y(y),
            // Bottom left
            size_info.scale_x(x - x_60_degrees_offset),
            size_info.scale_y(y - y_60_degrees_offset),*/
        ]
    }
}

pub fn parse_svg_path() -> Vec<f32> {
    // tree is created by hand on some svg editor, let's make an SVG Path parser to create the
    // lines, this should be read from the config file
    let res = vec![];
    let _tree = "M 8 8 L 7 7 L 7 6 L 7 5 L 6 4 L 6 2 L 8 2 L 9 1 L 7 1 L 8 0 L 5 -1 L 5 1 L 2 -1 \
                 L 3 1 L 3 2 L 2 2 L 1 3 L 2 3 L 3 3 L 3 4 L 3 4 L 4 5 L 5 6 L 4 7 L 3 8";
    res
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
        // SEB TODO: There seems to be bug where it hanngs in this function after 1 or two
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
