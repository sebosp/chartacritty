use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fmt, ptr};

use ahash::RandomState;
use crossfont::Metrics;
use glutin::context::{ContextApi, GlContext, PossiblyCurrentContext};
use glutin::display::{GetGlDisplay, GlDisplay};
use log::{LevelFilter, debug, info};
use unicode_width::UnicodeWidthChar;

use alacritty_terminal::index::Point;
use alacritty_terminal::term::cell::Flags;

use crate::config::debug::RendererPreference;
use crate::display::SizeInfo;
use crate::display::color::Rgb;
use crate::display::content::RenderableCell;
use crate::gl;
use crate::renderer::charts::ChartRenderer;
use crate::renderer::hex_bg::HexBgRenderer;
use crate::renderer::rects::{RectRenderer, RenderRect};
use crate::renderer::shader::ShaderError;

pub mod charts;
pub mod hex_bg;
pub mod platform;
pub mod rects;
mod shader;
mod text;

pub use text::{GlyphCache, LoaderApi};

use shader::ShaderVersion;
use text::{Gles2Renderer, Glsl3Renderer, TextRenderer};

/// Whether the OpenGL functions have been loaded.
pub static GL_FUNS_LOADED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub enum Error {
    /// Shader error.
    Shader(ShaderError),

    /// Other error.
    Other(String),
}

#[derive(Debug, Clone)]
pub enum DrawArrayMode {
    Points,
    LineStrip,
    LineLoop,
    // GlTriangleFan,
    // GlLines,
    // GlTriangleStrip,
    GlTriangles,
    // GlQuadStrip, // Unsupported
    // GlQuads,
    // GlPolygon,
}

impl From<DrawArrayMode> for u32 {
    fn from(src: DrawArrayMode) -> Self {
        // Translate our enum to opengl enum, maybe this can be ommitted?
        // Maybe we can extend the enum with custom classes that end up being like this.
        // So then it should become a trait
        match src {
            DrawArrayMode::Points => gl::POINTS,
            DrawArrayMode::LineStrip => gl::LINE_STRIP,
            DrawArrayMode::LineLoop => gl::LINE_LOOP,
            // DrawArrayMode::GlTriangleFan => gl::TRIANGLE_FAN,
            // DrawArrayMode::GlLines => gl::LINES,
            // DrawArrayMode::GlTriangleStrip => gl::TRIANGLE_STRIP,
            DrawArrayMode::GlTriangles => gl::TRIANGLES,
            // DrawArrayMode::GlQuadStrip => gl::QUAD_STRIP, // Unsupported?
            // DrawArrayMode::GlQuads => gl::QUADS,
            // DrawArrayMode::GlPolygon => gl::POLYGON_MODE,
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Shader(err) => err.source(),
            Error::Other(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Shader(err) => {
                write!(f, "There was an error initializing the shaders: {err}")
            },
            Error::Other(err) => {
                write!(f, "{err}")
            },
        }
    }
}

impl From<ShaderError> for Error {
    fn from(val: ShaderError) -> Self {
        Error::Shader(val)
    }
}

impl From<String> for Error {
    fn from(val: String) -> Self {
        Error::Other(val)
    }
}

#[derive(Debug)]
enum TextRendererProvider {
    Gles2(Gles2Renderer),
    Glsl3(Glsl3Renderer),
}

#[derive(Debug)]
pub struct Renderer {
    text_renderer: TextRendererProvider,
    rect_renderer: RectRenderer,
    chart_renderer: ChartRenderer,
    hex_bg_renderer: HexBgRenderer,
    robustness: bool,
}

/// Wrapper around gl::GetString with error checking and reporting.
fn gl_get_string(
    string_id: gl::types::GLenum,
    description: &str,
) -> Result<Cow<'static, str>, Error> {
    unsafe {
        let string_ptr = gl::GetString(string_id);
        match gl::GetError() {
            gl::NO_ERROR if !string_ptr.is_null() => {
                Ok(CStr::from_ptr(string_ptr as *const _).to_string_lossy())
            },
            gl::INVALID_ENUM => {
                Err(format!("OpenGL error requesting {description}: invalid enum").into())
            },
            error_id => Err(format!("OpenGL error {error_id} requesting {description}").into()),
        }
    }
}

