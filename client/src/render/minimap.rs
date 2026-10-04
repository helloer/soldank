//! The minimap (F3): the map's polygons drawn small once per map (`MapGfx.Minimap` in
//! `LoadMapGraphics`), with the flags and the player's team on it (`RenderInterface`).

use super::hud::{Hud, HudState};
use super::*;
use gfx::Interface;

/// `ui_minimap_*` positions are in 640x480 units.
pub struct Minimap {
    target: RenderTarget,
    /// The texture's size in pixels.
    size: Vec2,
    /// Interface units per texture pixel (`Minimap.Scale`).
    pixel: f32,
    /// `MinimapOffset`, `MinimapScale`: world to minimap units.
    offset: Vec2,
    scale: f32,
}

/// `n`: the map is drawn this much larger, then filtered down.
const SUPERSAMPLING: u16 = 4;

impl Minimap {
    /// Draws the minimap of `map`, `pixel` interface units per screen pixel, so that its
    /// width and height add up to 260 interface units.
    pub fn new(
        context: &mut Gfx2dContext,
        graphics: &mut MapGraphics,
        batch: &mut DrawBatch,
        map: &MapFile,
        pixel: f32,
    ) -> Option<Minimap> {
        let mut vertices = map.polygons.iter().flat_map(|p| &p.vertices);
        let first = vertices.next()?;
        let (mut left, mut right, mut top, mut bottom) = (first.x, first.x, first.y, first.y);
        for v in vertices {
            left = left.min(v.x);
            right = right.max(v.x);
            top = top.min(v.y);
            bottom = bottom.max(v.y);
        }

        // r_scaleinterface: 260 game units, in render pixels
        let w = 260.0 / pixel;
        let sx = w / ((right - left) + (bottom - top));
        let width = (sx * (right - left)).round_ties_even() as u16;
        let height = (sx * (bottom - top)).round_ties_even() as u16;
        if width == 0 || height == 0 {
            return None;
        }

        let max = context.max_texture_size() as u16;
        let n = SUPERSAMPLING.min(max / width.max(height)).max(1);
        let big = context.new_render_target((n * width, n * height));
        context.begin_target(&big);

        // the background's colours beyond its gradient, then the gradient and the polygons
        let d = map_background_height(map);
        let (top_color, bottom_color) = (map.bg_color_top, map.bg_color_bottom);
        let top_color = rgb(top_color.r, top_color.g, top_color.b);
        let bottom_color = rgb(bottom_color.r, bottom_color.g, bottom_color.b);
        let band = |y0: f32, y1: f32, color: Color| {
            [
                vertex(vec2(0.0, y0), Vec2::ZERO, color),
                vertex(vec2(1.0, y0), Vec2::ZERO, color),
                vertex(vec2(1.0, y1), Vec2::ZERO, color),
                vertex(vec2(0.0, y1), Vec2::ZERO, color),
            ]
        };
        batch.clear();
        batch.add_quad(None, &band((-d).min(top), top, top_color));
        batch.add_quad(None, &band(bottom, d.max(bottom), bottom_color));
        let strip = Transform::ortho(0.0, 1.0, top, bottom).matrix();
        context.draw(&mut batch.all(), &strip);
        context.draw(&mut graphics.background(), &strip);
        let world = Transform::ortho(left, right, top, bottom).matrix();
        context.draw(&mut graphics.polys_back(), &world);
        context.draw(&mut graphics.polys_front(), &world);
        context.end_target();

        // filtered down to the real size
        let target = context.new_render_target((width, height));
        context.begin_target(&target);
        let white = rgb(255, 255, 255);
        batch.clear();
        batch.add_quad(
            Some(&big.texture()),
            &[
                vertex(vec2(0.0, 0.0), vec2(0.0, 0.0), white),
                vertex(vec2(1.0, 0.0), vec2(1.0, 0.0), white),
                vertex(vec2(1.0, 1.0), vec2(1.0, 1.0), white),
                vertex(vec2(0.0, 1.0), vec2(0.0, 1.0), white),
            ],
        );
        context.draw(
            &mut batch.all(),
            &Transform::ortho(0.0, 1.0, 0.0, 1.0).matrix(),
        );
        context.end_target();
        context.delete_render_target(big);
        batch.clear();

        Some(Minimap {
            target,
            size: vec2(f32::from(width), f32::from(height)),
            pixel,
            // Soldat offsets by the right and bottom bounds, which works because maps are
            // centered on the origin
            offset: vec2(-right, -bottom),
            scale: sx * pixel,
        })
    }

    pub fn delete(self, context: &mut Gfx2dContext) {
        context.delete_render_target(self.target);
    }

