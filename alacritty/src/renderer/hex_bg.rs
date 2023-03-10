use std::mem;

use crate::display::SizeInfo;
use crate::gl;
use crate::gl::types::*;
use crate::renderer::shader::{ShaderError, ShaderProgram, ShaderVersion};
use crate::renderer::{self, cstr};

static HXBG_SHADER_F: &str = include_str!("../../res/hex_bg.f.glsl");
static HXBG_SHADER_V: &str = include_str!("../../res/hex_bg.v.glsl");

#[derive(Debug)]
pub struct HexBgRenderer {
    // GL buffer objects.
    pub vao: GLuint,
    pub vbo: GLuint,
    // The Frame Buffer
    pub fbo: GLuint,

    program: HexagonShaderProgram,
}

impl HexBgRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, renderer::Error> {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let mut fbo: GLuint = 0;
        let program = HexagonShaderProgram::new(shader_version)?;
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC1_COLOR, gl::ONE_MINUS_SRC1_COLOR);
            gl::Enable(gl::MULTISAMPLE);

            // Allocate buffers.
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::GenFramebuffers(1, &mut fbo);

            gl::BindVertexArray(vao);

            // VBO binding is not part of VAO itself, but VBO binding is stored in attributes.
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);

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

            // Texture.
            // SEB XXX: Unharcode the 1024 x 768
            let mut rendered_texture: GLuint = 0;
            gl::GenTextures(1, &mut rendered_texture);
            // "Bind" the newly created texture : all future texture functions will modify this texture
            gl::BindTexture(gl::TEXTURE_2D, rendered_texture);
            // Give an empty image to OpenGL ( the last "0" )
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGB as i32,
                1024,
                768,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
            // Poor filtering. Needed !
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);

            // The depth buffer
            let mut depth_render_buffer: GLuint = 0;
            gl::GenRenderbuffers(1, &mut depth_render_buffer);
            gl::BindRenderbuffer(gl::RENDERBUFFER, depth_render_buffer);
            gl::RenderbufferStorage(gl::RENDERBUFFER, gl::DEPTH_COMPONENT, 1024, 768);
            gl::FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_ATTACHMENT,
                gl::RENDERBUFFER,
                depth_render_buffer,
            );
            // Set "renderedTexture" as our colour attachement #0
            gl::FramebufferTexture(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, rendered_texture, 0);
            // Set the list of draw buffers.
            let draw_buffers = vec![gl::COLOR_ATTACHMENT0];
            gl::DrawBuffers(1, draw_buffers.as_ptr() as *const _); // "1" is the size of DrawBuffers
            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                log::error!("CheckFramebufferStatus is not COMPLETE state");
            }

            // Reset buffer bindings.
            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        Ok(Self { vao, vbo, fbo, program })
    }

    pub fn draw(
        &mut self,
        opengl_data: &[f32],
        gl_mode: u32,
        size_info: &SizeInfo,
        time_secs_with_ms: f32,
    ) {
        let max_dimension = size_info.width().max(size_info.height());
        unsafe {
            // Bind VAO to enable vertex attribute slots.
            gl::BindVertexArray(self.vao);

            // Bind VBO only once for buffer data upload only.
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

            // Swap program
            gl::UseProgram(self.program.id());
            self.program.update_uniforms(
                max_dimension * 16. - (time_secs_with_ms * 200. % (max_dimension * 32.)),
                size_info,
                time_secs_with_ms / 1000.,
            );

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
    u_active_x_shine_offset: Option<GLint>,
    // The resolution uniforms
    u_resolution: Option<GLint>,
    // The resolution uniforms
    u_time: Option<GLint>,
}

impl HexagonShaderProgram {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, ShaderError> {
        // XXX: This must be in-sync with fragment shader defines.
        let header: Option<&str> = None;
        let program = ShaderProgram::new(shader_version, header, HXBG_SHADER_V, HXBG_SHADER_F)?;

        Ok(HexagonShaderProgram {
            u_active_x_shine_offset: program.get_uniform_location(cstr!("activeXShineOffset")).ok(),
            u_resolution: program.get_uniform_location(cstr!("iResolution")).ok(),
            u_time: program.get_uniform_location(cstr!("iTime")).ok(),
            program,
        })
    }

    pub fn update_uniforms(&self, time_secs_with_ms: f32, size_info: &SizeInfo, time_in_secs: f32) {
        unsafe {
            if let Some(u_active_x_shine_offset) = self.u_active_x_shine_offset {
                gl::Uniform1f(u_active_x_shine_offset, time_secs_with_ms);
            }
            if let Some(u_resolution) = self.u_resolution {
                gl::Uniform3f(u_resolution, size_info.width(), size_info.height(), 0.);
            }
            if let Some(u_time) = self.u_time {
                gl::Uniform1f(u_time, time_in_secs);
            }
        }
    }

    fn id(&self) -> GLuint {
        self.program.id()
    }
}
