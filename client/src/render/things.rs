use super::*;
use gfx::SpriteData;

/// Draws a thing: its weapon or kit sprite along the line from particle 1 to particle 2.
/// TODO: flags (cloth), parachutes, stationary guns, Soldat's exact offsets.
pub fn render_thing(
    thing: &Thing,
    weapons: &WeaponTable,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
) {
    let particles = thing.skeleton.particles();
    if particles.len() < 2 {
        return;
    }

    let pos = |p: &Particle| lerp(p.old_pos, p.pos, frame_percent);
    let (p1, p2) = (pos(&particles[0]), pos(&particles[1]));

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