impl Renderer {
    /// Create a new renderer.
    ///
    /// This will automatically pick between the GLES2 and GLSL3 renderer based on the GPU's
    /// supported OpenGL version.
    pub fn new(
        context: &PossiblyCurrentContext,
        renderer_preference: Option<RendererPreference>,
    ) -> Result<Self, Error> {
        // We need to load OpenGL functions once per instance, but only after we make our context
        // current due to WGL limitations.
        if !GL_FUNS_LOADED.swap(true, Ordering::Relaxed) {
            let gl_display = context.display();
            gl::load_with(|symbol| {
                let symbol = CString::new(symbol).unwrap();
                gl_display.get_proc_address(symbol.as_c_str()).cast()
            });
        }

        let shader_version = gl_get_string(gl::SHADING_LANGUAGE_VERSION, "shader version")?;
        let gl_version = gl_get_string(gl::VERSION, "OpenGL version")?;
        let renderer = gl_get_string(gl::RENDERER, "renderer version")?;

        info!("Running on {renderer}");
        info!("OpenGL version {gl_version}, shader_version {shader_version}");

        // Check if robustness is supported.
        let robustness = Self::supports_robustness();

        let is_gles_context = matches!(context.context_api(), ContextApi::Gles(_));

        // Use the config option to enforce a particular renderer configuration.
        let (use_glsl3, allow_dsb) = match renderer_preference {
            Some(RendererPreference::Glsl3) => (true, true),
            Some(RendererPreference::Gles2) => (false, true),
            Some(RendererPreference::Gles2Pure) => (false, false),
            None => (shader_version.as_ref() >= "3.3" && !is_gles_context, true),
        };

        let (text_renderer, rect_renderer, chart_renderer, hex_bg_renderer) = if use_glsl3 {
            let text_renderer = TextRendererProvider::Glsl3(Glsl3Renderer::new()?);
            let rect_renderer = RectRenderer::new(ShaderVersion::Glsl3)?;
            let chart_renderer = ChartRenderer::new(ShaderVersion::Glsl3)?;
            let hex_bg_renderer = HexBgRenderer::new(ShaderVersion::Glsl3)?;
            (text_renderer, rect_renderer, chart_renderer, hex_bg_renderer)
        } else {
            let text_renderer =
                TextRendererProvider::Gles2(Gles2Renderer::new(allow_dsb, is_gles_context)?);
            let rect_renderer = RectRenderer::new(ShaderVersion::Gles2)?;
            let chart_renderer = ChartRenderer::new(ShaderVersion::Gles2)?;
            let hex_bg_renderer = HexBgRenderer::new(ShaderVersion::Gles2)?;
            (text_renderer, rect_renderer, chart_renderer, hex_bg_renderer)
        };

        // Enable debug logging for OpenGL as well.
        if log::max_level() >= LevelFilter::Debug && GlExtensions::contains("GL_KHR_debug") {
            debug!("Enabled debug logging for OpenGL");
            unsafe {
                gl::Enable(gl::DEBUG_OUTPUT);
                gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);
                gl::DebugMessageCallback(Some(gl_debug_log), ptr::null_mut());
            }
        }

