//! Drawing bullets (`TBullet.Render`): every style with its sprite and trail.

use super::*;
use gfx::{Spark as S, SpriteData, Weapon as W};

/// `BULLETTRAIL`, `BULLETLENGTH`, `BULLETALPHA`.
const BULLET_TRAIL: f32 = 13.0;
const BULLET_LENGTH: f32 = 21.0;
const BULLET_ALPHA: u8 = 110;
/// The timeouts (`BULLET_TIMEOUT`, `GRENADE_TIMEOUT`, `M2BULLET_TIMEOUT`, `FLAMER_TIMEOUT`)
/// and `ARROW_RESIST`, in ticks.
const BULLET_TIMEOUT: f32 = 7.0 * 60.0;
const GRENADE_TIMEOUT: f32 = 3.0 * 60.0;
const M2BULLET_TIMEOUT: f32 = 60.0;
const FLAMER_TIMEOUT: f32 = 32.0;
const ARROW_RESIST: f32 = 280.0;

/// The guns' bullet images; any other draws as the Colt's.
const BULLET_IMAGES: [W; 9] = [
    W::DeaglesBullet,
    W::Mp5Bullet,
    W::Ak74Bullet,
    W::SteyrBullet,
    W::RugerBullet,
    W::BarrettBullet,
    W::MinimiBullet,
    W::MinigunBullet,
    W::ColtBullet,
];

fn color(rgb: u32, alpha: u8) -> Color {
    rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, alpha)
}

struct Painter<'a> {
    batch: &'a mut DrawBatch,
    sprites: &'a [Vec<Sprite>],
}

impl Painter<'_> {
    /// `GfxDrawSprite(s, x, y, sx, sy, rx, ry, r, color)`: scaled, turned by `r` about
    /// `center`.
    fn draw(&mut self, w: W, p: Vec2, scale: Vec2, center: Vec2, r: f32, color: Color) {
        let sprite = &self.sprites[gfx::Group::Weapon.id()][w.id()];
        let transform = Transform::origin(p, scale, (-r, center));
        self.batch.add_sprite(sprite, color, transform);
    }

    /// `GfxDrawSprite(s, x, y, sx, sy, 0, 0, r, color)`.
    fn turned(&mut self, w: W, p: Vec2, scale: Vec2, r: f32, color: Color) {
        self.draw(w, p, scale, Vec2::ZERO, r, color);
    }

    /// Frame `back` before the last of the flames.
    fn flame(&mut self, back: usize, p: Vec2) {
        let id = S::FlamesExplode16.id().saturating_sub(back);
        let sprite = &self.sprites[gfx::Group::Spark.id()][id];
        self.batch
            .add_sprite(sprite, rgb(255, 255, 255), Transform::Pos(p));
    }
}

/// `-Angle2Points(p1, p2)`: what a sprite along `p1` to `p2` is drawn turned by.
fn roto(p1: Vec2, p2: Vec2) -> f32 {
    -vec2angle(p2 - p1)
}

/// Where a grenade's trail runs beside it: off its corner away from the way it flies.
fn trail_offset(velocity: Vec2) -> Vec2 {
    vec2(
        if velocity.y > 0.0 { -1.0 } else { 1.0 },
        if velocity.x > 0.0 { 1.0 } else { -1.0 },
    )
}

