//! Renders tessellated nannou primitives with their own vertex colors.

use std::mem;

use crate::gl::types::*;
use crate::renderer::shader::{ShaderProgram, ShaderVersion};
use crate::{gl, renderer};

static NANNOU_SHADER_F: &str = include_str!("../../res/nannou.f.glsl");
static NANNOU_SHADER_V: &str = include_str!("../../res/nannou.v.glsl");

#[derive(Debug)]
pub struct NannouRenderer {
    // GL buffer objects.
    vao: GLuint,
    vbo: GLuint,

    program: ShaderProgram,
}

impl NannouRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let program = ShaderProgram::new(shader_version, None, NANNOU_SHADER_V, NANNOU_SHADER_F)?;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            gl::BindVertexArray(vao);

            // VBO binding is not part of VAO itself, but VBO binding is stored in attributes.
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            // [3(x,y,z) + 4(r,g,b,a) ] -> 7
            let stride = (mem::size_of::<f32>() * 7) as i32;

            // Position.
            gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, std::ptr::null());
            gl::EnableVertexAttribArray(0);

            // Color.
            gl::VertexAttribPointer(
                1,
                4,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (mem::size_of::<f32>() * 3) as *const _,
            );
            gl::EnableVertexAttribArray(1);

            // Reset buffer bindings.
            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }
        Ok(Self { vao, vbo, program })
    }

    /// Draws `opengl_data`, made of xyzrgba vertices, as a triangle list.
    pub fn draw(&mut self, opengl_data: &[f32]) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::UseProgram(self.program.id());

            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(opengl_data) as _,
                opengl_data.as_ptr() as *const _,
                gl::STREAM_DRAW,
            );
            gl::DrawArrays(gl::TRIANGLES, 0, (opengl_data.len() / 7) as i32);

            gl::UseProgram(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
    }
}

impl Drop for NannouRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}
