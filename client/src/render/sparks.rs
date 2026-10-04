//! Drawing sparks (`TSpark.Render`).

use super::*;
use crate::sparks::Sparks;
use gfx::{Spark as S, SpriteData, Weapon as W};

/// `RGBA(rgb, a)`: the alpha wraps like Soldat's `Byte(Trunc(a))`.
fn tint(rgb: u32, a: f32) -> Color {
    let a = (a.trunc() as i64 & 0xFF) as u8;
    rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, a)
}

const WHITE: u32 = 0xFFFFFF;

struct Painter<'a> {
    batch: &'a mut DrawBatch,
    sprites: &'a [Vec<Sprite>],
}

impl Painter<'_> {
    /// `GfxDrawSprite(s, x, y, sx, sy, 0, 0, r, color)`: scaled, turned about its corner.
    fn draw(&mut self, sprite: &Sprite, p: Vec2, scale: Vec2, r: f32, color: Color) {
        let transform = Transform::origin(p, scale, (-r, Vec2::ZERO));
        self.batch.add_sprite(sprite, color, transform);
    }

    fn spark(&mut self, s: S, p: Vec2, scale: f32, color: Color) {
        let sprite = &self.sprites[gfx::Group::Spark.id()][s.id()];
        self.draw(sprite, p, vec2(scale, scale), 0.0, color);
    }

    fn spark_scaled(&mut self, s: S, p: Vec2, scale: Vec2, r: f32, color: Color) {
        let sprite = &self.sprites[gfx::Group::Spark.id()][s.id()];
        self.draw(sprite, p, scale, r, color);
    }

    fn weapon(&mut self, w: W, p: Vec2, scale: Vec2, r: f32, color: Color) {
        let sprite = &self.sprites[gfx::Group::Weapon.id()][w.id()];
        self.draw(sprite, p, scale, r, color);
    }

    /// Frame `i` of the sparks (explosion and smoke animations count down from a frame).
    fn frame(&mut self, base: S, back: usize, p: Vec2, scale: f32, color: Color) {
        let id = base.id().saturating_sub(back);
        let sprite = &self.sprites[gfx::Group::Spark.id()][id];
        self.draw(sprite, p, vec2(scale, scale), 0.0, color);
    }
}

/// An explosion animation: the frame before it fades under it.
fn explosion(painter: &mut Painter, p: Vec2, back: usize, scale: f32, under: u32, alpha: f32) {
    let first = S::ExplosionExplode1.id();
    let i = S::ExplosionExplode16.id().saturating_sub(back);
    if i > first {
        painter.frame(
            S::ExplosionExplode16,
            back + 1,
            p,
            scale,
            tint(under, 100.0),
        );
    }
    painter.frame(S::ExplosionExplode16, back, p, scale, tint(WHITE, alpha));
}

