//! Sparks (`Sparks.pas`): the client's particles. The simulation asks for them
//! ([`SparkSpawn`]); here they fly, bounce off the map, spawn smoke of their own and die.

use crate::audio::{Audio, Listener};
use soldank_core::*;

/// Soldat's `MAX_SPARKS` (slot 0 unused).
const SLOTS: usize = 558;
/// `SPARK_SURFACECOEF`
const SURFACE_COEF: f32 = 0.7;
/// `EXPLOSION_ANIMS`
const EXPLOSION_ANIMS: f32 = 16.0;

/// Styles that don't move by themselves (`NONEULER_STYLE`).
const NON_EULER: &[u8] = &[
    12, 13, 14, 15, 17, 24, 25, 28, 29, 31, 36, 37, 50, 54, 56, 60,
];
/// Styles that bounce off the map (`COLLIDABLE_STYLE`).
const COLLIDABLE: &[u8] = &[
    2, 4, 5, 6, 7, 8, 9, 10, 11, 13, 16, 18, 19, 20, 21, 22, 23, 30, 32, 33, 34, 40, 41, 42, 43,
    48, 49, 51, 52, 57, 62, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
];

#[derive(Debug, Clone, Default)]
pub struct Spark {
    pub active: bool,
    pub style: u8,
    pub owner: Option<SoldierId>,
    pub life: u8,
    pub life_prev: u8,
    pub collide_count: u8,
    pub particle: Particle,
}

pub struct Sparks {
    pub sparks: Vec<Spark>,
    /// `SparksCount`: active sparks at the last update.
    pub count: usize,
    rng: PascalRandom,
}

impl Default for Sparks {
    fn default() -> Sparks {
        Sparks {
            sparks: vec![Spark::default(); SLOTS],
            count: 0,
            rng: PascalRandom::new(0x5ba2),
        }
    }
}

/// What a spark needs from the game around it.
pub struct SparkCtx<'a> {
    pub world: &'a World,
    pub audio: &'a mut Audio,
    pub listener: &'a Listener,
    /// The soldier the camera follows (`CameraFollowSprite`), for what is worth showing.
    pub follow: Option<SoldierId>,
    /// `r_maxsparks`
    pub max_sparks: usize,
    /// The game width (sparks out of sight aren't created).
    pub game_width: f32,
}

impl Sparks {
    pub fn clear(&mut self) {
        *self = Sparks::default();
    }

    fn random(&mut self, n: i32) -> i32 {
        self.rng.below(n.max(1))
    }

    /// `CreateSpark`
    pub fn create(&mut self, spawn: &SparkSpawn, ctx: &SparkCtx) {
        let style = spawn.style;
        let owner = match spawn.owner {
            SparkOwner::Soldier(id) => Some(id),
            _ => None,
        };

        // only what can be seen (`PointVisible`)
        if let Some(me) = ctx.follow.and_then(|id| ctx.world.soldiers.get(id))
            && style != 38
        {
            let aim = vec2(me.control.mouse_aim_x as f32, me.control.mouse_aim_y as f32);
            let s = me.particle.pos - (me.particle.pos - aim) / 2.0;
            let d = (spawn.pos - s).abs();
            if d.x >= ctx.game_width || d.y >= 480.0 {
                return;
            }
        }

        let max = ctx.max_sparks;
        let count = self.count;
        let mut slot = 0;
        for i in 1..=max + 1 {
            if (count + 50 > max && matches!(style, 3 | 4 | 26 | 27 | 59 | 2))
                || (count + 40 > max && style == 1)
                || (count + 30 > max && style == 24)
            {
                return;
            }
            if i == max {
                slot = self.random((max / 3) as i32) as usize + 1;
                break;
            }
            if i < SLOTS && !self.sparks[i].active && self.sparks[i].style == 0 {
                slot = i;
                break;
            }
        }
        if slot == 0 || slot >= SLOTS {
            return;
        }

        tracing::trace!(style, slot, "spark");
        let gravity = ctx.world.config.gravity / 1.4;
        self.sparks[slot] = Spark {
            active: true,
            style,
            owner,
            life: spawn.life,
            life_prev: spawn.life,
            collide_count: 0,
            particle: Particle {
                active: true,
                pos: spawn.pos,
                old_pos: spawn.pos,
                velocity: spawn.velocity,
                force: Vec2::ZERO,
                one_over_mass: 1.0,
                timestep: 1.0,
                gravity,
                e_damping: 0.998,
                v_damping: 0.0,
            },
        };
    }

