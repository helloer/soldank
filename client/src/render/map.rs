use super::*;
use std::ops::Range;

pub struct MapGraphics {
    pub batch: DrawBatch,
    /// Scenery props that are animated GIFs (`UpdateProps`).
    animations: Vec<Animation>,
    pub background: Range<usize>,
    pub polys_back: Range<usize>,
    pub polys_front: Range<usize>,
    pub scenery_back: Range<usize>,
    pub scenery_mid: Range<usize>,
    pub scenery_front: Range<usize>,
    /// Its textures, deleted with it.
    textures: Vec<Texture>,
}

/// An animated scenery prop.
struct Animation {
    /// Its quad's 6 vertices in the batch, from here.
    vertices: usize,
    frames: Vec<Frame>,
    shown: usize,
}

/// A frame of an animated prop: its texture coordinates (x, then y) and how long it shows
/// (hundredths of a second, `Sprite.Delay`).
#[derive(Clone, Copy)]
struct Frame {
    x: (f32, f32),
    y: (f32, f32),
    delay: u32,
}

/// The frames of an animated GIF scenery file (with the resolved name); `None` for others.
fn gif_frames(vfs: &Vfs, file: &str) -> Option<(String, Vec<(image::RgbaImage, u32)>)> {
    let name = vfs.find_with_extensions(&format!("scenery-gfx/{file}"), IMAGE_EXTENSIONS)?;
    if !name.ends_with(".gif") {
        return None;
    }
    let frames = gfx2d_extra::decode_gif_frames(&vfs.read(&name).ok()?).ok()?;
    (frames.len() > 1).then_some((name, frames))
}

fn is_prop_active(map: &MapFile, prop: &MapProp) -> bool {
    prop.active && prop.level <= 2 && prop.style > 0 && prop.style as usize <= map.scenery.len()
}

fn is_background_poly(poly: &MapPolygon) -> bool {
    matches!(
        poly.polytype,
        PolyType::Background | PolyType::BackgroundTransition
    )
}

fn add_poly(batch: &mut DrawBatch, poly: &MapPolygon, texture: &Texture) {
    let (a, b, c) = (&poly.vertices[0], &poly.vertices[1], &poly.vertices[2]);

    batch.add(
        Some(texture),
        &[
            vertex(
                vec2(a.x, a.y),
                vec2(a.u, a.v),
                rgba(a.color.r, a.color.g, a.color.b, a.color.a),
            ),
            vertex(
                vec2(b.x, b.y),
                vec2(b.u, b.v),
                rgba(b.color.r, b.color.g, b.color.b, b.color.a),
            ),
            vertex(
                vec2(c.x, c.y),
                vec2(c.u, c.v),
                rgba(c.color.r, c.color.g, c.color.b, c.color.a),
            ),
        ],
    );
}

/// A scenery prop, remembering where an animated one's quad is.
fn add_animated(
    batch: &mut DrawBatch,
    animations: &mut Vec<Animation>,
    animated: &[Option<Vec<(Sprite, u32)>>],
    prop: (&MapProp, &Sprite),
) {
    let vertices = batch.len();
    add_scenery(batch, prop);
    if let Some(frames) = &animated[prop.0.style as usize - 1] {
        animations.push(Animation {
            vertices,
            frames: frames
                .iter()
                .map(|(sprite, delay)| Frame {
                    x: sprite.texcoords_x,
                    y: sprite.texcoords_y,
                    delay: *delay,
                })
                .collect(),
            shown: 0,
        });
    }
}

fn add_scenery(batch: &mut DrawBatch, (prop, sprite): (&MapProp, &Sprite)) {
    let color = rgba(prop.color.r, prop.color.g, prop.color.b, prop.alpha);
    let mut sprite = sprite.clone();
    sprite.width = prop.width as f32;
    sprite.height = prop.height as f32;

    batch.add_sprite(
        &sprite,
        color,
        Transform::FromOrigin {
            pos: vec2(prop.x, prop.y),
            scale: vec2(prop.scale_x, prop.scale_y),
            rot: (-prop.rotation, vec2(0.0, 1.0)),
        },
    );
}