        Ok(Self { text_renderer, rect_renderer, chart_renderer, hex_bg_renderer, robustness })
    }

    pub fn draw_cells<I: Iterator<Item = RenderableCell>>(
        &mut self,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
        cells: I,
    ) {
        match &mut self.text_renderer {
            TextRendererProvider::Gles2(renderer) => {
                renderer.draw_cells(size_info, glyph_cache, cells)
            },
            TextRendererProvider::Glsl3(renderer) => {
                renderer.draw_cells(size_info, glyph_cache, cells)
            },
        }
    }

    /// Draw a string in a variable location. Used for printing the render timer, warnings and
    /// errors.
    pub fn draw_string(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        string_chars: impl Iterator<Item = char>,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
    ) {
        let mut wide_char_spacer = false;
        let cells = string_chars.enumerate().filter_map(|(i, character)| {
            let flags = if wide_char_spacer {
                wide_char_spacer = false;
                return None;
            } else if character.width() == Some(2) {
                // The spacer is always following the wide char.
                wide_char_spacer = true;
                Flags::WIDE_CHAR
            } else {
                Flags::empty()
            };

            Some(RenderableCell {
                point: Point::new(point.line, point.column + i),
                character,
                extra: None,
                flags,
                bg_alpha: 1.0,
                fg,
                bg,
                underline: fg,
            })
        });

        self.draw_cells(size_info, glyph_cache, cells);
    }

    /* TODO: figure out how to use indices for DrawElements
    // ---------------------
    // Filled Hexagon Setup
    // ---------------------
    // Order of vertices:
    //          N
    //      3-------2
    //     /         \
    //    /           \
    // W 4      0      1 E
    //    \           /
    //     \         /
    //      5-------6
    //          S
    gl::GenBuffers(1, &mut hex_ebo);
    let indices: [u32; 18] = [
        0, 1, 2, // North-East
        0, 2, 3, // North
        0, 3, 4, // North-West
        0, 4, 5, // South-West
        0, 5, 6, // South
        0, 6, 1, // South-East
    ];

    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, hex_ebo);
    gl::BufferData(
        gl::ELEMENT_ARRAY_BUFFER,
        (indices.len() * size_of::<u32>()) as isize,
        indices.as_ptr() as *const _,
        gl::STATIC_DRAW,
    );*/

    pub fn with_loader<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(LoaderApi<'_>) -> T,
    {
        match &mut self.text_renderer {
            TextRendererProvider::Gles2(renderer) => renderer.with_loader(func),
            TextRendererProvider::Glsl3(renderer) => renderer.with_loader(func),
        }
    }

    pub fn prepare_rect_rendering_state(size_info: &SizeInfo) {
        // Prepare rect rendering state.
        unsafe {
            // Remove padding from viewport.
            gl::Viewport(0, 0, size_info.width() as i32, size_info.height() as i32);
            gl::BlendFuncSeparate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::SRC_ALPHA, gl::ONE);
        }
    }

    pub fn activate_regular_state(&self, size_info: &SizeInfo) {
        // Activate regular state again.
        unsafe {
            // Reset blending strategy.
            gl::BlendFunc(gl::SRC1_COLOR, gl::ONE_MINUS_SRC1_COLOR);

            // Restore viewport with padding.
            self.set_viewport(size_info);
        }
    }

    /// Draw all rectangles simultaneously to prevent excessive program swaps.
    pub fn draw_rects(&mut self, size_info: &SizeInfo, metrics: &Metrics, rects: Vec<RenderRect>) {
        if rects.is_empty() {
            return;
        }

        Self::prepare_rect_rendering_state(size_info);

        self.rect_renderer.draw(size_info, metrics, rects);

        self.activate_regular_state(size_info);
    }

    /// `draw_xyzrgba_array` draws an array of triangles with properties (x,y,z,r,g,b,a)
    pub fn draw_xyzrgba_vertices(
        &mut self,
        size_info: &SizeInfo,
        opengl_data: &[f32],
        mode: DrawArrayMode,
        time_secs_with_ms: f32,
    ) {
        // This function expects a vector that contains 7 data points per vertex:
        // 3 are x,y,z position and the other 4 are the r,g,b,a
        // let opengl_data = vec![
        // 0.5f32, 0.5f32, 0.0f32 // x, y, z
        // 1.0f32, 0.0f32, 0.0f32, 1.0f32, // RGBA
        // 0.8f32, 0.8f32, 0.0f32 // x, y, z
        // 0.0f32, 1.0f32, 0.0f32, 1.0f32, // RGBA
        // 0.7f32, 0.3f32, 0.0f32 // x, y, z
        // 0.0f32, 0.0f32, 1.0f32, 1.0f32, // RGBA
        // ];
        Self::prepare_rect_rendering_state(size_info);

        self.hex_bg_renderer.draw(opengl_data, mode.into(), size_info, time_secs_with_ms);

        self.activate_regular_state(size_info);
    }

    /// `draw_array` draws a vec made of 2D values in a specific mode
    pub fn draw_array(
        &mut self,
        size_info: &SizeInfo,
        opengl_vecs: &[f32],
        color: Rgb,
        alpha: f32,
        mode: DrawArrayMode,
    ) {
        match mode {
            DrawArrayMode::Points => (),
            _ =>
            // All types, except for Points, need at least 2 x,y coordinates to work on
            {
                if opengl_vecs.len() < 4 {
                    return;
                }
            },
        };
        let mut opengl_data_with_color: Vec<f32> = Vec::with_capacity((opengl_vecs.len() / 2) * 6);
        for position in opengl_vecs.chunks(2) {
            opengl_data_with_color.push(position[0]);
            opengl_data_with_color.push(position[1]);
            opengl_data_with_color.push(f32::from(color.r) / 255.);
            opengl_data_with_color.push(f32::from(color.g) / 255.);
            opengl_data_with_color.push(f32::from(color.b) / 255.);
            opengl_data_with_color.push(alpha);
        }

        Self::prepare_rect_rendering_state(size_info);

        self.chart_renderer.draw(&opengl_data_with_color, mode.into());

        self.activate_regular_state(size_info);
    }

    /// Fill the window with `color` and `alpha`.
    pub fn clear(&self, color: Rgb, alpha: f32) {
        unsafe {
            gl::ClearColor(
                (f32::from(color.r) / 255.0).min(1.0) * alpha,
                (f32::from(color.g) / 255.0).min(1.0) * alpha,
                (f32::from(color.b) / 255.0).min(1.0) * alpha,
                alpha,
            );
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    /// Get the context reset status.
    pub fn was_context_reset(&self) -> bool {
        // If robustness is not supported, don't use its functions.
        if !self.robustness {
            return false;
        }

        let status = unsafe { gl::GetGraphicsResetStatus() };
        if status == gl::NO_ERROR {
            false
        } else {
            let reason = match status {
                gl::GUILTY_CONTEXT_RESET_KHR => "guilty",
                gl::INNOCENT_CONTEXT_RESET_KHR => "innocent",
                gl::UNKNOWN_CONTEXT_RESET_KHR => "unknown",
                _ => "invalid",
            };

            info!("GPU reset ({reason})");

            true
        }
    }

    fn supports_robustness() -> bool {
        let mut notification_strategy = 0;
        if GlExtensions::contains("GL_KHR_robustness") {
            unsafe {
                gl::GetIntegerv(gl::RESET_NOTIFICATION_STRATEGY_KHR, &mut notification_strategy);
            }
        } else {
            notification_strategy = gl::NO_RESET_NOTIFICATION_KHR as gl::types::GLint;
        }

        if notification_strategy == gl::LOSE_CONTEXT_ON_RESET_KHR as gl::types::GLint {
            info!("GPU reset notifications are enabled");
            true
        } else {
            info!("GPU reset notifications are disabled");
            false
        }
    }

    pub fn finish(&self) {
        unsafe {
            gl::Finish();
        }
    }

    /// Set the viewport for cell rendering.
    #[inline]
    pub fn set_viewport(&self, size: &SizeInfo) {
        unsafe {
            gl::Viewport(
                size.padding_x() as i32,
                size.padding_y() as i32,
                size.width() as i32 - 2 * size.padding_x() as i32,
                size.height() as i32 - 2 * size.padding_y() as i32,
            );
        }
    }

    /// Resize the renderer.
    pub fn resize(&self, size_info: &SizeInfo) {
        self.set_viewport(size_info);
        match &self.text_renderer {
            TextRendererProvider::Gles2(renderer) => renderer.resize(size_info),
            TextRendererProvider::Glsl3(renderer) => renderer.resize(size_info),
        }
    }
}

struct GlExtensions;

impl GlExtensions {
    /// Check if the given `extension` is supported.
    ///
    /// This function will lazily load OpenGL extensions.
    fn contains(extension: &str) -> bool {
        static OPENGL_EXTENSIONS: OnceLock<HashSet<&'static str, RandomState>> = OnceLock::new();

        OPENGL_EXTENSIONS.get_or_init(Self::load_extensions).contains(extension)
    }

    /// Load available OpenGL extensions.
    fn load_extensions() -> HashSet<&'static str, RandomState> {
        unsafe {
            let extensions = gl::GetString(gl::EXTENSIONS);

            if extensions.is_null() {
                let mut extensions_number = 0;
                gl::GetIntegerv(gl::NUM_EXTENSIONS, &mut extensions_number);

                (0..extensions_number as gl::types::GLuint)
                    .flat_map(|i| {
                        let extension = CStr::from_ptr(gl::GetStringi(gl::EXTENSIONS, i) as *mut _);
                        extension.to_str()
                    })
                    .collect()
            } else {
                match CStr::from_ptr(extensions as *mut _).to_str() {
                    Ok(ext) => ext.split_whitespace().collect(),
                    Err(_) => Default::default(),
                }
            }
        }
    }
}

extern "system" fn gl_debug_log(
    _: gl::types::GLenum,
    _: gl::types::GLenum,
    _: gl::types::GLuint,
    _: gl::types::GLenum,
    _: gl::types::GLsizei,
    msg: *const gl::types::GLchar,
    _: *mut std::os::raw::c_void,
) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };
    debug!("[gl_render] {msg}");
}
