//! The menus' previews, each in a texture egui shows: the player's soldier with the settings'
//! looks and secondary weapon (standing in a little world of its own, drawn by the game's own
//! renderer), and the picked map, whole.

use crate::menus::PRIMARY_WEAPONS;
use crate::render::GameGraphics;
use crate::render::minimap::render_map_preview;
use egui::TextureId;
use gfx2d::{Gfx2dContext, RenderTarget};
use soldank_core::assets::Vfs;
use soldank_core::config::Console;
use soldank_core::*;
use std::sync::Arc;

/// The soldier preview's texture (pixels), and how much of the game it shows around the
/// soldier.
const SOLDIER_PIXELS: (u16, u16) = (240, 300);
const SOLDIER_VIEW: Vec2 = Vec2::new(40.0, 50.0);
/// The most a map preview gets (pixels).
const MAP_PIXELS: (u16, u16) = (720, 405);

struct SoldierPreview {
    graphics: GameGraphics,
    world: World,
    id: SoldierId,
    target: RenderTarget,
    texture: TextureId,
}

struct MapPreview {
    name: String,
    /// `None`: a map without polygons.
    target: Option<(RenderTarget, TextureId)>,
    /// The map's title (`mapname`).
    pub title: String,
}

/// What the menus showed last frame is drawn before this one's.
#[derive(Default)]
pub struct Previews {
    soldier: Option<SoldierPreview>,
    map: Option<MapPreview>,
    /// Set by the menus each frame they show a preview.
    pub want_soldier: bool,
    pub want_map: Option<String>,
}

/// egui's name for a texture of ours: its GL name.
fn egui_texture(context: &Gfx2dContext, target: &RenderTarget) -> TextureId {
    // SAFETY: the texture is the target's, alive as long as the preview
    let raw = unsafe { context.ctx.texture_raw_id(target.texture().id()) };
    #[allow(unreachable_patterns)]
    match raw {
        gfx2d::mq::RawId::OpenGl(id) => TextureId::User(u64::from(id)),
        _ => TextureId::default(),
    }
}

impl SoldierPreview {
    fn new(context: &mut Gfx2dContext, vfs: &Vfs, console: &Console) -> Option<SoldierPreview> {
        let data = Arc::new(GameData::load(vfs).ok()?);
        let map = MapFile::load(vfs, &crate::app::first_map(vfs)).ok()?;
        let config = WorldConfig::from_cvars(&console.cvars, &data);
        let mut world = World::new(data, map, config);
        let id = world.spawn_soldier();
        world.respawn_soldier(id);
        let mut graphics = GameGraphics::new();
        graphics.load_sprites(context, vfs, console.cvars.string("ui_style"));
        let target = context.new_render_target(SOLDIER_PIXELS);
        let texture = egui_texture(context, &target);
        Some(SoldierPreview {
            graphics,
            world,
            id,
            target,
            texture,
        })
    }

    /// A tick of standing there with the settings' looks and weapon, drawn.
    fn update(&mut self, context: &mut Gfx2dContext, console: &Console) {
        if self.world.game.ended() || !self.world.soldiers.contains_key(self.id) {
            return;
        }
        let cvars = &console.cvars;
        let secondary = cvars.int("cl_player_secwep").clamp(0, 3) as usize;
        let kind = WeaponKind::values()[PRIMARY_WEAPONS + secondary];
        let gun = self.world.config.weapons.get(kind);
        let soldier = &mut self.world.soldiers[self.id];
        crate::player_net_looks(cvars).apply(soldier);
        if soldier.weapons[soldier.active_weapon].kind != kind {
            soldier.weapons[soldier.active_weapon] = gun;
            // nothing on the back: only what the settings pick
            let other = (soldier.active_weapon + 1) % 2;
            soldier.weapons[other] = Weapon::new(WeaponKind::NoWeapon, false);
        }
        // looking a little up, to the right
        let input = Input {
            buttons: Buttons::empty(),
            aim: soldier.particle.pos + vec2(200.0, -40.0),
        };
        self.world.step(&[(self.id, input)]);

        let soldier = &self.world.soldiers[self.id];
        let points: Vec<Vec2> = (1..=soldier.skeleton.len())
            .map(|i| soldier.skeleton.pos(i))
            .collect();
        let min = points.iter().fold(Vec2::splat(f32::MAX), |a, &p| a.min(p));
        let max = points.iter().fold(Vec2::splat(f32::MIN), |a, &p| a.max(p));
        let center = (min + max) / 2.0;
        let view = (center - SOLDIER_VIEW / 2.0, center + SOLDIER_VIEW / 2.0);
        self.graphics
            .render_soldier_preview(context, soldier, &self.target, view);
    }

    fn delete(mut self, context: &mut Gfx2dContext) {
        self.graphics.delete(context);
        context.delete_render_target(self.target);
    }
}

impl MapPreview {
    fn new(context: &mut Gfx2dContext, vfs: &Vfs, name: &str) -> MapPreview {
        let map = MapFile::load(vfs, name).ok();
        let target = map
            .as_ref()
            .and_then(|map| render_map_preview(context, vfs, map, MAP_PIXELS))
            .map(|target| {
                let texture = egui_texture(context, &target);
                (target, texture)
            });
        MapPreview {
            name: name.to_string(),
            target,
            title: map.map_or_else(String::new, |m| m.mapname.trim().to_string()),
        }
    }

    fn delete(self, context: &mut Gfx2dContext) {
        if let Some((target, _)) = self.target {
            context.delete_render_target(target);
        }
    }
}

impl Previews {
    /// Before the menus are drawn: the previews they showed last frame, drawn now.
    pub fn update(&mut self, context: &mut Gfx2dContext, vfs: &Vfs, console: &Console) {
        if std::mem::take(&mut self.want_soldier) {
            if self.soldier.is_none() {
                self.soldier = SoldierPreview::new(context, vfs, console);
            }
            if let Some(preview) = &mut self.soldier {
                preview.update(context, console);
            }
        }
        if let Some(name) = self.want_map.take()
            && self.map.as_ref().is_none_or(|m| m.name != name)
        {
            if let Some(old) = self.map.take() {
                old.delete(context);
            }
            self.map = Some(MapPreview::new(context, vfs, &name));
        }
    }

    /// The soldier's texture and its size, once drawn.
    pub fn soldier(&self) -> Option<(TextureId, egui::Vec2)> {
        let size = egui::vec2(f32::from(SOLDIER_PIXELS.0), f32::from(SOLDIER_PIXELS.1));
        self.soldier.as_ref().map(|p| (p.texture, size))
    }

    /// The map's texture, its size and title, once drawn (of the map asked for).
    pub fn map(&self, name: &str) -> Option<(Option<(TextureId, egui::Vec2)>, &str)> {
        let preview = self.map.as_ref().filter(|m| m.name == name)?;
        let texture = preview.target.as_ref().map(|(target, texture)| {
            let (w, h) = target.texture().dimensions();
            (*texture, egui::vec2(f32::from(w), f32::from(h)))
        });
        Some((texture, &preview.title))
    }

    /// Everything goes (a session starts: it has its own graphics).
    pub fn clear(&mut self, context: &mut Gfx2dContext) {
        if let Some(preview) = self.soldier.take() {
            preview.delete(context);
        }
        if let Some(preview) = self.map.take() {
            preview.delete(context);
        }
    }
}
