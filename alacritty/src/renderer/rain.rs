//! Renders rain falling in a region of the window.

use std::mem;

use crate::gl::types::*;
use crate::renderer::shader::{ShaderProgram, ShaderVersion};
use crate::{gl, renderer};

static RAIN_SHADER_F: &str = include_str!("../../res/rain.f.glsl");
static RAIN_SHADER_V: &str = include_str!("../../res/rain.v.glsl");

/// Floats per vertex: [2(x,y) + 4(r,g,b,a) + 2(u,v)].
pub const RAIN_VERTEX_FLOATS: usize = 8;

#[derive(Debug)]
pub struct RainRenderer {
    // GL buffer objects.
    vao: GLuint,
    vbo: GLuint,

    program: ShaderProgram,
    u_time: Option<GLint>,
    u_wind: Option<GLint>,
}

impl RainRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let program = ShaderProgram::new(shader_version, None, RAIN_SHADER_V, RAIN_SHADER_F)?;
        let u_time = program.get_uniform_location(c"iTime").ok();
        let u_wind = program.get_uniform_location(c"iWind").ok();
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            gl::BindVertexArray(vao);

            // VBO binding is not part of VAO itself, but VBO binding is stored in attributes.
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            // The attribute locations are not fixed by the GLES2 shaders, so look them up.
            let stride = (mem::size_of::<f32>() * RAIN_VERTEX_FLOATS) as i32;
            let mut offset = 0;
            for (name, size) in [(c"aPos", 2), (c"aColor", 4), (c"aUv", 2)] {
                let location = gl::GetAttribLocation(program.id(), name.as_ptr());
                if location >= 0 {
                    let location = location as GLuint;
                    gl::VertexAttribPointer(
                        location,
                        size,
                        gl::FLOAT,
                        gl::FALSE,
                        stride,
                        (offset * mem::size_of::<f32>()) as *const _,
                    );
                    gl::EnableVertexAttribArray(location);
                }
                offset += size as usize;
            }

            // Reset buffer bindings.
            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }
        Ok(Self { vao, vbo, program, u_time, u_wind })
    }

    /// Draws `vertices`, a triangle list of `RAIN_VERTEX_FLOATS` floats per vertex whose alpha is
    /// the intensity of the rain, animated at `time` seconds and slanted by `wind`, from 0 to 1.
    pub fn draw(&mut self, vertices: &[f32], time: f32, wind: f32) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::UseProgram(self.program.id());
            if let Some(u_time) = self.u_time {
                gl::Uniform1f(u_time, time);
            }
            if let Some(u_wind) = self.u_wind {
                gl::Uniform1f(u_wind, wind);
            }

            gl::BufferData(
                gl::ARRAY_BUFFER,
                mem::size_of_val(vertices) as _,
                vertices.as_ptr() as *const _,
                gl::STREAM_DRAW,
            );
            gl::DrawArrays(gl::TRIANGLES, 0, (vertices.len() / RAIN_VERTEX_FLOATS) as i32);

            gl::UseProgram(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
        }
    }
}

impl Drop for RainRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}