    /// `TSpark.Update` for every spark. Returns how much an explosion in view shakes the
    /// camera.
    pub fn update(&mut self, ctx: &mut SparkCtx) -> Vec2 {
        let mut shake = Vec2::ZERO;
        let mut born = Vec::new();
        let many = ctx.max_sparks + 10 > SLOTS;
        self.count = 0;

        for i in 1..SLOTS {
            if !self.sparks[i].active {
                continue;
            }
            self.count += 1;
            let style = self.sparks[i].style;

            if !NON_EULER.contains(&style) {
                self.sparks[i].particle.euler();
            }

            // out of bounds
            let map = &ctx.world.map;
            let bound = (map.sectors_num * map.sectors_division - 10) as f32;
            let p = self.sparks[i].particle.pos;
            if p.x.abs() > bound || p.y.abs() > bound {
                self.kill(i);
                continue;
            }

            if COLLIDABLE.contains(&style) {
                self.check_map_collision(i, &mut born, ctx);
                if !self.sparks[i].active {
                    continue;
                }
            }

            let spark = &self.sparks[i];
            let (life, pos, velocity, owner) = (
                spark.life,
                spark.particle.pos,
                spark.particle.velocity,
                spark.owner,
            );

            // the screen shakes with an explosion close by
            if matches!(style, 17 | 12 | 14 | 15 | 28)
                && f32::from(life) > EXPLOSION_ANIMS * 2.3
                && ctx.follow.is_some()
            {
                let wobble = i32::from(life) / 6;
                let wx = self.random(2 * wobble + 1);
                let wy = self.random(2 * wobble);
                shake += vec2((wx - wobble) as f32, (wy - wobble) as f32);
            }

            let smoke = |born: &mut Vec<SparkSpawn>, life: i32| {
                let owner = owner.map_or(SparkOwner::None, SparkOwner::Soldier);
                born.push(SparkSpawn::new(pos, velocity, 31, owner, life));
            };
            // smoking shells
            if many && style > 64 && life > 235 && self.random(32) == 0 {
                smoke(&mut born, 40);
            }
            if many && style == 52 {
                if life > 235 && self.random(6) == 0 {
                    smoke(&mut born, 40);
                }
                if life > 85 && life < 235 && self.random(15) == 0 {
                    smoke(&mut born, 35);
                }
                if life < 85 && self.random(24) == 0 {
                    smoke(&mut born, 30);
                }
            }
            // sparks off a little fire
            if many && style == 2 && self.random(8) == 0 {
                let owner = owner.map_or(SparkOwner::None, SparkOwner::Soldier);
                born.push(SparkSpawn::new(pos, Vec2::ZERO, 26, owner, 35));
            }

            let spark = &mut self.sparks[i];
            spark.life_prev = spark.life;
            spark.life = spark.life.wrapping_sub(1);
            if spark.life == 0 {
                self.kill(i);
            }
        }

        for spawn in born {
            self.create(&spawn, ctx);
        }
        shake
    }

    /// `MakeRain`, `MakeSandStorm`, `MakeSnow`: weather falling around the camera.
    pub fn weather(&mut self, camera: Vec2, half: Vec2, tick: u64, ctx: &SparkCtx) {
        let (style, start, b, top, life) = match ctx.world.map.weather {
            1 => (38, camera.x - half.x - 128.0, vec2(0.0, 12.0), -128.0, 60),
            2 => (
                39,
                camera.x - half.x - 1.5 * 512.0,
                vec2(10.0, 7.0),
                -256.0,
                80,
            ),
            3 => (53, camera.x - half.x - 256.0, vec2(1.0, 2.0), 0.0, 80),
            _ => return,
        };
        let modder = if ctx.max_sparks + 10 < SLOTS { 34 } else { 17 };
        if !tick.is_multiple_of(modder) {
            return;
        }
        let mut x = start;
        for _ in 0..8 {
            x += 128.0 - 50.0 + self.random(90) as f32;
            let y = camera.y - half.y + top - 60.0 + self.random(150) as f32;
            let spawn = SparkSpawn::new(vec2(x, y), b, style, SparkOwner::None, life);
            self.create(&spawn, ctx);
        }
    }

    fn kill(&mut self, i: usize) {
        let spark = &mut self.sparks[i];
        spark.active = false;
        spark.style = 0;
        spark.particle.active = false;
    }

