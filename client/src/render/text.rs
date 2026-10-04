//! Text: glyphs of the TrueType font (`font_1_filename`, Soldat's `play-regular.ttf`) are
//! rasterized into one atlas per font style at the current screen scale, and drawn as quads in
//! interface coordinates (480 units high, like Soldat's 640x480 interface).

use super::*;
use ab_glyph::{Font as _, FontVec, PxScale, ScaleFont};

/// Font styles of Soldat's interface (`FontStyles`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FontStyle {
    /// `FONT_MENU`: font 1, `font_menusize` 12, stretched 1.5.
    Menu,
    /// `FONT_SMALL`: font 2, `font_consolesize` 9, stretched 1.25.
    Small,
    /// `FONT_WEAPONS_MENU`: font 2, `font_weaponmenusize` 8, stretched 1.25.
    WeaponsMenu,
    /// `FONT_SMALLEST`: font 2, `font_consolesmallsize` 7, stretched 1.25.
    Smallest,
    /// `FONT_BIG`: font 1, `font_bigsize` 28, stretched 1.5 (big messages).
    Big,
}

impl FontStyle {
    const ALL: [FontStyle; 5] = [
        FontStyle::Menu,
        FontStyle::Small,
        FontStyle::WeaponsMenu,
        FontStyle::Smallest,
        FontStyle::Big,
    ];

    /// Size in interface units and horizontal stretch.
    fn metrics(self) -> (f32, f32) {
        match self {
            FontStyle::Menu => (12.0, 1.5),
            FontStyle::Small => (9.0, 1.25),
            FontStyle::WeaponsMenu => (8.0, 1.25),
            FontStyle::Smallest => (7.0, 1.25),
            FontStyle::Big => (28.0, 1.5),
        }
    }
}

#[derive(Copy, Clone, Default)]
struct Glyph {
    /// Offset from the pen position (baseline) and size, in pixels.
    offset: Vec2,
    size: Vec2,
    advance: f32,
    texcoords: (Vec2, Vec2),
}

struct StyleAtlas {
    texture: Option<Texture>,
    glyphs: Vec<Glyph>,
    ascent: f32,
    line_height: f32,
}

/// Characters in the atlases: Latin-1 (Soldat's interface texts are mostly ASCII).
const FIRST_CHAR: u32 = 32;
const LAST_CHAR: u32 = 255;
const ATLAS_SIZE: usize = 512;

pub struct Fonts {
    font: Option<FontVec>,
    /// Pixels per interface unit the atlases were built for.
    scale: f32,
    atlases: Vec<StyleAtlas>,
}

impl Fonts {
    pub fn load(vfs: &Vfs, filename: &str) -> Fonts {
        let font = match vfs.read(filename) {
            Ok(data) => match FontVec::try_from_vec(data) {
                Ok(font) => Some(font),
                Err(error) => {
                    tracing::warn!(filename, %error, "cannot load font");
                    None
                }
            },
            Err(error) => {
                tracing::warn!(filename, %error, "font not found, menus have no text");
                None
            }
        };

        Fonts {
            font,
            scale: 0.0,
            atlases: Vec::new(),
        }
    }

    /// Builds the atlases for `scale` pixels per interface unit, if they aren't yet.
    pub fn prepare(&mut self, context: &mut Gfx2dContext, scale: f32) {
        if self.scale == scale || self.font.is_none() {
            return;
        }
        self.scale = scale;
        for atlas in self.atlases.drain(..) {
            if let Some(texture) = atlas.texture {
                context.delete_texture(texture);
            }
        }
        let font = self.font.as_ref().unwrap();
        self.atlases = FontStyle::ALL
            .iter()
            .map(|style| build_atlas(context, font, *style, scale))
            .collect();
    }

    /// The atlases' textures go.
    pub fn delete(&mut self, context: &mut Gfx2dContext) {
        for texture in self.atlases.drain(..).filter_map(|atlas| atlas.texture) {
            context.delete_texture(texture);
        }
        self.scale = 0.0;
    }

    fn atlas(&self, style: FontStyle) -> Option<&StyleAtlas> {
        let index = FontStyle::ALL.iter().position(|s| *s == style)?;
        self.atlases.get(index)
    }

    fn glyph(atlas: &StyleAtlas, c: char) -> Glyph {
        let c = u32::from(c);
        let c = if (FIRST_CHAR..=LAST_CHAR).contains(&c) {
            c
        } else {
            u32::from('?')
        };
        atlas.glyphs[(c - FIRST_CHAR) as usize]
    }

    /// Width and height of a (single line) text in interface units.
    pub fn measure(&self, style: FontStyle, text: &str) -> Vec2 {
        let Some(atlas) = self.atlas(style) else {
            return Vec2::ZERO;
        };
        let width: f32 = text.chars().map(|c| Self::glyph(atlas, c).advance).sum();
        vec2(width, atlas.line_height) / self.scale
    }

    /// Height of the font above the baseline in interface units.
    pub fn ascent(&self, style: FontStyle) -> f32 {
        self.atlas(style).map_or(0.0, |a| a.ascent / self.scale)
    }

    /// Draws text with its top left corner at `pos` (interface units); lines split at '\n'.
    /// A shadow is drawn one pixel down right first.
    pub fn draw(
        &self,
        batch: &mut DrawBatch,
        style: FontStyle,
        text: &str,
        pos: Vec2,
        color: Color,
        shadow: Option<Color>,
    ) {
        self.draw_scaled(batch, style, text, pos, color, shadow, 1.0);
    }

