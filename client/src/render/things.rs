use super::*;
use gfx::SpriteData;

/// Draws a thing (`TThing.Render`): weapons along particles 1-2 (with their magazine, the
/// images for the side their owner faced), the parachute, flag handles, the stationary gun.
/// Flag cloths and kits come later in [`render_thing_polygons`].
pub fn render_thing(
    thing: &Thing,
    world: &World,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
    elapsed: f64,
) {
    let particles = thing.skeleton.particles();
    if particles.len() < 2 {
        return;
    }
    // realistic mode: an unseen soldier's things are unseen too
    let unseen = |id: Option<SoldierId>| {
        world.config.realistic_mode
            && id
                .and_then(|id| world.soldiers.get(id))
                .is_some_and(|s| s.active && s.visible == 0)
    };
    if unseen(thing.owner) {
        return;
    }

    let pos = |i: usize| {
        let p = &particles[i.min(particles.len() - 1)];
        lerp(p.old_pos, p.pos, frame_percent)
    };
    let (p1, p2, p3, p4) = (pos(0), pos(1), pos(2), pos(3));
    let mut painter = Painter { batch, sprites };
    let glow = (5.0 + 20.0 * (5.1 * elapsed).sin()).abs().round() as u8;

    // the Rambo bow lying about glows (`GameThingTarget`)
    if thing.kind == ThingKind::RamboBow {
        painter.ilum(p1 + (p2 - p1) / 2.0 - vec2(12.5, 12.5), glow);
    }

    use ThingKind::*;
    match thing.kind {
        AlphaFlag | BravoFlag | PointmatchFlag => {
            if unseen(thing.holding) || blinked_out(thing) {
                return;
            }
            let handle = gfx::Object::FlagHandle;
            let angle = vec2angle(p2 - p1);
            painter.draw(
                handle.group(),
                handle.id(),
                p1,
                Vec2::ONE,
                Vec2::ZERO,
                angle,
                rgb(255, 255, 255),
            );
            // a flag in its base glows
            if thing.in_base {
                painter.ilum(p1 + (p2 - p1) / 2.0 - vec2(12.5, 12.5), glow);
            }
        }
        Parachute => {
            use gfx::Soldier as G;
            let rope = |painter: &mut Painter, angle: f32| {
                let at = p4 - vec2(0.0, 0.55);
                let (group, id) = (G::ParaRope.group(), G::ParaRope.id());
                painter.draw(
                    group,
                    id,
                    at,
                    Vec2::ONE,
                    vec2(0.0, 0.55),
                    angle,
                    rgb(255, 255, 255),
                );
            };
            rope(&mut painter, vec2angle(p2 - p4));
            rope(&mut painter, vec2angle(p3 - p4) - 5f32.to_radians());
            rope(&mut painter, vec2angle(p1 - p4));
            // (too stretched: only the ropes)
            let scale = (p2 - p3).length() / 45.83;
            if scale > 2.0 {
                return;
            }
            let c = thing.color;
            let color = rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
            let (group, scale) = (G::Para.group(), Vec2::splat(scale));
            let angle = vec2angle(p1 - p3);
            painter.draw(group, G::Para2.id(), p3, scale, Vec2::ZERO, angle, color);
            let angle = vec2angle(p2 - p1);
            painter.draw(group, G::Para.id(), p1, scale, Vec2::ZERO, angle, color);
        }
        StationaryGun => {
            let base = gfx::Weapon::M2Stat;
            let angle = vec2angle(p2 - p3);
            let at = p3 - vec2(0.0, 20.0);
            painter.draw(
                base.group(),
                base.id(),
                at,
                Vec2::ONE,
                Vec2::ZERO,
                angle,
                rgb(255, 255, 255),
            );
            // redder as it heats up
            let barrel = if p4.x >= p1.x {
                gfx::Weapon::M22
            } else {
                gfx::Weapon::M2
            };
            let heat = thing.interest.clamp(0, 19) as u8;
            let color = rgb(255, 255 - 10 * heat, 255 - 13 * heat);
            let at = p1 - vec2(0.0, 13.0);
            let angle = -vec2angle(p1 - p4);
            painter.draw(
                barrel.group(),
                barrel.id(),
                at,
                Vec2::ONE,
                vec2(5.0, 4.0),
                angle,
                color,
            );
        }
        kind if kind.weapon().is_some() || kind == RamboBow => {
            // about to go: it blinks
            if blinked_out(thing) {
                return;
            }
            let Some((image, magazine)) = weapon_images(kind, &world.config.weapons) else {
                return;
            };
            let k = usize::from(thing.flip);
            let at = p1 - vec2(0.0, 3.0);
            let angle = vec2angle(p2 - p1);
            let group = gfx::Group::Weapon;
            for image in std::iter::once(image).chain(magazine) {
                painter.draw(
                    group,
                    image.id() + k,
                    at,
                    Vec2::ONE,
                    vec2(0.0, 2.0),
                    angle,
                    rgb(255, 255, 255),
                );
            }
        }
        _ => {}
    }
}