/// A bullet as the frame has it, between its last two ticks.
pub fn render_bullet(
    bullet: &Bullet,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    elapsed: f64,
    frame_percent: f32,
) {
    let p = frame_percent;
    let pos = lerp(bullet.particle.old_pos, bullet.particle.pos, p);
    let vel = lerp(bullet.velocity_prev, bullet.particle.velocity, p);
    let hit_multiply = lerp(bullet.hit_multiply_prev, bullet.hit_multiply, p);
    let t = lerp(f32::from(bullet.timeout_prev), f32::from(bullet.timeout), p);
    let speed = vel.length();
    let sinus = (f64::from(t) + 5.1 * elapsed).sin() as f32;
    let white = |alpha: u8| color(0xFFFFFF, alpha);
    let mut painter = Painter { batch, sprites };

    match bullet.style {
        BulletStyle::Bullet if t < BULLET_TIMEOUT - 2.0 => {
            let image = bullet
                .sprite
                .filter(|s| BULLET_IMAGES.iter().any(|w| w.id() == s.id()))
                .unwrap_or(W::ColtBullet);
            let ahead = pos + vel;
            let r = roto(ahead, ahead - vel);
            let scale = vec2(speed / BULLET_TRAIL, 1.0);
            let mut alpha = ((hit_multiply * scale.x * scale.x) / 4.63 * 255.0).clamp(50.0, 230.0);
            if bullet.ping_add < 1 {
                painter.turned(image, ahead, scale, r, white(alpha.round() as u8));
            }
            // the way a late shot came
            if bullet.ping_add > 0 {
                let came = (pos - bullet.initial_pos).length();
                let left = f32::from(bullet.ping_add + 2) / f32::from(bullet.ping_add_start);
                let length = came * (1.0 / BULLET_LENGTH).min(left / BULLET_TRAIL);
                alpha /= if bullet.active { 6.0 } else { 4.0 };
                painter.turned(
                    image,
                    ahead,
                    vec2(length, 1.0),
                    r,
                    white(alpha.round() as u8),
                );
            }
            if t < BULLET_TIMEOUT - 7.0 {
                // through a body it's redder
                let (length, tint) = match bullet.hit_body {
                    Some(_) => (speed / 4.0, 0xFFDDDD),
                    None => (speed / 3.5, 0xFFFFFF),
                };
                let trail = color(tint, BULLET_ALPHA / 2);
                painter.turned(image, pos, vec2(length, 1.0), r, trail);
            }
        }
        BulletStyle::FragGrenade => {
            if t < GRENADE_TIMEOUT - 3.0 {
                let at = pos + trail_offset(vel) - vec2(0.0, 3.0);
                let r = roto(at, at - vel);
                // BULLETALPHA * 0.75, rounded half to even
                let trail = color(0x64FF64, 82);
                painter.turned(W::Bullet, at, vec2(speed / 3.0, 1.0), r, trail);
            }
            painter.turned(
                W::FragGrenade,
                pos - vec2(1.0, 4.0),
                Vec2::ONE,
                0.0,
                white(255),
            );
        }
        BulletStyle::GaugeBullet if t < BULLET_TIMEOUT - 2.0 => {
            let ahead = pos + vel;
            let r = roto(ahead, ahead - vel);
            painter.turned(W::SpasBullet, ahead, Vec2::ONE, r, white(150));
            if t < BULLET_TIMEOUT - 3.0 {
                let trail = white(BULLET_ALPHA / 5);
                painter.turned(W::Bullet, pos, vec2(speed / 9.0, 1.0), r, trail);
            }
        }
        BulletStyle::M79Grenade if t < BULLET_TIMEOUT - 2.0 => {
            let at = pos + vec2(0.0, 1.0);
            let r = roto(at, at - vel);
            // it tumbles as it flies
            let spin = (t * 6.0).to_radians();
            painter.turned(W::M79Bullet, at, Vec2::ONE, spin, white(252));
            if t < BULLET_TIMEOUT - 4.0 {
                let at = pos + trail_offset(vel);
                let trail = color(0xFFFF55, BULLET_ALPHA);
                painter.turned(W::Bullet, at, vec2(speed / 4.0, 1.3), r, trail);
            }
        }
        BulletStyle::Flame => {
            if t > 0.0 && t <= FLAMER_TIMEOUT {
                // (a fresh one starts a sprite before the flames, like Soldat's)
                painter.flame((t / 2.0) as usize, pos - vec2(8.0, 17.0));
            }
        }
        BulletStyle::Arrow | BulletStyle::FlameArrow if t < BULLET_TIMEOUT - 2.0 => {
            let ahead = pos + vel;
            let r = roto(ahead, ahead - vel);
            painter.turned(W::Arrow, ahead, Vec2::ONE, r, white(255));
            if bullet.style == BulletStyle::Arrow && t > ARROW_RESIST {
                let trail = white(BULLET_ALPHA / 7);
                painter.turned(W::Bullet, pos, vec2(speed / 3.0, 1.0), r, trail);
            }
        }
        BulletStyle::ClusterGrenade => {
            let spin = (t * 5.0).to_radians() * if vel.x < 0.0 { -1.0 } else { 1.0 };
            painter.turned(
                W::ClusterGrenade,
                pos - vec2(0.0, 3.0),
                Vec2::ONE,
                spin,
                white(255),
            );
        }
        BulletStyle::Cluster => {
            painter.turned(W::Cluster, pos - vec2(0.0, 2.0), Vec2::ONE, 0.0, white(255));
        }
        BulletStyle::LAWMissile if t < BULLET_TIMEOUT - 2.0 => {
            let ahead = pos + vel;
            let r = roto(ahead, ahead - vel);
            painter.turned(W::Missile, ahead, Vec2::ONE, r, white(255));
            if t < BULLET_TIMEOUT - 7.0 {
                let trail = white(BULLET_ALPHA / 5);
                painter.turned(W::Bullet, pos, vec2(speed / 3.0, 1.0), r, trail);
            }
        }
        BulletStyle::ThrownKnife => {
            let ahead = pos + vel;
            let r = t / PI;
            let (knife, r) = if vel.x >= 0.0 {
                (W::Knife, r)
            } else {
                (W::Knife2, -r)
            };
            painter.draw(knife, ahead, Vec2::ONE, vec2(4.0, 1.0), r, white(255));
        }
        BulletStyle::M2Bullet if t < M2BULLET_TIMEOUT - 2.0 => {
            let ahead = pos + vel;
            let r = roto(ahead, ahead - vel);
            let scale = vec2(speed / BULLET_TRAIL, 1.2);
            painter.turned(
                W::Bullet,
                ahead,
                scale,
                r,
                color(0xFFBF77, BULLET_ALPHA * 2),
            );
            if t < M2BULLET_TIMEOUT - 13.0 {
                let trail = white(BULLET_ALPHA / 5);
                painter.turned(W::Bullet, pos, vec2(speed / 3.0, 1.0), r, trail);
                let smudge = vec2(speed / (sinus + 2.5), sinus);
                painter.turned(W::Smudge, pos, smudge, r, white(BULLET_ALPHA / 6));
            }
        }
        _ => {}
    }
}