    /// `WorldToMinimap`
    fn to_minimap(&self, pos: Vec2) -> Vec2 {
        (pos - self.offset) * self.scale
    }
}

/// Half the height of the map's background gradient (`d` in `LoadMapGraphics`).
pub fn map_background_height(map: &MapFile) -> f32 {
    25.0 * f32::max(map.sectors_division as f32, f32::ceil(0.5 * 480.0 / 25.0))
}

/// The minimap with the flags, the player and teammates (`ui_minimap_*`,
/// `sv_minimap_locations`).
pub(super) fn render_minimap(hud: &mut Hud, state: &HudState, minimap: &Minimap) {
    let cvars = state.cvars;
    let corner = vec2(
        cvars.int("ui_minimap_posx") as f32 * hud.iscale,
        cvars.int("ui_minimap_posy") as f32,
    )
    .floor();

    let alpha = (f32::from(state.alpha) * 0.85).round_ties_even() as u8;
    let color = rgba(255, 255, 255, alpha);
    let p1 = corner + minimap.size * minimap.pixel;
    hud.batch.add_quad(
        Some(&minimap.target.texture()),
        &[
            vertex(corner, vec2(0.0, 0.0), color),
            vertex(vec2(p1.x, corner.y), vec2(1.0, 0.0), color),
            vertex(p1, vec2(1.0, 1.0), color),
            vertex(vec2(corner.x, p1.y), vec2(0.0, 1.0), color),
        ],
    );

    if cvars.bool("sv_minimap_locations") {
        render_locations(hud, state, minimap, corner);
    }
    if state.spectator.is_some() {
        render_camera_square(hud, state, minimap, corner);
    }
}

/// `RenderMinimapSquare`: what a spectator sees, boxed on the minimap.
fn render_camera_square(hud: &mut Hud, state: &HudState, minimap: &Minimap, corner: Vec2) {
    let half = state.size / 2.0 * state.zoom;
    let max = corner + minimap.size * minimap.pixel;
    let start = (corner + minimap.to_minimap(state.camera - half))
        .floor()
        .max(corner);
    let end = (corner + minimap.to_minimap(state.camera + half))
        .floor()
        .min(max);
    let t = 0.5;
    let color = rgba(255, 255, 255, 127);
    let mut rect = |p0: Vec2, p1: Vec2| {
        hud.batch.add_quad(
            None,
            &[
                vertex(p0, Vec2::ZERO, color),
                vertex(vec2(p1.x, p0.y), Vec2::ZERO, color),
                vertex(p1, Vec2::ZERO, color),
                vertex(vec2(p0.x, p1.y), Vec2::ZERO, color),
            ],
        );
    };
    // DrawBox: top, right, bottom, left
    rect(start - vec2(t, t), vec2(end.x + t, start.y));
    rect(vec2(end.x, start.y), vec2(end.x + t, end.y));
    rect(vec2(start.x - t, end.y), end + vec2(t, t));
    rect(vec2(start.x - t, start.y), vec2(start.x, end.y));
}