struct Painter<'a> {
    batch: &'a mut DrawBatch,
    sprites: &'a [Vec<Sprite>],
}

impl Painter<'_> {
    /// `GfxDrawSprite(s, x, y, sx, sy, rx, ry, -angle, color)`: the image's corner at `at`,
    /// scaled, turned by `angle` about `center`.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        group: gfx::Group,
        id: usize,
        at: Vec2,
        scale: Vec2,
        center: Vec2,
        angle: f32,
        color: Color,
    ) {
        let Some(sprite) = self.sprites[group.id()].get(id) else {
            return;
        };
        let transform = Transform::origin(at, scale, (angle, center));
        self.batch.add_sprite(sprite, color, transform);
    }

    /// The pulsing glow (`GFX_OBJECTS_ILUM`).
    fn ilum(&mut self, at: Vec2, alpha: u8) {
        let ilum = gfx::Object::Ilum;
        let color = rgba(255, 255, 255, alpha);
        self.draw(
            ilum.group(),
            ilum.id(),
            at,
            Vec2::ONE,
            Vec2::ZERO,
            0.0,
            color,
        );
    }
}

/// A weapon thing's image and magazine (`Tex1`, `Tex2` of `CreateThing`): the pistols and
/// the bow have images of their own lying about; only some rifles show their magazine.
fn weapon_images(
    kind: ThingKind,
    weapons: &WeaponTable,
) -> Option<(gfx::Weapon, Option<gfx::Weapon>)> {
    use ThingKind::*;
    match kind {
        Ussocom => return Some((gfx::Weapon::NSocom, None)),
        DesertEagle => return Some((gfx::Weapon::NDeagles, None)),
        RamboBow => return Some((gfx::Weapon::NBow, None)),
        _ => {}
    }
    let gun = weapons.get(kind.weapon()?);
    Some(match kind {
        HkMp5 | Ak74 | SteyrAug | BarrettM82A1 | Minimi => (gun.sprite?, gun.clip_sprite),
        _ => (gun.sprite?, None),
    })
}

/// A flag about to return home, a weapon about to go, blinks (`TimeOut < 300`).
fn blinked_out(thing: &Thing) -> bool {
    thing.timeout < 300 && thing.timeout.rem_euclid(6) < 3
}