/// Realistic mode: an unseen soldier's bullet is unseen too behind walls from the player
/// (`sv_realisticmode` in `TBullet.Render`).
pub fn bullet_hidden(
    bullet: &Bullet,
    world: &World,
    player: Option<SoldierId>,
    reach: f32,
    frame_percent: f32,
) -> bool {
    let owner_unseen = world
        .soldiers
        .get(bullet.owner)
        .is_some_and(|o| o.active && o.visible == 0);
    let Some(me) = player.and_then(|id| world.soldiers.get(id)) else {
        return false;
    };
    if !world.config.realistic_mode || !owner_unseen {
        return false;
    }
    let pos = lerp(bullet.particle.old_pos, bullet.particle.pos, frame_percent);
    let filter = RayCast {
        player: true,
        flag: false,
        bullet: true,
        check_collider: false,
        team: Team::None,
    };
    world
        .map
        .ray_cast(pos, me.skeleton.pos(9), reach, filter)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// How many sprites a bullet of `style` draws, `timeout` ticks before it's gone.
    fn drawn(style: BulletStyle, timeout: i16) -> usize {
        let bullet = Bullet {
            active: true,
            style,
            timeout,
            timeout_prev: timeout + 1,
            hit_multiply: 1.0,
            hit_multiply_prev: 1.0,
            particle: Particle {
                pos: vec2(100.0, 100.0),
                old_pos: vec2(90.0, 100.0),
                velocity: vec2(10.0, 0.0),
                ..Default::default()
            },
            velocity_prev: vec2(10.0, 0.0),
            ..Default::default()
        };
        let mut batch = DrawBatch::new();
        render_bullet(&bullet, &sprites(), &mut batch, 0.0, 0.5);
        batch.len() / 6
    }

    #[test]
    fn every_flying_bullet_shows() {
        use BulletStyle::*;
        // the M79's grenade and its trail; just fired, nothing a couple of ticks, then
        // no trail yet
        assert_eq!(drawn(M79Grenade, 100), 2);
        assert_eq!(drawn(M79Grenade, 419), 0);
        assert_eq!(drawn(M79Grenade, 416), 1);
        assert_eq!(drawn(FragGrenade, 100), 2);
        assert_eq!(drawn(Bullet, 100), 2);
        assert_eq!(drawn(GaugeBullet, 100), 2);
        assert_eq!(drawn(Flame, 20), 1);
        assert_eq!(drawn(Arrow, 300), 2);
        assert_eq!(drawn(FlameArrow, 300), 1);
        assert_eq!(drawn(ClusterGrenade, 100), 1);
        assert_eq!(drawn(Cluster, 100), 1);
        assert_eq!(drawn(LAWMissile, 100), 2);
        assert_eq!(drawn(ThrownKnife, 100), 1);
        assert_eq!(drawn(M2Bullet, 30), 3);
        // fists and blades are only there to hit
        assert_eq!(drawn(Fist, 100), 0);
        assert_eq!(drawn(Blade, 100), 0);
    }
}
