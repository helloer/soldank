use super::*;
use gfx::SpriteData;

/// Draws a thing (`TThing.Render`): weapons along particles 1-2, flag handles, stationary
/// guns. Flag cloths and kits come later in [`render_thing_polygons`].
/// TODO: parachutes, the bow, Soldat's exact weapon offsets.
pub fn render_thing(
    thing: &Thing,
    weapons: &WeaponTable,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
    elapsed: f64,
) {
    let particles = thing.skeleton.particles();
    if particles.len() < 2 {
        return;
    }

    let pos = |p: &Particle| lerp(p.old_pos, p.pos, frame_percent);
    let (p1, p2) = (pos(&particles[0]), pos(&particles[1]));

    // flags: the handle here, the cloth in the polygon pass
    if thing.kind.is_flag() {
        if flag_blinked_out(thing) {
            return;
        }
        let handle = gfx::Object::FlagHandle;
        batch.add_sprite(
            &sprites[handle.group().id()][handle.id()],
            rgb(255, 255, 255),
            Transform::WithPivot {
                pivot: Vec2::ZERO,
                pos: p1,
                scale: vec2(1.0, 1.0),
                rot: vec2angle(p2 - p1),
            },
        );
        // a flag in its base glows
        if thing.in_base {
            let ilum = gfx::Object::Ilum;
            let alpha = (5.0 + 20.0 * (5.1 * elapsed).sin()).abs().round() as u8;
            batch.add_sprite(
                &sprites[ilum.group().id()][ilum.id()],
                rgba(255, 255, 255, alpha),
                Transform::Pos(p1 + (p2 - p1) / 2.0 - vec2(12.5, 12.5)),
            );
        }
        return;
    }

    // kits are drawn in the polygon pass
    if thing.kind.is_kit() {
        return;
    }

    // stationary gun: the tripod along particles 3-2, the barrel from 1 toward 4, redder
    // as it heats up
    if thing.kind == ThingKind::StationaryGun && particles.len() >= 4 {
        let (p3, p4) = (pos(&particles[2]), pos(&particles[3]));
        let base = gfx::Weapon::M2Stat;
        batch.add_sprite(
            &sprites[base.group().id()][base.id()],
            rgb(255, 255, 255),
            Transform::WithPivot {
                pivot: Vec2::ZERO,
                pos: vec2(p3.x, p3.y - 20.0),
                scale: vec2(1.0, 1.0),
                rot: vec2angle(p2 - p3),
            },
        );
        let barrel = if p4.x >= p1.x {
            gfx::Weapon::M22
        } else {
            gfx::Weapon::M2
        };
        let heat = thing.interest.clamp(0, 19) as u8;
        batch.add_sprite(
            &sprites[barrel.group().id()][barrel.id()],
            rgb(255, 255 - 10 * heat, 255 - 13 * heat),
            Transform::WithPivot {
                pivot: vec2(5.0, 4.0),
                pos: vec2(p1.x, p1.y - 13.0),
                scale: vec2(1.0, 1.0),
                rot: -vec2angle(p1 - p4),
            },
        );
        return;
    }

    let sprite = if let Some(weapon) = thing.kind.weapon() {
        weapons.get(weapon).sprite
    } else {
        None
    };

    let (group, id) = match (sprite, thing.kind) {
        (Some(sprite), _) => (sprite.group().id(), sprite.id()),
        (None, kind) => {
            let object = match kind {
                ThingKind::MedicalKit => gfx::Object::Medikit,
                ThingKind::GrenadeKit => gfx::Object::Grenadekit,
                ThingKind::FlamerKit => gfx::Object::Flamerkit,
                ThingKind::PredatorKit => gfx::Object::Predatorkit,
                ThingKind::VestKit => gfx::Object::Vestkit,
                ThingKind::BerserkKit => gfx::Object::Berserkerkit,
                _ => return,
            };
            (object.group().id(), object.id())
        }
    };

    let sprite = &sprites[group][id];
    batch.add_sprite(
        sprite,
        rgb(255, 255, 255),
        Transform::WithPivot {
            pivot: vec2(0.0, 0.5 * sprite.height),
            pos: p1,
            scale: vec2(1.0, 1.0),
            rot: vec2angle(p2 - p1),
        },
    );
}

/// A flag about to return home blinks (`TimeOut < 300`).
fn flag_blinked_out(thing: &Thing) -> bool {
    thing.timeout < 300 && thing.timeout.rem_euclid(6) < 3
}

/// `TThing.PolygonsRender`: flag cloths and kits as a textured quad over the four
/// particles, shaded per corner.
pub fn render_thing_polygons(
    thing: &Thing,
    infiltration: bool,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
) {
    let particles = thing.skeleton.particles();
    if particles.len() < 4 {
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
    if thing.kind.is_flag() && flag_blinked_out(thing) {
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