    /// [`Fonts::draw`], `zoom` times as big (from `pos`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scaled(
        &self,
        batch: &mut DrawBatch,
        style: FontStyle,
        text: &str,
        pos: Vec2,
        color: Color,
        shadow: Option<Color>,
        zoom: f32,
    ) {
        if let Some(shadow) = shadow {
            self.draw_plain(batch, style, text, pos + vec2(1.0, 1.0), shadow, zoom);
        }
        self.draw_plain(batch, style, text, pos, color, zoom);
    }

    fn draw_plain(
        &self,
        batch: &mut DrawBatch,
        style: FontStyle,
        text: &str,
        pos: Vec2,
        color: Color,
        zoom: f32,
    ) {
        let Some(atlas) = self.atlas(style) else {
            return;
        };
        let Some(texture) = atlas.texture.as_ref() else {
            return;
        };
        let s = self.scale;
        // snap the pen to pixels so glyphs stay crisp
        let origin = (pos * s).round();
        let mut pen = vec2(0.0, atlas.ascent.round());

        for c in text.chars() {
            if c == '\n' {
                pen = vec2(0.0, pen.y + atlas.line_height.round());
                continue;
            }
            let g = Self::glyph(atlas, c);
            if g.size.x > 0.0 {
                let p0 = (origin + (pen + g.offset) * zoom) / s;
                let p1 = (origin + (pen + g.offset + g.size) * zoom) / s;
                let (t0, t1) = g.texcoords;
                batch.add_quad(
                    Some(texture),
                    &[
                        vertex(p0, t0, color),
                        vertex(vec2(p1.x, p0.y), vec2(t1.x, t0.y), color),
                        vertex(p1, t1, color),
                        vertex(vec2(p0.x, p1.y), vec2(t0.x, t1.y), color),
                    ],
                );
            }
            pen.x += g.advance;
        }
    }
}

/// Shelf-packs the glyphs into a `size` square atlas (premultiplied white pixels), or
/// `None` when they don't fit.
fn pack_glyphs(font: &FontVec, px: PxScale, size: usize) -> Option<(Vec<u8>, Vec<Glyph>)> {
    let scaled = font.as_scaled(px);
    let mut pixels = vec![0u8; size * size * 4];
    let mut glyphs = Vec::new();
    // shelf packing with a pixel of padding
    let (mut x, mut y, mut shelf) = (1usize, 1usize, 0usize);

    for code in FIRST_CHAR..=LAST_CHAR {
        let c = char::from_u32(code).unwrap_or('?');
        let id = scaled.glyph_id(c);
        let advance = scaled.h_advance(id);
        let Some(outlined) = font.outline_glyph(id.with_scale(px)) else {
            glyphs.push(Glyph {
                advance,
                ..Default::default()
            });
            continue;
        };

        let bounds = outlined.px_bounds();
        let (w, h) = (bounds.width() as usize, bounds.height() as usize);
        if x + w + 1 > size {
            x = 1;
            y += shelf + 1;
            shelf = 0;
        }
        if y + h + 1 > size {
            return None;
        }

        outlined.draw(|gx, gy, coverage| {
            let i = ((y + gy as usize) * size + x + gx as usize) * 4;
            let a = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            pixels[i..i + 4].copy_from_slice(&[a, a, a, a]);
        });

        let size_px = vec2(w as f32, h as f32);
        let atlas = size as f32;
        glyphs.push(Glyph {
            offset: vec2(bounds.min.x, bounds.min.y),
            size: size_px,
            advance,
            texcoords: (
                vec2(x as f32, y as f32) / atlas,
                (vec2(x as f32, y as f32) + size_px) / atlas,
            ),
        });

        x += w + 1;
        shelf = shelf.max(h);
    }
    Some((pixels, glyphs))
}

fn build_atlas(
    context: &mut Gfx2dContext,
    font: &FontVec,
    style: FontStyle,
    scale: f32,
) -> StyleAtlas {
    let (size, stretch) = style.metrics();
    let px = PxScale {
        x: size * stretch * scale,
        y: size * scale,
    };
    let scaled = font.as_scaled(px);

    // the smallest atlas the glyphs fit in
    let (atlas_size, pixels, glyphs) = [ATLAS_SIZE, 1024, 2048, 4096]
        .into_iter()
        .find_map(|size| pack_glyphs(font, px, size).map(|(p, g)| (size, p, g)))
        .unwrap_or_else(|| {
            tracing::warn!("font too large for an atlas");
            let advance = |c| scaled.h_advance(scaled.glyph_id(c));
            let glyphs = (FIRST_CHAR..=LAST_CHAR)
                .map(|code| Glyph {
                    advance: advance(char::from_u32(code).unwrap_or('?')),
                    ..Default::default()
                })
                .collect();
            (ATLAS_SIZE, vec![0; ATLAS_SIZE * ATLAS_SIZE * 4], glyphs)
        });

    let texture = Texture::new(
        context,
        (atlas_size as u16, atlas_size as u16),
        &pixels,
        FilterMethod::Scale,
        WrapMode::Clamp,
    );

    StyleAtlas {
        texture: Some(texture),
        glyphs,
        ascent: scaled.ascent(),
        line_height: scaled.height() + scaled.line_gap(),
    }
}