pub fn render_sparks(
    sparks: &Sparks,
    world: &World,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
) {
    let mut painter = Painter { batch, sprites };
    let deg = |d: f32| d.to_radians();

    for spark in sparks.sparks.iter().filter(|s| s.active) {
        let owner = spark.owner.and_then(|id| world.soldiers.get(id));
        // realistic mode: sparks of the unseen stay unseen
        if world.config.realistic_mode && owner.is_some_and(|o| o.visible == 0) {
            continue;
        }
        let p = lerp(spark.particle.old_pos, spark.particle.pos, frame_percent);
        let l = f32::from(spark.life_prev)
            + (f32::from(spark.life) - f32::from(spark.life_prev)) * frame_percent;
        let shirt = owner.map_or(WHITE, |o| o.looks.shirt);
        let one = Vec2::ONE;

        match spark.style {
            1 => painter.spark(S::Smoke, p, 1.0, tint(WHITE, l + 10.0)),
            2 => painter.spark(S::Lilfire, p, 1.0, tint(WHITE, l)),
            3 => painter.spark(S::Odprysk, p, 1.0, tint(WHITE, l * 3.0 + 10.0)),
            4 => painter.spark_scaled(
                S::Lilblood,
                p,
                vec2(0.75, 0.75),
                deg(l * 10.0),
                tint(WHITE, l * 2.0 + 65.0),
            ),
            5 => {
                let s = if l > 10.0 { 0.33 + 10.0 / l } else { 1.0 };
                painter.spark_scaled(
                    S::Blood,
                    p,
                    vec2(s, s),
                    deg(l * 2.0),
                    tint(WHITE, l * 2.0 + 85.0),
                );
            }
            6 => {
                if let Some(owner) = owner {
                    let cap = match owner.head_cap {
                        1 => gfx::Soldier::Helm,
                        2 => gfx::Soldier::Kap,
                        _ => continue,
                    };
                    let sprite = &sprites[gfx::Group::Soldier.id()][cap.id()];
                    painter.draw(sprite, p, one, deg(l * 2.0), tint(shirt, 255.0));
                }
            }
            7 => painter.weapon(W::Shell, p, one, deg(l * 4.0), tint(WHITE, 255.0)),
            8 => painter.weapon(
                W::Shell,
                p,
                vec2(1.1, 1.2),
                deg(l * 3.5),
                tint(0xFF3333, 255.0),
            ),
            9 => painter.weapon(W::Ak74Clip, p + vec2(8.0, 0.0), one, PI, tint(WHITE, 255.0)),
            10 => painter.weapon(
                W::MinimiClip,
                p + vec2(8.0, 0.0),
                one,
                PI,
                tint(WHITE, 255.0),
            ),
            11 => painter.weapon(W::Mp5Clip, p + vec2(8.0, 0.0), one, PI, tint(WHITE, 255.0)),
            12 => explosion(
                &mut painter,
                p - vec2(19.0, 38.0),
                (l / 4.0).round() as usize,
                0.75,
                0xADADAD,
                255.0 - EXPLOSION_FADE + l,
            ),
            13 => painter.frame(
                S::ExplosionExplode16,
                (l / 3.0).round() as usize,
                p - vec2(8.0, 17.0),
                0.3,
                tint(WHITE, 255.0 - 2.0 * l),
            ),
            14 => explosion(
                &mut painter,
                p - vec2(50.0, 100.0),
                (l / 3.0).round() as usize,
                2.0,
                0xADADAD,
                255.0 - 2.0 * l,
            ),
            15 => explosion(
                &mut painter,
                p - vec2(75.0, 150.0),
                (l / 3.0).round() as usize,
                3.0,
                0xADADAD,
                255.0 - 2.0 * l,
            ),
            16 => painter.weapon(
                W::Shell,
                p,
                vec2(1.0, 2.0),
                deg(l * 3.5),
                tint(0x8877FF, 255.0),
            ),
            17 => explosion(
                &mut painter,
                p - vec2(25.0, 50.0),
                (l / 4.0).round() as usize,
                1.0,
                0xAAAAAA,
                255.0 - EXPLOSION_FADE + l,
            ),
            18 => painter.weapon(
                W::DeaglesClip,
                p + vec2(8.0, 0.0),
                one,
                PI,
                tint(WHITE, 255.0),
            ),
            19 => painter.weapon(
                W::SteyrClip,
                p + vec2(8.0, 0.0),
                one,
                PI,
                tint(WHITE, 255.0),
            ),
            20 => painter.weapon(
                W::BarrettClip,
                p + vec2(8.0, 0.0),
                one,
                PI,
                tint(WHITE, 255.0),
            ),
            21 => painter.weapon(
                W::Shell,
                p,
                vec2(1.1, 1.0),
                deg(l * 3.77),
                tint(WHITE, 255.0),
            ),
            22 => painter.weapon(
                W::Shell,
                p,
                vec2(1.3, 1.0),
                deg(l * 3.5),
                tint(WHITE, 255.0),
            ),
            23 => painter.weapon(
                W::SocomClip,
                p + vec2(8.0, 0.0),
                one,
                PI,
                tint(WHITE, 255.0),
            ),
            24 => {
                let s = vec2(0.6 + (75.0 / l) / 126.0, 0.6 + (75.0 / l) / 120.0);
                let q = vec2(p.x - 22.0 * s.x, p.y - 64.0 + l / 2.0);
                painter.spark_scaled(S::Bigsmoke, q, s, 0.0, tint(WHITE, (3.0 * l).trunc()));
            }
            25 => painter.spark_scaled(
                S::Spawnspark,
                p - vec2(20.0, 20.0),
                one,
                deg(l),
                tint(shirt, (6.0 * l).min(255.0)),
            ),
            26 => painter.spark(
                S::Odprysk,
                p,
                1.0,
                tint(0xFFFE35, (l * 3.0 + 154.0).min(255.0)),
            ),
            27 => painter.spark(
                S::Odprysk,
                p,
                1.0,
                tint(0xAAAAAA, (l * 3.0 + 154.0).min(255.0)),
            ),
            28 => painter.frame(
                S::ExplosionExplode16,
                (l / 3.0).round() as usize,
                p - vec2(15.0, 37.0),
                0.5,
                tint(WHITE, 255.0 - 2.0 * l),
            ),
            29 => {
                let s = vec2(
                    0.5 * (0.6 + (75.0 / l) / 96.0),
                    0.5 * (0.6 + (75.0 / l) / 90.0),
                );
                let q = vec2(p.x - 22.0 * s.x, p.y - 48.0 + l / 1.5);
                painter.spark_scaled(S::Bigsmoke, q, s, 0.0, tint(WHITE, (2.5 * l).trunc()));
            }
            30 => painter.spark_scaled(S::Pin, p, one, deg(l * 4.0), tint(WHITE, 255.0)),
            31 => painter.spark(S::Lilsmoke, p, 1.0, tint(WHITE, l + 10.0)),
            32 => painter.spark(S::Stuff, p, 1.0, tint(WHITE, l + 10.0)),
            33 => painter.weapon(W::Shell, p, one, deg(l * 4.0), tint(0xBBAAA9, 255.0)),
            34 => painter.spark_scaled(S::Cygaro, p, one, deg(l * 4.0), tint(WHITE, 255.0)),
            35 => painter.spark(S::Lilsmoke, p, 1.0, tint(WHITE, l * 13.0)),
            36 | 64 => {
                let s = l / 35.0;
                let alpha = if spark.style == 36 {
                    (l * 2.0 + 185.0).min(255.0)
                } else {
                    l * 2.0 + 185.0
                };
                painter.spark(S::Plomyk, vec2(p.x, p.y - 1.0 / s), s, tint(WHITE, alpha));
            }
            37 => {
                let s = l / 75.0;
                painter.spark(
                    S::Blacksmoke,
                    vec2(p.x, p.y - 1.0 / s),
                    s,
                    tint(WHITE, l * 3.0),
                );
            }
            38 => painter.spark(S::Rain, p, 1.0, tint(WHITE, 105.0)),
            39 => painter.spark(S::Sand, p, 1.0, tint(WHITE, 105.0)),
            40..=43 => {
                let s = [S::Odlamek1, S::Odlamek2, S::Odlamek3, S::Odlamek4]
                    [usize::from(spark.style - 40)];
                painter.spark_scaled(s, p, one, deg(l * 8.0), tint(WHITE, (l + 10.0).trunc()));
            }
            44..=47 => {
                let s = [S::Odlamek1, S::Odlamek2, S::Odlamek3, S::Odlamek4]
                    [usize::from(spark.style - 44)];
                painter.spark(s, p, 0.7, tint(WHITE, (l * 2.0).trunc() + 15.0));
            }
            48 | 49 => {
                let color = if spark.style == 48 {
                    shirt
                } else {
                    owner.map_or(WHITE, |o| o.looks.pants)
                };
                painter.spark_scaled(
                    S::Skrawek,
                    p,
                    one,
                    deg(l * 5.0),
                    tint(color, (l * 2.0).trunc() + 15.0),
                );
            }
            50 => painter.spark(S::Puff, p, 1.0, tint(WHITE, l.trunc() + 5.0)),
            51 => painter.weapon(W::SpasShell, p, one, deg(l * 3.77), tint(WHITE, 255.0)),
            52 => painter.weapon(W::M79Shell, p, one, deg(l * 3.77), tint(WHITE, 255.0)),
            53 => painter.spark(S::Snow, p, 1.0, tint(WHITE, 105.0)),
            54 => {
                if l <= SMOKE_ANIMS * 4.0 {
                    let q = p - vec2(26.0, 48.0);
                    let back = (l / 4.0).round() as usize;
                    if S::Minismoke.id().saturating_sub(back) > S::ExplosionSmoke1.id() {
                        painter.frame(
                            S::Minismoke,
                            back + 1,
                            q,
                            1.0,
                            tint(0xCCCCCC, 2.0 * l + 10.0),
                        );
                    }
                    painter.frame(S::Minismoke, back, q, 1.0, tint(0xDDDDDD, 3.0 * l + 10.0));
                }
            }
            55 => {
                let s = if l > 20.0 { 0.63 + 10.0 / l } else { 1.0 };
                painter.spark_scaled(
                    S::Splat,
                    p,
                    vec2(s, s),
                    deg(l),
                    tint(WHITE, (l * 2.0 + 55.0).min(255.0)),
                );
            }
            56 => painter.spark(
                S::Minismoke,
                p - vec2(3.0, 3.0),
                1.0,
                tint(WHITE, (2.5 * l).trunc()),
            ),
            57 => painter.spark(S::Odprysk, p, 1.0, tint(0xFFFF00, l * 2.0 + 10.0)),
            58 => painter.spark(S::Odprysk, p, 1.0, tint(0xFFFF00, l * 3.0 + 10.0)),
            59 => {
                let s = 1.5 + (620.0 / l) / 50.0;
                painter.spark(
                    S::Smoke,
                    vec2(p.x, p.y - (50.0 - l)),
                    s,
                    tint(WHITE, l * 2.0),
                );
            }
            60 => {
                let s = 0.5 + 16.0 / (l + 50.0);
                let q = vec2(p.x - 14.0 * s, p.y - 30.0);
                painter.spark(S::Bigsmoke, q, s, tint(WHITE, (l / 3.3).trunc()));
                let second = if l > 30.0 {
                    tint(0x666666, ((255.0 - l) / 9.0).trunc())
                } else {
                    tint(0xBFBFBF, l.trunc())
                };
                painter.spark(S::Bigsmoke2, q, s, second);
            }
            61 => painter.spark_scaled(
                S::Spawnspark,
                p - vec2(10.0, 10.0),
                vec2(0.5, 0.5),
                deg(l),
                tint(shirt, 14.0 * l),
            ),
            62 => {
                let jet = owner.map_or(WHITE, |o| o.looks.jet);
                painter.spark_scaled(S::Jetfire, p, one, deg(l), tint(jet, l * 5.0));
            }
            65..=73 => {
                let shells = [
                    W::ColtShell,
                    W::DeaglesShell,
                    W::Mp5Shell,
                    W::Ak74Shell,
                    W::SteyrShell,
                    W::RugerShell,
                    W::BarrettShell,
                    W::MinimiShell,
                    W::MinigunShell,
                ];
                let turn = if spark.style == 71 { 3.5 } else { 4.0 };
                painter.weapon(
                    shells[usize::from(spark.style - 65)],
                    p,
                    one,
                    deg(l * turn),
                    tint(WHITE, 255.0),
                );
            }
            _ => {}
        }
    }
}

/// `EXPLOSION_ANIMS * 5`, `SMOKE_ANIMS`
const EXPLOSION_FADE: f32 = 16.0 * 5.0;
const SMOKE_ANIMS: f32 = 10.0;