    /// `TSpark.CheckMapCollision`: bounce off polygons (only sparks with an owner).
    fn check_map_collision(&mut self, i: usize, born: &mut Vec<SparkSpawn>, ctx: &mut SparkCtx) {
        let map = &ctx.world.map;
        let Some(owner) = self.sparks[i]
            .owner
            .and_then(|id| ctx.world.soldiers.get(id))
        else {
            return;
        };
        let p = self.sparks[i].particle.pos;
        let pos = vec2(p.x - 8.0, p.y - 1.0);
        let div = map.sectors_division as f32;
        let (kx, ky) = (
            (pos.x / div).round_ties_even() as i32,
            (pos.y / div).round_ties_even() as i32,
        );
        let n = map.sectors_num;
        if kx < -n || kx > n || ky < -n || ky > n {
            return;
        }

        for &w in map.sector(kx, ky) {
            let w = w as usize - 1;
            let polytype = map.polygons[w].polytype;
            if !team_collides(polytype, owner.team, false)
                || (polytype == PolyType::Bouncy && owner.holded_thing.is_none())
                || matches!(
                    polytype,
                    PolyType::OnlyBulletsCollide
                        | PolyType::OnlyPlayersCollide
                        | PolyType::NoCollide
                        | PolyType::Background
                        | PolyType::BackgroundTransition
                )
                || !map.point_in_poly_edges(pos.x, pos.y, w as i32)
            {
                continue;
            }

            let (mut d, mut b) = (0.0, 0);
            let mut perp =
                vec2normalize(map.closest_perpendicular(w as i32, pos, &mut d, &mut b)) * d;
            let spark = &mut self.sparks[i];
            spark.particle.velocity = (spark.particle.velocity - perp) * SURFACE_COEF;
            let (style, count, owner_id) = (spark.style, spark.collide_count, spark.owner);
            let spark_pos = spark.particle.pos;
            let owner = owner_id.map_or(SparkOwner::None, SparkOwner::Soldier);
            let shell_hit = matches!(count, 0 | 2 | 4);

            match style {
                2 | 62 | 33 | 34 | 57 => {
                    perp *= if style == 57 { 0.75 } else { 2.5 };
                    perp.x = perp.x - 0.5 + self.random(11) as f32 / 10.0;
                    perp.y = -perp.y;
                    match style {
                        2 | 62 => {
                            if self.random(2) == 0 {
                                let style = if self.random(2) == 0 { 26 } else { 27 };
                                born.push(SparkSpawn::new(pos, perp, style, owner, 35));
                                ctx.audio.play_at(Sfx::Ts, spark_pos, ctx.listener);
                            }
                        }
                        33 | 34 => {
                            let style = if self.random(7) == 0 { 26 } else { 27 };
                            born.push(SparkSpawn::new(pos, perp, style, owner, 35));
                            if count > 4 {
                                self.kill(i);
                            }
                        }
                        _ => {
                            self.random(2);
                            born.push(SparkSpawn::new(pos, perp, 58, owner, 50));
                        }
                    }
                }
                4 | 5 => {
                    if style == 5 {
                        let v = self.sparks[i].particle.velocity;
                        born.push(SparkSpawn::new(spark_pos, v, 55, owner, 30));
                    }
                    if count > 1 {
                        self.kill(i);
                    }
                }
                6 | 9 | 10 | 11 | 18 | 19 | 20 | 23 => {
                    let clink = if style == 6 {
                        shell_hit
                    } else {
                        matches!(count, 0 | 4)
                    };
                    if clink {
                        ctx.audio.play_at(Sfx::Clipfall, spark_pos, ctx.listener);
                    }
                    if count > 4 {
                        self.kill(i);
                    }
                }
                7 | 21 | 22 | 16 | 30 | 52 | 65..=73 => {
                    if shell_hit {
                        let sfx = Sfx::Shell.offset(self.random(2) as u8);
                        ctx.audio.play_at(sfx, spark_pos, ctx.listener);
                    }
                    if count > 4 {
                        self.kill(i);
                    }
                }
                51 => {
                    ctx.audio.play_at(Sfx::Gaugeshell, spark_pos, ctx.listener);
                    if count > 4 {
                        self.kill(i);
                    }
                }
                32 | 48 | 49 if count > 2 => self.kill(i),
                _ => {}
            }

            let spark = &mut self.sparks[i];
            spark.collide_count = spark.collide_count.wrapping_add(1);
            return;
        }
    }
}
