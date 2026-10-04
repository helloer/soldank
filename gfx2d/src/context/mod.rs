use super::*;
use miniquad::*;

mod pipeline;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub texcoords: [f32; 2],
    pub color: [u8; 4],
}

pub fn vertex(pos: Vec2, texcoords: Vec2, color: Color) -> Vertex {
    Vertex {
        pos: [pos.x, pos.y],
        texcoords: [texcoords.x, texcoords.y],
        color: color.into(),
    }
}

// Gfx2dContext

pub struct Gfx2dContext {
    pub ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    // sequential indices (0, 1, 2, ...) shared by all batches, grown on demand
    ibuf: BufferId,
    ibuf_len: usize,
    white: Texture,
    // a vertex to bind `white` with, outside of draws
    vertex: BufferId,
    in_pass: bool,
    // drawing into a render target, whose rows OpenGL stores bottom-up
    in_target: bool,
    // the window's drawing area keeps a width/height ratio within this
    aspect: (f32, f32),
}

/// The part of the window drawn in, in pixels from the top left.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The largest centered part of a `screen` whose width/height ratio is within `aspect`;
/// the rest stays black.
pub fn letterbox(screen: (f32, f32), aspect: (f32, f32)) -> Viewport {
    let (sw, sh) = screen;
    let ratio = (sw / sh).clamp(aspect.0, aspect.1);
    let (width, height) = if sw / sh > ratio {
        ((sh * ratio).round(), sh)
    } else {
        (sw, (sw / ratio).round())
    };
    Viewport {
        x: ((sw - width) / 2.0).floor(),
        y: ((sh - height) / 2.0).floor(),
        width,
        height,
    }
}

/// A texture to draw into (`GfxCreateRenderTarget`). Its texture lives as long as it.
pub struct RenderTarget {
    pass: RenderPass,
    texture: Texture,
}

impl RenderTarget {
    pub fn texture(&self) -> Texture {
        self.texture
    }
}

