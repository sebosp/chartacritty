use std::mem;

use crate::gl;
use crate::gl::types::*;
use crate::renderer;
use crate::renderer::shader::{ShaderError, ShaderProgram, ShaderVersion};

static CHRT_SHADER_F: &str = include_str!("../../res/rect.f.glsl");
static CHRT_SHADER_V: &str = include_str!("../../res/rect.v.glsl");

#[derive(Debug)]
pub struct ChartRenderer {
    // GL buffer objects.
    pub vao: GLuint,
    pub vbo: GLuint,

    program: ChartsShaderProgram,
}

impl ChartRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let program = ChartsShaderProgram::new(shader_version)?;
        unsafe {
            // Allocate buffers.
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            gl::BindVertexArray(vao);

            // VBO binding is not part of VAO itself, but VBO binding is stored in attributes.
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            let mut attribute_offset = 0;

            // Position.
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

            // Color.
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
        Ok(Self { vao, vbo, program })
    }

    pub fn draw(&mut self, opengl_data: &[f32], gl_mode: u32) {
        unsafe {
            // Bind VAO to enable vertex attribute slots.
            gl::BindVertexArray(self.vao);

            // Bind VBO only once for buffer data upload only.
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

            // Swap program
            gl::UseProgram(self.program.id());

            // Load vertex data into array buffer
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (mem::size_of::<f32>() * opengl_data.len()) as _,
                opengl_data.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            // Draw the incoming array, opengl_data contains:
            // [2(x,y) + 4(r,g,b,a) ] -> 6
            gl::DrawArrays(gl_mode, 0, (opengl_data.len() / 6usize) as i32);

            // Disable program.
            gl::UseProgram(0);

            // Reset buffer bindings to nothing.
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
    }
}

/// Charts drawing program.
#[derive(Debug)]
pub struct ChartsShaderProgram {
    // Shader program
    program: ShaderProgram,
}

impl ChartsShaderProgram {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, ShaderError> {
        // XXX: This must be in-sync with fragment shader defines.
        let header: Option<&str> = None;
        let program = ShaderProgram::new(shader_version, header, CHRT_SHADER_V, CHRT_SHADER_F)?;
        Ok(Self { program })
    }

    fn id(&self) -> GLuint {
        self.program.id()
    }
}
