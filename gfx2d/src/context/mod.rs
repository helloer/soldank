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
    in_pass: bool,
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

        Gfx2dContext {
            ctx,
            pipeline,
            ibuf,
            ibuf_len: 0,
            white,
            in_pass: false,
        }
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

        self.in_pass = true;
    }

    pub fn draw(&mut self, slice: &mut DrawSlice, transform: &Mat2d) {
        if !self.in_pass {
            self.ctx.begin_default_pass(PassAction::Nothing);
            self.in_pass = true;
        }

        slice.batch.update(&mut *self.ctx);
        self.ensure_indices(slice.batch.len());

        let mut bindings = Bindings {
            vertex_buffers: vec![slice.buffer()],
            index_buffer: self.ibuf,
            images: vec![self.white.id()],
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
