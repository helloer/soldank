use super::*;
use miniquad::{
    FilterMode, MipmapFilterMode, RenderingBackend, TextureFormat, TextureId, TextureParams,
    TextureWrap,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FilterMethod {
    Scale,
    Trilinear,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum WrapMode {
    Clamp,
    Tile,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Texture {
    id: TextureId,
    dimensions: (u16, u16),
}

impl Texture {
    /// Creates a texture from a decoded image, applying the color key and premultiplying alpha.
    pub fn from_image(
        g: &mut Gfx2dContext,
        mut img: image::RgbaImage,
        filter: FilterMethod,
        wrap: WrapMode,
        color_key: Option<Color>,
    ) -> Texture {
        if let Some(color) = color_key {
            remove_color_key(&mut img, color);
        }

        premultiply_image(&mut img);

        let dimensions = (img.width() as u16, img.height() as u16);
        Texture::new(g, dimensions, &img, filter, wrap)
    }

    pub fn new(
        g: &mut Gfx2dContext,
        dimensions: (u16, u16),
        data: &[u8],
        filter: FilterMethod,
        wrap: WrapMode,
    ) -> Texture {
        create_texture(&mut *g.ctx, dimensions, data, filter, wrap)
    }

    pub fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    pub fn id(&self) -> TextureId {
        self.id
    }

    pub fn is(&self, other: &Texture) -> bool {
        self.id == other.id
    }
}

pub(crate) fn create_texture(
    ctx: &mut dyn RenderingBackend,
    (w, h): (u16, u16),
    data: &[u8],
    filter: FilterMethod,
    wrap: WrapMode,
) -> Texture {
    let params = TextureParams {
        format: TextureFormat::RGBA8,
        wrap: match wrap {
            WrapMode::Clamp => TextureWrap::Clamp,
            WrapMode::Tile => TextureWrap::Repeat,
        },
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        mipmap_filter: match filter {
            FilterMethod::Scale => MipmapFilterMode::None,
            FilterMethod::Trilinear => MipmapFilterMode::Linear,
        },
        width: u32::from(w),
        height: u32::from(h),
        allocate_mipmaps: filter == FilterMethod::Trilinear,
        ..Default::default()
    };

    let id = ctx.new_texture_from_data_and_format(data, params);

    if filter == FilterMethod::Trilinear {
        ctx.texture_generate_mipmaps(id);
    }

    Texture {
        id,
        dimensions: (w, h),
    }
}

/// Decodes an image file (png, bmp, jpg, gif, ...) from memory.
pub fn decode_image_rgba(data: &[u8]) -> image::ImageResult<image::RgbaImage> {
    Ok(image::load_from_memory(data)?.into_rgba8())
}

pub fn premultiply_image(img: &mut image::RgbaImage) {
    for pixel in img.pixels_mut() {
        let a = f32::from(pixel[3]) / 255.0;

        *pixel = image::Rgba([
            (f32::from(pixel[0]) * a) as u8,
            (f32::from(pixel[1]) * a) as u8,
            (f32::from(pixel[2]) * a) as u8,
            pixel[3],
        ]);
    }
}

pub fn remove_color_key(img: &mut image::RgbaImage, color_key: Color) {
    for pixel in img.pixels_mut() {
        if rgba(pixel[0], pixel[1], pixel[2], pixel[3]) == color_key {
            *pixel = image::Rgba([0, 0, 0, 0]);
        }
    }
}
