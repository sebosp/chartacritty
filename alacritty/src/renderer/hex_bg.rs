use std::mem;

use crate::gl;
use crate::gl::types::*;
use crate::renderer;
use crate::renderer::shader::{ShaderError, ShaderProgram, ShaderVersion};

static HXBG_SHADER_F: &str = include_str!("../../res/hex_bg.f.glsl");
static HXBG_SHADER_V: &str = include_str!("../../res/hex_bg.v.glsl");

#[derive(Debug)]
pub struct HexBgRenderer {
    // GL buffer objects.
    pub vao: GLuint,
    pub vbo: GLuint,

    program: HexagonShaderProgram,
}

impl HexBgRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let program = HexagonShaderProgram::new(shader_version)?;
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC1_COLOR, gl::ONE_MINUS_SRC1_COLOR);
            gl::Enable(gl::MULTISAMPLE);

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
                3, // position has 3 values: X, Y, Z
                gl::FLOAT,
                gl::FALSE,
                // [3(x,y,z) + 4(r,g,b,a) ] -> 7
                (mem::size_of::<f32>() * 7) as i32,
                attribute_offset as *const _,
            );
            gl::EnableVertexAttribArray(0);
            attribute_offset += mem::size_of::<f32>() * 3;

            // Color.
            gl::VertexAttribPointer(
                1, // location=1 is the color
                4, // Color has 4 items, R, G, B, A
                gl::FLOAT,
                gl::FALSE,
                // [3(x,y,z) + 4(r,g,b,a) ] -> 7
                (mem::size_of::<f32>() * 7) as i32,
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
            // [3(x,y,z) + 4(r,g,b,a) ] -> 7
            gl::DrawArrays(gl_mode, 0, (opengl_data.len() / 7usize) as i32);

            // Disable program.
            gl::UseProgram(0);

            // Reset buffer bindings to nothing.
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
    }
}

/// Hexagon Background Shader Program
#[derive(Debug)]
pub struct HexagonShaderProgram {
    // Program id,
    program: ShaderProgram,
    // The time uniform to be used to change opacity of different regions
    // u_epoch_millis: GLint,
}

impl HexagonShaderProgram {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, ShaderError> {
        // XXX: This must be in-sync with fragment shader defines.
        let header: Option<&str> = None;
        let program = ShaderProgram::new(shader_version, header, HXBG_SHADER_V, HXBG_SHADER_F)?;

        // get uniform locations
        // let u_epoch_millis =
        //     unsafe { gl::GetUniformLocation(program, b"epoch_millis\0".as_ptr() as *const _) };

        Ok(HexagonShaderProgram { program })
    }

    // fn set_epoch_millis(&self, epoch_millis: f32) {
    // unsafe {
    // gl::Uniform1f(self.u_epoch_millis, epoch_millis);
    // }
    // }
    fn id(&self) -> GLuint {
        self.program.id()
    }
}