impl Gfx2dContext {
    pub fn new() -> Gfx2dContext {
        let mut ctx = window::new_rendering_backend();

        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: pipeline::VERT_SOURCE,
                    fragment: pipeline::FRAG_SOURCE,
                },
                pipeline::meta(),
            )
            .unwrap();

        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &pipeline::attributes(),
            shader,
            pipeline::params(),
        );

        let ibuf = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&[0u32; 0]),
        );

        let white = texture::create_texture(
            &mut *ctx,
            (16, 16),
            &[255u8; 4 * 16 * 16],
            FilterMethod::Scale,
            WrapMode::Clamp,
        );

        let vertex = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&[vertex(Vec2::ZERO, Vec2::ZERO, rgb(255, 255, 255))]),
        );

        Gfx2dContext {
            ctx,
            pipeline,
            ibuf,
            ibuf_len: 0,
            white,
            vertex,
            in_pass: false,
            in_target: false,
            aspect: (0.0, f32::INFINITY),
        }
    }

    /// Draws keep to the part of the window whose width/height ratio is within `min` and
    /// `max` (black bars around it).
    pub fn set_aspect_range(&mut self, min: f32, max: f32) {
        self.aspect = (min, max);
    }

    /// Where the screen is drawn in the window.
    pub fn viewport(&self) -> Viewport {
        letterbox(window::screen_size(), self.aspect)
    }

    fn apply_viewport(&mut self) {
        let (_, sh) = window::screen_size();
        let vp = self.viewport();
        // OpenGL's y goes up from the bottom
        let y = sh - vp.y - vp.height;
        self.ctx
            .apply_viewport(vp.x as i32, y as i32, vp.width as i32, vp.height as i32);
    }

    /// A transparent texture of `size` pixels to draw into.
    pub fn new_render_target(&mut self, (w, h): (u16, u16)) -> RenderTarget {
        let id = self.ctx.new_render_texture(TextureParams {
            format: TextureFormat::RGBA8,
            wrap: TextureWrap::Clamp,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            width: u32::from(w),
            height: u32::from(h),
            ..Default::default()
        });
        let pass = self.ctx.new_render_pass(id, None);
        RenderTarget {
            pass,
            texture: Texture::from_id(id, (w, h)),
        }
    }

    pub fn delete_texture(&mut self, texture: Texture) {
        self.unbind_textures();
        self.ctx.delete_texture(texture.id());
    }

    pub fn delete_render_target(&mut self, target: RenderTarget) {
        self.unbind_textures();
        self.ctx.delete_render_pass(target.pass);
    }

    /// Binds `white`, which lives as long as the context, before a texture is deleted.
    /// miniquad would keep the deleted texture's GL name as bound; GL gives that name to the
    /// next new texture, and miniquad then skips binding it to upload its pixels (they'd go
    /// nowhere: the map drawn black after two map loads in a row).
    fn unbind_textures(&mut self) {
        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&Bindings {
            vertex_buffers: vec![self.vertex],
            index_buffer: self.ibuf,
            images: vec![self.white.id()],
        });
    }

    /// Draws go to `target`, cleared to transparent, until [`Self::end_target`]. Transforms
    /// work as on screen: the texture's first row is the top.
    pub fn begin_target(&mut self, target: &RenderTarget) {
        if self.in_pass {
            self.ctx.end_render_pass();
        }
        self.ctx.begin_pass(
            Some(target.pass),
            PassAction::clear_color(0.0, 0.0, 0.0, 0.0),
        );
        self.in_pass = true;
        self.in_target = true;
    }

    /// Back to drawing on the screen (without clearing it).
    pub fn end_target(&mut self) {
        if self.in_pass {
            self.ctx.end_render_pass();
        }
        self.in_pass = false;
        self.in_target = false;
    }

    fn ensure_indices(&mut self, len: usize) {
        if len > self.ibuf_len {
            let n = len.next_power_of_two();
            let indices: Vec<u32> = (0..n as u32).collect();
            self.ctx.delete_buffer(self.ibuf);
            self.ibuf = self.ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Immutable,
                BufferSource::slice(&indices),
            );
            self.ibuf_len = n;
        }
    }

    pub fn clear(&mut self, color: Color) {
        if self.in_pass {
            self.ctx.end_render_pass();
        }

        self.ctx.begin_default_pass(PassAction::clear_color(
            f32::from(color.r()) / 255.0,
            f32::from(color.g()) / 255.0,
            f32::from(color.b()) / 255.0,
            f32::from(color.a()) / 255.0,
        ));
        self.apply_viewport();

        self.in_pass = true;
    }

    pub fn draw(&mut self, slice: &mut DrawSlice, transform: &Mat2d) {
        if !self.in_pass {
            self.ctx.begin_default_pass(PassAction::Nothing);
            self.apply_viewport();
            self.in_pass = true;
        }

        slice.batch.update(&mut *self.ctx);
        self.ensure_indices(slice.batch.len());

        let mut bindings = Bindings {
            vertex_buffers: vec![slice.buffer()],
            index_buffer: self.ibuf,
            images: vec![self.white.id()],
        };

        let transform = if self.in_target {
            Mat2d::scale(1.0, -1.0) * *transform
        } else {
            *transform
        };
        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx
            .apply_uniforms(UniformsSource::table(&pipeline::Uniforms {
                transform: transform.to_mat4(),
            }));

        for cmd in slice.commands() {
            bindings.images[0] = cmd.texture.unwrap_or(self.white).id();
            self.ctx.apply_bindings(&bindings);

            let start = cmd.vertex_range.start as i32;
            let count = cmd.vertex_range.len() as i32;
            self.ctx.draw(start, count, 1);
        }
    }

    /// The screen as drawn so far this frame (call before [`Self::present`]), opaque.
    pub fn read_screen(&mut self) -> image::RgbaImage {
        let (_, sh) = window::screen_size();
        let vp = self.viewport();
        let (w, h) = (vp.width as u32, vp.height as u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        unsafe {
            gl::glReadPixels(
                vp.x as i32,
                (sh - vp.y - vp.height) as i32,
                w as i32,
                h as i32,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                pixels.as_mut_ptr() as *mut _,
            );
        }
        // OpenGL's rows go bottom-up
        let mut rows: Vec<u8> = pixels
            .chunks_exact((w * 4) as usize)
            .rev()
            .flatten()
            .copied()
            .collect();
        for alpha in rows.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
        image::RgbaImage::from_raw(w, h, rows).expect("w * h pixels")
    }

    /// Ends the current pass, for other code drawing with the backend (`ctx`) itself.
    pub fn end_pass(&mut self) {
        if self.in_pass {
            self.ctx.end_render_pass();
            self.in_pass = false;
        }
    }

    pub fn present(&mut self) {
        if self.in_pass {
            self.ctx.end_render_pass();
            self.in_pass = false;
        }

        self.ctx.commit_frame();
    }

    pub fn max_texture_size(&self) -> usize {
        // guaranteed minimum on any hardware we care about
        4096
    }
}

impl Default for Gfx2dContext {
    fn default() -> Self {
        Self::new()
    }
}