/// The flags and the players on the minimap (`sv_minimap_locations`).
fn render_locations(hud: &mut Hud, state: &HudState, minimap: &Minimap, corner: Vec2) {
    let cvars = state.cvars;
    let world = state.world;
    let alpha = cvars.int("ui_minimap_transparency").clamp(0, 255) as u8;
    let dot_width = hud.sprite(Interface::Smalldot).width;
    let dot_height = hud.sprite(Interface::Smalldot).height;
    // ToMinimap: the dot centered on the place
    let dot = |hud: &mut Hud, pos: Vec2, scale: f32, rgb: u32| {
        let p = corner + minimap.to_minimap(pos) - vec2(dot_width, dot_height) * scale / 2.0;
        let color = rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, alpha);
        hud.draw(Interface::Smalldot, p.floor(), scale, color, (0.0, 1.0));
    };

    let thing = |kind: ThingKind| world.things.iter().find(|t| t.active && t.kind == kind);
    let mode = world.config.game_mode;
    let flags = (thing(ThingKind::AlphaFlag), thing(ThingKind::BravoFlag));
    if let (Some(alpha_flag), Some(bravo_flag)) = flags
        && matches!(mode, GameMode::CaptureTheFlag | GameMode::Infiltration)
    {
        for (flag, color) in [(alpha_flag, 0xFF0000), (bravo_flag, 0x1313FF)] {
            if flag.in_base && flag.holding.is_none() {
                dot(hud, flag.skeleton.pos(1), 1.0, color);
            }
        }
    }
    if mode == GameMode::HoldTheFlag
        && let Some(flag) = thing(ThingKind::PointmatchFlag)
        && flag.holding.is_none()
    {
        dot(hud, flag.skeleton.pos(1), 1.0, 0xFFFF00);
    }
    if mode == GameMode::Rambo
        && let Some(bow) = thing(ThingKind::RamboBow)
        && bow.holding.is_none()
    {
        dot(hud, bow.skeleton.pos(1), 1.0, 0xFFFFFF);
    }

    // the player's team; a spectator sees everyone, the followed one in white
    let spectating = state.spectator.is_some();
    let me = if spectating {
        state.spectator
    } else {
        state.player
    };
    let Some(me) = me.and_then(|id| world.soldiers.get(id).map(|s| (id, s))) else {
        return;
    };
    for (id, soldier) in world.soldiers.iter().filter(|(_, s)| s.active) {
        if soldier.is_spectator() || (!spectating && soldier.team != me.1.team) {
            continue;
        }
        let chest = soldier.skeleton.pos(7);
        let holds_flag = soldier
            .holded_thing
            .and_then(|slot| world.things.get(slot))
            .is_some_and(|t| t.kind.is_flag());
        if holds_flag {
            dot(hud, chest, 1.0, 0xFFFF00);
        } else if id == me.0 || (spectating && state.follow == Some(id)) {
            dot(hud, chest, 0.8, 0xFFFFFF);
        } else if mode != GameMode::Deathmatch && soldier.team != Team::None {
            // dead teammates get a transparent dot
            let color = match soldier.team {
                Team::Alpha => 0xFF0000,
                Team::Bravo => 0x1313FF,
                Team::Charlie => 0xFFFF00,
                _ => 0x00FF00,
            };
            if !soldier.dead_meat {
                dot(hud, chest, 0.65, color);
            }
            // chat indicator
            if state
                .messages
                .chats
                .iter()
                .any(|c| c.id == id && c.delay > 0)
            {
                dot(hud, chest - vec2(0.0, 40.0), 0.5, 0xFFFFFF);
            }
        }
    }
}

/// The whole of `map` (sky, polygons, scenery) in a new target at most `size` pixels, fitted
/// to its shape (the menus' preview); `None` for a map without polygons.
pub fn render_map_preview(
    context: &mut Gfx2dContext,
    vfs: &Vfs,
    map: &MapFile,
    size: (u16, u16),
) -> Option<RenderTarget> {
    let mut vertices = map.polygons.iter().flat_map(|p| &p.vertices);
    let first = vertices.next()?;
    let (mut left, mut right, mut top, mut bottom) = (first.x, first.x, first.y, first.y);
    for v in vertices {
        left = left.min(v.x);
        right = right.max(v.x);
        top = top.min(v.y);
        bottom = bottom.max(v.y);
    }
    // a margin, and the target's shape around the map's
    let margin = 0.04 * (right - left).max(bottom - top);
    let (left, right, top, bottom) = (left - margin, right + margin, top - margin, bottom + margin);
    let scale = (f32::from(size.0) / (right - left)).min(f32::from(size.1) / (bottom - top));
    let width = ((right - left) * scale).round().max(1.0) as u16;
    let height = ((bottom - top) * scale).round().max(1.0) as u16;

    let mut graphics = MapGraphics::new(context, vfs, map, None);
    let mut batch = DrawBatch::new();
    let target = context.new_render_target((width, height));
    context.begin_target(&target);
    let d = map_background_height(map);
    let (top_color, bottom_color) = (map.bg_color_top, map.bg_color_bottom);
    let top_color = rgb(top_color.r, top_color.g, top_color.b);
    let bottom_color = rgb(bottom_color.r, bottom_color.g, bottom_color.b);
    let band = |y0: f32, y1: f32, color: Color| {
        [
            vertex(vec2(0.0, y0), Vec2::ZERO, color),
            vertex(vec2(1.0, y0), Vec2::ZERO, color),
            vertex(vec2(1.0, y1), Vec2::ZERO, color),
            vertex(vec2(0.0, y1), Vec2::ZERO, color),
        ]
    };
    batch.add_quad(None, &band((-d).min(top), top, top_color));
    batch.add_quad(None, &band(bottom, d.max(bottom), bottom_color));
    let strip = Transform::ortho(0.0, 1.0, top, bottom).matrix();
    context.draw(&mut batch.all(), &strip);
    context.draw(&mut graphics.background(), &strip);
    let world = Transform::ortho(left, right, top, bottom).matrix();
    context.draw(&mut graphics.scenery_back(), &world);
    context.draw(&mut graphics.polys_back(), &world);
    context.draw(&mut graphics.scenery_mid(), &world);
    context.draw(&mut graphics.polys_front(), &world);
    context.draw(&mut graphics.scenery_front(), &world);
    context.end_target();
    graphics.delete(context);
    Some(target)
}
