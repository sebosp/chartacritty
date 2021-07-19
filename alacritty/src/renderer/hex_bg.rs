use std::mem;

use crate::gl;
use crate::gl::types::*;
use crate::renderer;

static HXBG_SHADER_F: &str = include_str!("../../res/hex_bg.f.glsl");
static HXBG_SHADER_V: &str = include_str!("../../res/hex_bg.v.glsl");

#[derive(Debug)]
pub struct HexBgRenderer {
    // GL buffer objects.
    pub vao: GLuint,
    pub vbo: GLuint,

    program: HexagonShaderProgram,

    vertices: Vec<f32>,
}

impl HexBgRenderer {
    pub fn new() -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let program = HexagonShaderProgram::new()?;

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

    pub fn draw(&mut self, opengl_data: &[f32]) {
        unsafe {
            // Bind VAO to enable vertex attribute slots.
            gl::BindVertexArray(self.vao);

            // Bind VBO only once for buffer data upload only.
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

            // Swap program
            gl::UseProgram(self.program.id);
        }

        // TODO: put this somewhere before DrawArrays
        // self.hex_bg_program.set_epoch_millis(0.0f32);

        unsafe {
            // Load vertex data into array buffer
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (mem::size_of::<f32>() * opengl_data.len()) as _,
                opengl_data.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            // Draw the incoming array, opengl_data contains:
            // [2(x,y) + 4(r,g,b,a) ] -> 6
            gl::DrawArrays(gl::TRIANGLES, 0, (opengl_data.len() / 6usize) as i32);

            // Disable program
            gl::UseProgram(0);

            // Reset buffer bindings to nothing.
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
    }
}

/// Hexagon Background Shader Program
///
/// Uniforms are prefixed with "u"
#[derive(Debug)]
pub struct HexagonShaderProgram {
    // Program id,
    id: GLuint,
    /// The time uniform to be used to change opacity of different regions
    u_epoch_millis: GLint,
}

impl HexagonShaderProgram {
    pub fn new() -> Result<Self, renderer::ShaderCreationError> {
        let vertex_shader = renderer::create_shader(gl::VERTEX_SHADER, HXBG_SHADER_V)?;
        let fragment_shader = renderer::create_shader(gl::FRAGMENT_SHADER, HXBG_SHADER_F)?;
        let program = renderer::create_program(vertex_shader, fragment_shader)?;

        unsafe {
            gl::DeleteShader(fragment_shader);
            gl::DeleteShader(vertex_shader);
            gl::UseProgram(program);
        }

        // get uniform locations
        let u_epoch_millis =
            unsafe { gl::GetUniformLocation(program, b"epoch_millis\0".as_ptr() as *const _) };

        let shader = HexagonShaderProgram { id: program, u_epoch_millis };

        unsafe { gl::UseProgram(0) }

        Ok(shader)
    }

    //    fn set_epoch_millis(&self, epoch_millis: f32) {
    // unsafe {
    // gl::Uniform1f(self.u_epoch_millis, epoch_millis);
    // }
    // }
}

impl Drop for HexagonShaderProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}