/// `TThing.PolygonsRender`: flag cloths and kits as a textured quad over the four
/// particles, shaded per corner.
pub fn render_thing_polygons(
    thing: &Thing,
    world: &World,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
) {
    let particles = thing.skeleton.particles();
    if particles.len() < 4 {
        return;
    }
    let infiltration = world.config.game_mode == GameMode::Infiltration;
    // realistic mode: an unseen soldier's flag is unseen too
    let unseen_holder = thing
        .holding
        .and_then(|id| world.soldiers.get(id))
        .is_some_and(|s| s.active && s.visible == 0);
    if thing.kind.is_flag() && world.config.realistic_mode && unseen_holder {
        return;
    }

    use ThingKind::*;
    use gfx::Object;
    // texture and the colors of the base, low and top corners
    let (texture, base, low, top) = match thing.kind {
        AlphaFlag if infiltration => (Object::Infflag, 0xEEEEEE, 0xDDEEEE, 0xFFEEEE),
        AlphaFlag => (Object::Flag, 0xAD1515, 0x951515, 0xB51515),
        BravoFlag if infiltration => (Object::Infflag, 0x333333, 0x333322, 0x333344),
        BravoFlag => (Object::Flag, 0x0510AD, 0x051095, 0x0510B5),
        PointmatchFlag => (Object::Flag, 0xADAD15, 0x959515, 0xB5B515),
        MedicalKit => (Object::Medikit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        GrenadeKit => (Object::Grenadekit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        FlamerKit => (Object::Flamerkit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        PredatorKit => (Object::Predatorkit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        VestKit => (Object::Vestkit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        BerserkKit => (Object::Berserkerkit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        ClusterKit => (Object::Clusterkit, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
        _ => return,
    };
    if thing.kind.is_flag() && blinked_out(thing) {
        return;
    }
    let color = |c: u32| rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);

    let pos = |i: usize| lerp(particles[i].old_pos, particles[i].pos, frame_percent);
    let (mut p1, p2, p3, p4) = (pos(0), pos(1), pos(2), pos(3));
    // the cloth starts halfway up the pole
    if thing.kind.is_flag() {
        p1 += (p2 - p1) * 0.5;
    }

    let sprite = &sprites[texture.group().id()][texture.id()];
    let (left, right) = sprite.texcoords_x;
    let (top_t, bottom) = sprite.texcoords_y;
    batch.add_quad(
        sprite.texture.as_ref(),
        &[
            vertex(p2, vec2(left, top_t), color(base)),
            vertex(p1, vec2(left, bottom), color(top)),
            vertex(p4, vec2(right, bottom), color(base)),
            vertex(p3, vec2(right, top_t), color(low)),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// ctf_Ash with the game's files, if they're here.
    fn world() -> Option<World> {
        let path = std::env::var_os("SOLDANK_ASSETS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets")
            });
        if !path.exists() {
            eprintln!("skipping: no assets at {}", path.display());
            return None;
        }
        let mut vfs = Vfs::new();
        vfs.mount_game_files(&path).unwrap();
        let data = Arc::new(GameData::load(&vfs).unwrap());
        let map = MapFile::load(&vfs, "ctf_Ash").unwrap();
        let mut cvars = soldank_core::config::Cvars::new();
        register_cvars(&mut cvars);
        let config = WorldConfig::from_cvars(&cvars, &data);
        Some(World::new(data, map, config))
    }

    /// Sprites to draw with: any size, no texture.
    fn sprites() -> Vec<Vec<Sprite>> {
        let sprite = Sprite {
            width: 10.0,
            height: 2.0,
            texcoords_x: (0.0, 1.0),
            texcoords_y: (0.0, 1.0),
            texture: None,
        };
        vec![vec![sprite; 512]; gfx::Group::values().len()]
    }

    fn drawn(world: &World, kind: ThingKind) -> usize {
        let thing = world
            .things
            .iter()
            .find(|t| t.active && t.kind == kind)
            .unwrap();
        let mut batch = DrawBatch::new();
        render_thing(thing, world, &sprites(), &mut batch, 0.5, 0.0);
        batch.len() / 6
    }

    #[test]
    fn parachutes_and_dropped_weapons_show() {
        let Some(mut world) = world() else {
            return;
        };
        let id = world.spawn_soldier();
        let pos = world.soldiers[id].particle.pos;
        world.soldiers[id].parachute_spawn = Some(pos - vec2(0.0, 300.0));
        world.step(&[]);
        // three ropes and the two halves of the chute
        assert_eq!(drawn(&world, ThingKind::Parachute), 5);

        // an MP5 with its magazine; Deagles as a single gun
        for (weapon, kind, sprites) in [
            (WeaponKind::MP5, ThingKind::HkMp5, 2),
            (WeaponKind::DesertEagles, ThingKind::DesertEagle, 1),
        ] {
            let gun = world.config.weapons.get(weapon);
            let soldier = &mut world.soldiers[id];
            soldier.weapons[soldier.active_weapon] = gun;
            let config = world.config.clone();
            world.soldiers[id].drop_weapon(&config);
            world.step(&[]);
            assert_eq!(drawn(&world, kind), sprites, "{kind:?}");
        }
    }
}