impl MapGraphics {
    pub fn background(&mut self) -> DrawSlice<'_> {
        self.batch.slice(self.background.clone())
    }
    pub fn polys_back(&mut self) -> DrawSlice<'_> {
        self.batch.slice(self.polys_back.clone())
    }
    pub fn polys_front(&mut self) -> DrawSlice<'_> {
        self.batch.slice(self.polys_front.clone())
    }
    pub fn scenery_back(&mut self) -> DrawSlice<'_> {
        self.batch.slice(self.scenery_back.clone())
    }
    pub fn scenery_mid(&mut self) -> DrawSlice<'_> {
        self.batch.slice(self.scenery_mid.clone())
    }
    pub fn scenery_front(&mut self) -> DrawSlice<'_> {
        self.batch.slice(self.scenery_front.clone())
    }

    /// `UpdateProps`: each animated prop shows the frame for `t` (seconds), looping.
    pub fn animate(&mut self, t: f64) {
        for animation in &mut self.animations {
            let total: u32 = animation.frames.iter().map(|f| f.delay).sum();
            if total == 0 {
                continue;
            }
            let duration = f64::from(total) / 100.0;
            let time = (100.0 * (t - duration * (t / duration).trunc())) as u32;
            let (mut accum, mut frame) = (0, 0);
            while frame + 1 < animation.frames.len() && accum + animation.frames[frame].delay < time
            {
                accum += animation.frames[frame].delay;
                frame += 1;
            }
            if frame == animation.shown {
                continue;
            }
            animation.shown = frame;
            // the quad's two triangles: corners 0 1 2, then 2 0 3
            let Frame {
                x: (x0, x1),
                y: (y0, y1),
                ..
            } = animation.frames[frame];
            let corners = [[x0, y0], [x1, y0], [x1, y1], [x1, y1], [x0, y0], [x0, y1]];
            let start = animation.vertices;
            for (vertex, uv) in self
                .batch
                .vertices_mut(start..start + 6)
                .iter_mut()
                .zip(corners)
            {
                vertex.texcoords = uv;
            }
        }
    }

    pub fn empty() -> MapGraphics {
        MapGraphics {
            batch: DrawBatch::new_static(),
            animations: Vec::new(),
            background: 0..0,
            polys_back: 0..0,
            polys_front: 0..0,
            scenery_back: 0..0,
            scenery_mid: 0..0,
            scenery_front: 0..0,
            textures: Vec::new(),
        }
    }

    /// Its textures go.
    pub fn delete(&mut self, context: &mut Gfx2dContext) {
        for texture in self.textures.drain(..) {
            context.delete_texture(texture);
        }
    }

    /// The map's graphics; `background` replaces its sky colours (`r_forcebg`).
    pub fn new(
        context: &mut Gfx2dContext,
        vfs: &Vfs,
        map: &MapFile,
        background: Option<[(u8, u8, u8); 2]>,
    ) -> MapGraphics {
        let texture = if let (_, Some(image)) = load_image(vfs, "textures", &map.texture_name) {
            Texture::from_image(
                context,
                image,
                FilterMethod::Trilinear,
                WrapMode::Tile,
                None,
            )
        } else {
            Texture::new(
                context,
                (1, 1),
                &[255u8; 4],
                FilterMethod::Scale,
                WrapMode::Clamp,
            )
        };

        let (scenery_used, sprite_index) = {
            let mut scenery_used = vec![false; map.scenery.len()];
            let mut sprite_index = vec![0usize; map.scenery.len()];

            for prop in &map.props {
                if is_prop_active(map, prop) {
                    scenery_used[prop.style as usize - 1] |= true;
                }
            }

            let mut n = 0;
            for (i, _) in map.scenery.iter().enumerate() {
                if scenery_used[i] {
                    sprite_index[i] = n;
                    n += 1;
                }
            }

            (scenery_used, sprite_index)
        };

        let mut textures = vec![texture];
        // animated GIFs get a sheet of their own: their frames on one texture
        let mut animated: Vec<Option<Vec<(Sprite, u32)>>> = vec![None; map.scenery.len()];
        let mut scenery_info: Vec<SpriteInfo> = Vec::new();
        for (i, s) in map.scenery.iter().enumerate() {
            if !scenery_used[i] {
                continue;
            }
            let green = Some(rgb(0, 255, 0));
            if let Some((name, frames)) = gif_frames(vfs, &s.filename) {
                let infos = frames
                    .iter()
                    .map(|(image, _)| {
                        SpriteInfo::new(name.clone(), Some(image.clone()), vec2(1.0, 1.0), green)
                    })
                    .collect();
                let sheet = Spritesheet::new(context, 8, FilterMethod::Trilinear, infos);
                textures.extend(sheet.textures.iter().copied());
                let texture = sheet.sprites[0].texture;
                if sheet.sprites.iter().all(|sprite| sprite.texture == texture) {
                    // `Sprite.Delay`: hundredths of a second
                    let delays = frames.iter().map(|(_, ms)| ms / 10);
                    animated[i] = Some(sheet.sprites.into_iter().zip(delays).collect());
                }
            }
            let (name, image) = load_image(vfs, "scenery-gfx", &s.filename);
            let color_key = if name.ends_with(".bmp") || name.ends_with(".gif") {
                green
            } else {
                None
            };
            scenery_info.push(SpriteInfo::new(name, image, vec2(1.0, 1.0), color_key));
        }
        let sheet = Spritesheet::new(context, 8, FilterMethod::Trilinear, scenery_info);
        textures.extend(sheet.textures.iter().copied());
        let sprites = sheet.sprites;

        let props = {
            let mut sorted: [Vec<(&MapProp, &Sprite)>; 3] = [
                Vec::with_capacity(map.props.len()),
                Vec::with_capacity(map.props.len()),
                Vec::with_capacity(map.props.len()),
            ];

            for prop in &map.props {
                if is_prop_active(map, prop) {
                    let style = prop.style as usize - 1;
                    let sprite = match &animated[style] {
                        Some(frames) => &frames[0].0,
                        None => &sprites[sprite_index[style]],
                    };
                    sorted[prop.level as usize].push((prop, sprite));
                }
            }

            sorted
        };

        let mut batch = DrawBatch::new_static();
        let mut animations = Vec::new();

        let background = {
            let d = 25.0 * f32::max(map.sectors_division as f32, f32::ceil(0.5 * 480.0 / 25.0));
            let (top, btm) = (map.bg_color_top, map.bg_color_bottom);
            let (top, btm) = match background {
                Some([t, b]) => (rgb(t.0, t.1, t.2), rgb(b.0, b.1, b.2)),
                None => (rgb(top.r, top.g, top.b), rgb(btm.r, btm.g, btm.b)),
            };

            batch.add_quad(
                None,
                &[
                    vertex(vec2(0.0, -d), vec2(0.0, 0.0), top),
                    vertex(vec2(1.0, -d), vec2(0.0, 0.0), top),
                    vertex(vec2(1.0, d), vec2(0.0, 0.0), btm),
                    vertex(vec2(0.0, d), vec2(0.0, 0.0), btm),
                ],
            );

            batch.split()
        };

        map.polygons
            .iter()
            .filter(|&p| is_background_poly(p))
            .for_each(|poly| add_poly(&mut batch, poly, &texture));
        let polys_back = batch.split();

        for &prop in &props[0] {
            add_animated(&mut batch, &mut animations, &animated, prop);
        }
        let scenery_back = batch.split();

        for &prop in &props[1] {
            add_animated(&mut batch, &mut animations, &animated, prop);
        }
        let scenery_mid = batch.split();

        map.polygons
            .iter()
            .filter(|&p| !is_background_poly(p))
            .for_each(|poly| add_poly(&mut batch, poly, &texture));
        let polys_front = batch.split();

        for &prop in &props[2] {
            add_animated(&mut batch, &mut animations, &animated, prop);
        }
        let scenery_front = batch.split();

        MapGraphics {
            batch,
            animations,
            background,
            polys_back,
            polys_front,
            scenery_back,
            scenery_mid,
            scenery_front,
            textures,
        }
    }
}
