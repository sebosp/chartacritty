use std::mem;

use alacritty_terminal::term::color::Rgb;
use alacritty_terminal::term::SizeInfo;

use crate::gl;
use crate::gl::types::*;
use crate::renderer;

static CHRT_SHADER_F: &str = include_str!("../../res/rect.f.glsl");
static CHRT_SHADER_V: &str = include_str!("../../res/rect.v.glsl");

#[derive(Debug)]
pub struct ChartRenderer {
    // GL buffer objects.
    pub vao: GLuint,
    pub vbo: GLuint,

    program: ChartsShaderProgram,

    vertices: Vec<f32>,
}

impl ChartRenderer {
    pub fn new() -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let program = ChartsShaderProgram::new()?;
        unsafe {
            // Allocate buffers.
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            gl::BindVertexArray(vao);

            // VBO binding is not part of VAO itself, but VBO binding is stored in attributes.
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            let mut attribute_offset = 0;

            // Position
            gl::VertexAttribPointer(
                0, // location=0 is the vertex position
                2, // position has 2 values: X, Y
                gl::FLOAT,
                gl::FALSE,
                // [2(x,y) + 4(r,g,b,a) ] -> 6
                (mem::size_of::<f32>() * 6) as i32,
                attribute_offset as *const _,
            );
            gl::EnableVertexAttribArray(0);

            attribute_offset += mem::size_of::<f32>() * 2;

            // Colors
            gl::VertexAttribPointer(
                1, // location=1 is the color
                4, // Color has 4 items, R, G, B, A
                gl::FLOAT,
                gl::FALSE,
                // [2(x,y) + 4(r,g,b,a) ] -> 6
                (mem::size_of::<f32>() * 6) as i32,
                // The colors are offset by 2 (x,y) points
                attribute_offset as *const _,
            );
            gl::EnableVertexAttribArray(1);

            // Reset buffer bindings.
            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }
        Ok(Self { vao, vbo, program, vertices: Vec::new() })
    }

    pub fn draw(&mut self, opengl_vecs: &[f32], color: Rgb, alpha: f32, gl_mode: u32) {
        // TODO: Use the Charts Shader Program (For now a copy of rect)
        unsafe {
            // Setup data and buffers
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

            // Swap program
            gl::UseProgram(self.program.id);

            // Load vertex data into array buffer
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (mem::size_of::<f32>() * opengl_vecs.len()) as _,
                opengl_vecs.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );
        }

        // Color
        self.program.set_color(color, alpha);

        unsafe {
            // Draw the incoming array, opengl_vecs contains 2 points per vertex:
            gl::DrawArrays(gl_mode, 0, (opengl_vecs.len() / 2usize) as i32);

            // Disable program
            gl::UseProgram(0);

            // Reset buffer bindings to nothing.
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
    }
}

/// Charts Shader Program
///
/// Uniforms are prefixed with "u"
#[derive(Debug)]
pub struct ChartsShaderProgram {
    // Program id,
    id: GLuint,
    /// Line color
    u_color: GLint,
}

impl ChartsShaderProgram {
    pub fn new() -> Result<Self, renderer::ShaderCreationError> {
        let vertex_shader = renderer::create_shader(gl::VERTEX_SHADER, CHRT_SHADER_V)?;
        let fragment_shader = renderer::create_shader(gl::FRAGMENT_SHADER, CHRT_SHADER_F)?;
        let program = renderer::create_program(vertex_shader, fragment_shader)?;

        unsafe {
            gl::DeleteShader(fragment_shader);
            gl::DeleteShader(vertex_shader);
            gl::UseProgram(program);
        }

        // get uniform locations
        let u_color = unsafe { gl::GetUniformLocation(program, b"color\0".as_ptr() as *const _) };

        let shader = ChartsShaderProgram { id: program, u_color };

        unsafe { gl::UseProgram(0) }

        Ok(shader)
    }

    fn set_color(&self, color: Rgb, alpha: f32) {
        unsafe {
            gl::Uniform4f(
                self.u_color,
                f32::from(color.r) / 255.,
                f32::from(color.g) / 255.,
                f32::from(color.b) / 255.,
                alpha,
            );
        }
    }
}

impl Drop for ChartsShaderProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}
