mod batch;
mod color;
mod context;
mod spritesheet;
mod texture;
mod transform;

pub mod binpack;
pub mod math;

pub use batch::DrawBatch;
pub use batch::DrawSlice;
pub use color::Color;
pub use color::rgb;
pub use color::rgba;
pub use context::Gfx2dContext;
pub use context::Vertex;
pub use context::vertex;
pub use image;
pub use miniquad as mq;
pub use spritesheet::Sprite;
pub use spritesheet::SpriteInfo;
pub use spritesheet::Spritesheet;
pub use texture::FilterMethod;
pub use texture::Texture;
pub use texture::WrapMode;
pub use transform::Transform;

pub mod gfx2d_extra {
    pub use super::texture::decode_image_rgba;
    pub use super::texture::premultiply_image;
    pub use super::texture::remove_color_key;
}

use math::*;
