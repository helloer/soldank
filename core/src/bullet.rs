use super::*;
use slotmap::SlotMap;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum BulletStyle {
    #[default]
    Bullet = 1,
    FragGrenade = 2,
    GaugeBullet = 3,
    M79Grenade = 4,
    Flame = 5,
    Fist = 6,
    Arrow = 7,
    FlameArrow = 8,
    ClusterGrenade = 9,
    Cluster = 10,
    Blade = 11, // used for knife and chainsaw
    LAWMissile = 12,
    ThrownKnife = 13,
    M2Bullet = 14,
}

impl BulletStyle {
    /// From Soldat's `BULLET_STYLE_*` number.
    pub fn from_num(n: u8) -> Option<BulletStyle> {
        use BulletStyle::*;
        [
            Bullet,
            FragGrenade,
            GaugeBullet,
            M79Grenade,
            Flame,
            Fist,
            Arrow,
            FlameArrow,
            ClusterGrenade,
            Cluster,
            Blade,
            LAWMissile,
            ThrownKnife,
            M2Bullet,
        ]
        .into_iter()
        .find(|style| *style as u8 == n)
    }
}

/// Number of bullet slots (`MAX_BULLETS`). Bullets live in fixed slots like in Soldat: a new
/// bullet takes the first free slot and slots are updated in order, which decides whether a
/// bullet created during the bullet update (cluster grenade) moves in the same tick.
pub const MAX_BULLETS: usize = 254;

const ARROW_RESIST: i16 = 280;
const PART_RADIUS: f32 = 7.0;
/// Skeleton points checked for hits, in priority order: head, chest, hip, legs.
const BODY_PARTS_PRIORITY: [usize; 7] = [12, 11, 10, 6, 5, 4, 3];
const M79GRENADE_EXPLOSION_RADIUS: f32 = 64.0;
pub(crate) const FRAGGRENADE_EXPLOSION_RADIUS: f32 = 85.0;
const AFTER_EXPLOSION_RADIUS2: f32 = 50.0 * 50.0;
const CLUSTERGRENADE_EXPLOSION_RADIUS: f32 = 35.0;
// exactly representable, so FPC keeps them single precision
const EXPLOSION_IMPACT_MULTIPLY: f32 = 3.75;
/// `EXPLOSION_ANIMS`, `SMOKE_ANIMS`: frames of the explosion and smoke sparks.
const EXPLOSION_ANIMS: i32 = 16;
const SMOKE_ANIMS: i32 = 10;
const EXPLOSION_DEADIMPACT_MULTIPLY: f32 = 4.5;
const GRENADE_SURFACECOEF: f64 = 0.88;
const BULLET_GRAVITY_MULTIPLIER: f32 = 2.25;
const BULLET_E_DAMPING: f32 = 0.99;

#[derive(Debug, Copy, Clone)]
pub struct BulletParams {
    pub style: BulletStyle,
    pub weapon: WeaponKind,
    pub position: Vec2,
    pub velocity: Vec2,
    pub timeout: i16,
    pub hit_multiply: f32,
    pub team: Team,
    pub sprite: Option<sprites::Weapon>,
    /// Random seed of the bullet; `None` takes the owner's next `bullet_count`.
    pub seed: Option<u16>,
    /// `CreateBullet`'s MustCreate: otherwise flames start one step ahead.
    pub must_create: bool,
    /// `CreateBullet`'s Net: on the server, bots pay ammo only for these bullets.
    pub net: bool,
    /// The bullet can't hit its owner (`HitBody := Owner`, the mercy shot).
    pub owner_immune: bool,
}

/// What a bullet did when it hit something (`TBullet.Hit` types).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HitKind {
    Wall,
    Blood,
    /// Bullet pushed a thing (flag).
    Thing,
    /// Bullet passed through a body (pierce).
    BodyHit,
    Ricochet,
    /// M79 grenade, flame arrow or LAW hitting a wall.
    Explode,
    /// Frag grenade, M79, flame arrow and LAW timing out.
    FragGrenade,
    ClusterGrenade,
    Cluster,
    /// Stationary gun bullet timing out.
    Flak,
}

#[derive(Debug, Clone, Default)]
pub struct Bullet {
    pub active: bool,
    pub style: BulletStyle,
    pub weapon: WeaponKind,
    pub owner: SoldierId,
    pub team: Team,
    pub particle: Particle,
    pub initial_pos: Vec2,
    pub velocity_prev: Vec2,
    pub timeout: i16,
    pub timeout_prev: i16,
    pub hit_multiply: f32,
    pub hit_multiply_prev: f32,
    pub degrade_count: u8,
    pub ricochet_count: i32,
    pub hit_spot: Vec2,
    pub seed: u16,
    pub sprite: Option<sprites::Weapon>,
    /// Last soldier hit, who can't be hit again by this bullet.
    pub hit_body: Option<SoldierId>,
    /// Things this bullet pushed and until when it can't push them again
    /// (`ThingCollisions`).
    pub thing_collisions: Vec<(usize, u64)>,
    /// `HasHit`: it hurt a soldier already, which the weapon stats count once.
    pub has_hit: bool,
}

/// Things a bullet update asks the world to do.
#[derive(Debug, Clone)]
pub enum BulletOutcome {
    Hit {
        kind: HitKind,
        pos: Vec2,
    },
    /// New bullet created by this one (cluster grenade fragments).
    Spawn(BulletParams),
    /// A thrown knife stuck somewhere and becomes a knife thing (created by the world
    /// right after this bullet's update).
    KnifeThing(Vec2),
    /// It hurt another soldier who was alive until then (weapon stats).
    Hurt {
        victim: SoldierId,
    },
    Killed {
        victim: SoldierId,
        killer: SoldierId,
        kill: Kill,
        /// Weapon of the killing bullet.
        weapon: WeaponKind,
    },
}

/// World state a bullet update can touch. The updated bullet itself is taken out of
/// `bullets` (an inactive placeholder sits in its slot) so explosions can reach the others.
pub struct BulletCtx<'a> {
    pub config: &'a WorldConfig,
    pub things: &'a mut [Thing],
    pub tick: u64,
    pub soldiers: &'a mut SlotMap<SoldierId, Soldier>,
    pub bullets: &'a mut [Bullet],
    pub rng: &'a mut PascalRandom,
    pub outcomes: &'a mut Vec<BulletOutcome>,
    pub sounds: &'a mut Vec<SoundEvent>,
    pub sparks: &'a mut Vec<SparkSpawn>,
}

impl Bullet {
    /// `CreateBullet` + `BulletParts.CreatePart`.
    pub fn new(params: &BulletParams, owner: SoldierId, seed: u16, gravity: f32) -> Bullet {
        let mut position = params.position;

        // flames start one step ahead (`not MustCreate` in Soldat)
        if params.style == BulletStyle::Flame && !params.must_create {
            position += params.velocity;
        }

        let particle = Particle {
            active: true,
            pos: position,
            old_pos: position,
            velocity: params.velocity,
            one_over_mass: 1.0,
            timestep: 1.0,
            gravity: BULLET_GRAVITY_MULTIPLIER * gravity,
            e_damping: BULLET_E_DAMPING,
            ..Default::default()
        };

        Bullet {
            active: true,
            style: params.style,
            weapon: params.weapon,
            owner,
            team: params.team,
            particle,
            initial_pos: position,
            velocity_prev: params.velocity,
            timeout: params.timeout,
            timeout_prev: params.timeout,
            hit_multiply: params.hit_multiply,
            hit_multiply_prev: params.hit_multiply,
            degrade_count: 0,
            ricochet_count: 0,
            hit_spot: Vec2::ZERO,
            seed,
            sprite: params.sprite,
            hit_body: params.owner_immune.then_some(owner),
            thing_collisions: Vec::new(),
            has_hit: false,
        }
    }

    pub fn kill(&mut self) {
        self.active = false;
        self.particle.active = false;
        // a bullet killed by a wall is revived for the rest of its update: thing pushes
        // forget their cooldowns
        self.thing_collisions.clear();
    }

    /// Port of `TBullet.Update`. Integration (`BulletParts.DoEulerTimeStep`) happens
    /// afterwards for all bullets, see [`World::step`].
    pub fn update(&mut self, map: &MapFile, ctx: &mut BulletCtx) {
        self.timeout_prev = self.timeout;
        self.hit_multiply_prev = self.hit_multiply;
        self.velocity_prev = self.particle.velocity;

        let old_v = self.particle.velocity;
        let old_p = self.particle.pos;
        let old_op = self.particle.old_pos;

        self.check_out_of_bounds(map);

        // check collision with map
        let (x, y) = self.particle.pos.into();
        let hit = if self.style == BulletStyle::FragGrenade {
            self.check_map_collision(map, x, y - 2.0, ctx);
            let (x, y) = self.particle.pos.into();
            self.check_map_collision(map, x, y, ctx)
        } else {
            self.check_map_collision(map, x, y, ctx)
        };

        // Soldat uses (0, 0) for "no hit"
        let hit_p = hit.unwrap_or(Vec2::ZERO);
        let mut dist = -1.0;
        if !self.active {
            dist = (hit_p - old_op).length();
            self.particle.velocity = old_v;
            self.ricochet_count -= 1;
            self.particle.pos = old_p;
            self.particle.old_pos = old_op;
        }

        // check if hit collider
        let hit_p2 = self
            .check_collider_collision(map, dist, ctx)
            .unwrap_or(Vec2::ZERO);
        if !self.active {
            let a = if hit_p2.x == 0.0 {
                hit_p - old_op
            } else {
                hit_p2 - old_op
            };
            dist = a.length();
            self.particle.velocity = old_v;
            self.particle.pos = old_p;
            self.particle.old_pos = old_op;
        }

        // a bullet stopped by a wall can still hit a soldier in front of it
        let hit_p3 = self.check_sprite_collision(dist, ctx).unwrap_or(Vec2::ZERO);
        if !self.active {
            let a = if hit_p3.x != 0.0 {
                hit_p3
            } else if hit_p2.x != 0.0 {
                hit_p2
            } else {
                hit_p
            } - old_op;
            dist = a.length();
        }

        self.check_thing_collision(dist, ctx);

        // count time out
        self.timeout -= 1;
        if self.timeout == 0 {
            match self.style {
                BulletStyle::FragGrenade
                | BulletStyle::M79Grenade
                | BulletStyle::FlameArrow
                | BulletStyle::LAWMissile => self.hit(HitKind::FragGrenade, ctx),
                BulletStyle::Cluster => self.hit(HitKind::Cluster, ctx),
                BulletStyle::M2Bullet => self.hit(HitKind::Flak, ctx),
                _ => {}
            }
            self.kill();
        }

        // lose power on distance
        if self.timeout % 6 == 0
            && !matches!(
                self.weapon,
                WeaponKind::Barrett | WeaponKind::M79 | WeaponKind::Knife | WeaponKind::LAW
            )
        {
            let dist = (self.initial_pos - self.particle.pos).length();
            let threshold = match self.degrade_count {
                0 => Some(500.0),
                1 => Some(900.0),
                _ => None,
            };

            if threshold.is_some_and(|t| dist > t) {
                self.hit_multiply *= 0.5;
                self.degrade_count += 1;
            }
        }

        // flame rises
        if self.style == BulletStyle::Flame {
            self.particle.force.y = fpc(ext(self.particle.force.y) - 0.15);
        }

        // trails (Soldat gives them the bullet's number for an owner: none here)
        let pos = self.particle.pos;
        let none = SparkOwner::None;
        if self.style == BulletStyle::FlameArrow {
            if fx::random(2) == 0 {
                ctx.sparks
                    .push(SparkSpawn::new(pos, vec2(0.0, -0.5), 37, none, 40));
            }
            if fx::random(2) == 0 {
                ctx.sparks
                    .push(SparkSpawn::new(pos, vec2(0.0, -0.5), 36, none, 40));
            }
        }
        if self.style == BulletStyle::LAWMissile {
            ctx.sparks
                .push(SparkSpawn::new(pos, vec2(0.0, -1.5), 59, none, 50));
            if fx::random(2) == 0 {
                let velocity = self.particle.velocity;
                ctx.sparks.push(SparkSpawn::new(pos, velocity, 2, none, 5));
            }
        }
        // bleeding from the body it went through
        if self.hit_body.is_some() && fx::random(5) == 0 {
            let velocity = self.particle.velocity * 0.5;
            let owner = SparkOwner::Soldier(self.owner);
            ctx.sparks
                .push(SparkSpawn::new(pos, velocity, 4, owner, 90));
        }

        // its whoosh
        if self.timeout == BULLET_TIMEOUT as i16 - 25 && self.style != BulletStyle::GaugeBullet {
            ctx.sounds
                .push(SoundEvent::at(Sfx::Bulletby, self.particle.pos));
        }
    }

    fn check_out_of_bounds(&mut self, map: &MapFile) {
        let bound = (map.sectors_num * map.sectors_division - 10) as f32;
        let pos = self.particle.pos;

        if pos.x.abs() > bound || pos.y.abs() > bound {
            self.kill();
        }
    }

    fn collides(&self, map: &MapFile, poly: usize) -> bool {
        use PolyType::*;

        let polytype = map.polygons[poly].polytype;
        team_collides(polytype, self.team, true)
            && !matches!(
                polytype,
                OnlyPlayersCollide
                    | NoCollide
                    | OnlyFlaggers
                    | NotFlaggers
                    | Background
                    | BackgroundTransition
            )
    }

    /// Port of `TBullet.CheckMapCollision`; returns the hit position.
    fn check_map_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        ctx: &mut BulletCtx,
    ) -> Option<Vec2> {
        let velocity = self.particle.velocity;
        let largest = velocity.x.abs().max(velocity.y.abs());
        let det_acc = ((ext(largest) / 2.5) as i32).max(1);
        let step = velocity * (1.0 / det_acc as f32);
        let n = map.sectors_num;
        let div = map.sectors_division as f32;

        for b in 0..det_acc {
            let mut pos = vec2(x + b as f32 * step.x, y + b as f32 * step.y);
            let kx = (pos.x / div).round_ties_even() as i32;
            let ky = (pos.y / div).round_ties_even() as i32;

            if kx < -n || kx > n || ky < -n || ky > n {
                self.kill();
                return None;
            }

            for &w in map.sector(kx, ky) {
                let w = w as usize - 1;

                if !self.collides(map, w) || !map.point_in_poly_edges(pos.x, pos.y, w as i32) {
                    continue;
                }

                match self.style {
                    BulletStyle::Bullet
                    | BulletStyle::GaugeBullet
                    | BulletStyle::Fist
                    | BulletStyle::Blade
                    | BulletStyle::M2Bullet
                    | BulletStyle::M79Grenade
                    | BulletStyle::FlameArrow
                    | BulletStyle::LAWMissile => {
                        let explosive = matches!(
                            self.style,
                            BulletStyle::M79Grenade
                                | BulletStyle::FlameArrow
                                | BulletStyle::LAWMissile
                        );

                        self.particle.old_pos = self.particle.pos;
                        self.particle.pos = pos - self.particle.velocity;
                        let temp = self.particle.pos;
                        let temp2 = self.particle.velocity;

                        // ricochet!
                        if (self.particle.pos - self.hit_spot).length() > 50.0 {
                            self.ricochet_count += 1;
                            let (mut d, mut k) = (0.0, 0);
                            let perp = map.closest_perpendicular(
                                w as i32,
                                self.particle.pos,
                                &mut d,
                                &mut k,
                            );
                            let speed = self.particle.velocity.length();
                            let perp = vec2normalize(perp) * -speed;

                            let v = self.particle.velocity;
                            self.particle.velocity = vec2(
                                fpc(ext(v.x) * (25.0 / 35.0) + ext(perp.x) * (10.0 / 35.0)),
                                fpc(ext(v.y) * (25.0 / 35.0) + ext(perp.y) * (10.0 / 35.0)),
                            );
                            self.particle.pos = pos;
                            self.hit_spot = self.particle.pos;

                            let ahead = vec2normalize(self.particle.velocity) * (speed / 6.0);
                            if !explosive {
                                self.particle.old_pos = self.particle.pos;
                            }
                            pos = self.particle.pos + ahead;

                            let kx = (pos.x / div).round_ties_even() as i32;
                            let ky = (pos.y / div).round_ties_even() as i32;

                            if kx > -n && kx < n && ky > -n && ky < n {
                                for &w2 in map.sector(kx, ky) {
                                    let w2 = w2 as usize - 1;
                                    if self.collides(map, w2)
                                        && map.point_in_poly_edges(pos.x, pos.y, w2 as i32)
                                    {
                                        self.kill();
                                        break;
                                    }
                                }
                            }
                        } else {
                            self.kill();
                        }

                        // Hit() runs at the original wall contact, then the bounce state is restored
                        let bounced_velocity = self.particle.velocity;
                        self.particle.pos = temp;
                        self.particle.velocity = temp2;

                        let kind = match (self.active, explosive) {
                            (true, _) => HitKind::Ricochet,
                            (false, false) => HitKind::Wall,
                            (false, true) => HitKind::Explode,
                        };
                        self.hit(kind, ctx);

                        self.particle.pos = self.hit_spot;
                        self.particle.velocity = bounced_velocity;
                    }
                    BulletStyle::Arrow => {
                        self.particle.pos = pos - self.particle.velocity;
                        let gravity = self.particle.gravity;
                        self.particle.force.y -= gravity;

                        if self.timeout > ARROW_RESIST {
                            self.timeout = ARROW_RESIST;
                        }
                        if self.timeout < 20 {
                            self.particle.force.y += gravity;
                        }
                    }
                    BulletStyle::FragGrenade | BulletStyle::Flame => {
                        if self.style == BulletStyle::FragGrenade
                            && self.particle.velocity.length() > 1.5
                        {
                            ctx.sounds
                                .push(SoundEvent::at(Sfx::GrenadeBounce, self.particle.pos));
                        }
                        let (mut d, mut k) = (0.0, 0);
                        let perp =
                            map.closest_perpendicular(w as i32, self.particle.pos, &mut d, &mut k);
                        let perp = vec2normalize(perp) * d;
                        self.particle.pos = pos;
                        let v = self.particle.velocity - perp;
                        self.particle.velocity = vec2(
                            fpc(ext(v.x) * GRENADE_SURFACECOEF),
                            fpc(ext(v.y) * GRENADE_SURFACECOEF),
                        );

                        if self.style == BulletStyle::Flame && self.timeout > 16 {
                            self.timeout = 16;
                        }
                    }
                    BulletStyle::ClusterGrenade => {
                        self.hit(HitKind::ClusterGrenade, ctx);
                        self.kill();
                    }
                    BulletStyle::Cluster => {
                        self.hit(HitKind::Cluster, ctx);
                        self.kill();
                    }
                    BulletStyle::ThrownKnife => {
                        self.particle.pos = pos - self.particle.velocity;
                        ctx.outcomes
                            .push(BulletOutcome::KnifeThing(self.particle.pos));
                        self.hit(HitKind::Wall, ctx);
                        self.kill();
                    }
                }

                return Some(pos);
            }
        }

        None
    }

    /// Port of `TBullet.CheckColliderCollision`: map colliders stop bullets.
    fn check_collider_collision(
        &mut self,
        map: &MapFile,
        last_hit_dist: f32,
        ctx: &mut BulletCtx,
    ) -> Option<Vec2> {
        for collider in map.colliders.iter().filter(|c| c.active) {
            let start = self.particle.pos;
            let end = start + self.particle.velocity;
            let center = vec2(collider.x, collider.y);

            let Some(pos) =
                line_circle_collision(start, end, center, fpc(ext(collider.radius) / 1.7))
            else {
                continue;
            };

            // order collision
            if last_hit_dist > -1.0 && (pos - self.particle.old_pos).length() > last_hit_dist {
                break;
            }

            match self.style {
                BulletStyle::Bullet
                | BulletStyle::GaugeBullet
                | BulletStyle::Fist
                | BulletStyle::Blade
                | BulletStyle::ThrownKnife
                | BulletStyle::M2Bullet => {
                    self.particle.pos = pos - self.particle.velocity;
                    if self.style == BulletStyle::ThrownKnife {
                        ctx.outcomes
                            .push(BulletOutcome::KnifeThing(self.particle.pos));
                    }
                    // dirt
                    if many_sparks(ctx.config) {
                        for _ in 0..2 {
                            if fx::random(4) == 0 {
                                let a = vec2(
                                    (fx::random(100) as f32).sin(),
                                    (fx::random(100) as f32).cos(),
                                );
                                let style = 44 + fx::random(4) as u8;
                                ctx.sparks.push(SparkSpawn::new(
                                    pos,
                                    a,
                                    style,
                                    SparkOwner::None,
                                    120,
                                ));
                            }
                        }
                    }
                    ctx.sounds
                        .push(SoundEvent::at(Sfx::Colliderhit, self.particle.pos));
                    self.hit(HitKind::Wall, ctx);
                    self.kill();
                }
                BulletStyle::FragGrenade => {
                    if self.timeout < (GRENADE_TIMEOUT - 2) as i16 {
                        self.hit(HitKind::FragGrenade, ctx);
                        self.kill();
                    }
                }
                BulletStyle::Flame => self.kill(),
                BulletStyle::Arrow => {
                    if self.timeout > ARROW_RESIST {
                        let gravity = self.particle.gravity;
                        self.particle.force.y -= gravity;
                        self.hit(HitKind::Wall, ctx);
                        self.kill();
                    }
                }
                BulletStyle::M79Grenade | BulletStyle::FlameArrow | BulletStyle::LAWMissile => {
                    self.hit(HitKind::Explode, ctx);
                    self.kill();
                }
                BulletStyle::ClusterGrenade => {
                    self.hit(HitKind::ClusterGrenade, ctx);
                    self.kill();
                }
                BulletStyle::Cluster => {
                    self.hit(HitKind::Cluster, ctx);
                    self.kill();
                }
            }

            return Some(pos);
        }

        None
    }

    /// Port of `TBullet.CheckThingCollision`: bullets push flags (and other things that
    /// collide with bullets).
    fn check_thing_collision(&mut self, last_hit_dist: f32, ctx: &mut BulletCtx) {
        const FLAG_PART_RADIUS: f32 = 10.0;
        const THING_PUSH_MULTIPLIER: f32 = 9.0;
        const THING_COLLISION_COOLDOWN: u64 = 60;

        if self.style == BulletStyle::FragGrenade {
            return;
        }
        let infiltration = ctx.config.game_mode == GameMode::Infiltration;

        for (j, thing) in ctx.things.iter_mut().enumerate() {
            if !thing.active
                || self.timeout >= BULLET_TIMEOUT as i16 - 1
                || !thing.collide_with_bullets
                || thing.holding == Some(self.owner)
                || (infiltration && thing.kind != ThingKind::BravoFlag)
                || thing.kind == ThingKind::StationaryGun
            {
                continue;
            }

            let start = self.particle.pos;
            let end = start + self.particle.velocity;
            let Some((where_, pos)) = (1..=2).find_map(|i| {
                line_circle_collision(start, end, thing.skeleton.pos(i), FLAG_PART_RADIUS)
                    .map(|p| (i, p))
            }) else {
                continue;
            };

            // order collision
            if last_hit_dist > -1.0 && (pos - self.particle.old_pos).length() > last_hit_dist {
                break;
            }

            // push cooldown for this thing
            if self
                .thing_collisions
                .iter()
                .any(|&(num, end)| num == j && ctx.tick < end)
            {
                break;
            }
            self.thing_collisions
                .push((j, ctx.tick + THING_COLLISION_COOLDOWN));

            let thing_vel = thing.skeleton.pos(where_) - thing.skeleton.old_pos(where_);
            let push = ctx.config.weapons.get(self.weapon).push * THING_PUSH_MULTIPLIER;
            *thing.skeleton.pos_mut(where_) += (self.particle.velocity - thing_vel) * push;
            thing.static_type = false;

            if matches!(
                self.style,
                BulletStyle::Bullet | BulletStyle::FragGrenade | BulletStyle::GaugeBullet
            ) {
                self.hit(HitKind::Thing, ctx);
            }
            break;
        }
    }

    /// `TBullet.TargetableSprite`
    fn targetable(&self, id: SoldierId, soldier: &Soldier) -> bool {
        let owner_vulnerable_time = match self.style {
            BulletStyle::FragGrenade => GRENADE_TIMEOUT - 50,
            BulletStyle::M2Bullet => M2BULLET_TIMEOUT - 20,
            BulletStyle::Flame => FLAMER_TIMEOUT,
            _ => BULLET_TIMEOUT - 20,
        } as i16;

        soldier.active
            && (self.owner != id || self.timeout < owner_vulnerable_time)
            && self.hit_body != Some(id)
            && !soldier.is_spectator()
    }

    /// Port of `TBullet.CheckSpriteCollision`: hits soldiers in order of distance and
    /// returns the hit position.
    fn check_sprite_collision(&mut self, last_hit_dist: f32, ctx: &mut BulletCtx) -> Option<Vec2> {
        if (self.style == BulletStyle::Arrow && self.timeout <= ARROW_RESIST)
            || self.style == BulletStyle::ClusterGrenade
        {
            return None;
        }

        let mut bullet_velocity = self.particle.velocity;
        let melee = matches!(self.style, BulletStyle::Fist | BulletStyle::Blade);
        let weapon = ctx.config.weapons.get(self.weapon);

        // owner's hands, for melee reach
        let owner_hands = ctx
            .soldiers
            .get(self.owner)
            .map(|o| o.skeleton.pos(15) + o.hands_aim_direction() * 4.0);

        // FilterSpritesByDistance: insertion sort by squared distance, stable on ties
        let mut targets: Vec<(f32, SoldierId)> = Vec::new();
        for (id, soldier) in ctx.soldiers.iter() {
            if self.targetable(id, soldier) {
                let d = self.particle.pos - soldier.particle.pos;
                let rough = d.x * d.x + d.y * d.y;
                let at = targets
                    .iter()
                    .position(|&(dist, _)| rough < dist)
                    .unwrap_or(targets.len());
                targets.insert(at, (rough, id));
            }
        }

        let r = if self.style == BulletStyle::FragGrenade {
            PART_RADIUS + 1.0
        } else {
            PART_RADIUS
        };

        for &(_, id) in &targets {
            let target = &ctx.soldiers[id];
            let col = target.particle.pos;

            let (start, end) = match (melee, owner_hands) {
                (true, Some(hands)) => (hands, self.particle.pos + bullet_velocity),
                _ => (self.particle.pos, self.particle.pos + bullet_velocity),
            };

            // closest body part along the bullet's path
            let mut where_ = 0;
            let mut pos = Vec2::ZERO;
            let mut min_dist = f32::MAX;

            for part in BODY_PARTS_PRIORITY {
                let offset = target.skeleton.pos(part) - target.particle.pos;
                let mut col_pos = col + offset;
                // FIXME(skoskav): sprites are offset 2px to the left
                if !melee {
                    col_pos.x -= 2.0;
                }

                // Soldat keeps the closest part but the hit point of the last part hit
                // (LineCircleCollision's var parameter)
                if let Some(point) = line_circle_collision(start, end, col_pos, r) {
                    pos = point;
                    let dist = sqr_dist(start, point);
                    if dist < min_dist {
                        where_ = part;
                        min_dist = dist;
                    }
                }
            }

            if (melee && id == self.owner) || where_ == 0 {
                continue;
            }

            // order collision: the wall was hit first
            if last_hit_dist > -1.0 && (pos - self.particle.old_pos).length() > last_hit_dist {
                break;
            }

            let mut norm = (pos - target.skeleton.pos(where_)) * 1.3;
            norm.y = -norm.y;

            if let Some(brain) = ctx.soldiers[id].brain.as_mut() {
                brain.pissed_off = Some(self.owner);
            }
            let target = &ctx.soldiers[id];

            let owner_team = ctx.soldiers.get(self.owner).map(|o| o.team);
            if no_collision(
                weapon.no_collision,
                0,
                target.team,
                owner_team,
                id == self.owner,
            ) {
                continue;
            }

            if target.ceasefire_counter >= 0 {
                continue;
            }

            let target_dead = target.dead_meat;

            // knock-back
            if !target_dead
                && !matches!(
                    self.style,
                    BulletStyle::FragGrenade | BulletStyle::Flame | BulletStyle::Arrow
                )
            {
                ctx.soldiers[id].next_push += bullet_velocity * weapon.push;
            }

            let modifier = if where_ <= 4 {
                weapon.modifier_legs
            } else if where_ <= 11 {
                weapon.modifier_chest
            } else {
                weapon.modifier_head
            };

            let (owner, bullet_weapon) = (self.owner, self.weapon);
            let damage = |ctx: &mut BulletCtx, amount: f32| {
                let config = ctx.config;
                let hurt = health_hit(
                    ctx.soldiers,
                    id,
                    owner,
                    amount,
                    where_,
                    norm,
                    config,
                    Some(bullet_weapon),
                    ctx.rng,
                );
                outcomes_of_hurt(ctx, hurt, id, owner, bullet_weapon);
            };

            // the slap of a hit, before it hurts (client sounds)
            let hit_sound = |ctx: &mut BulletCtx, at: Vec2| {
                let target = &ctx.soldiers[id];
                let sound = if target.vest >= 1.0 {
                    Sound::new(Sfx::Vesthit)
                } else if target.dead_meat {
                    Sound::new(Sfx::DeadHit)
                } else {
                    Sound::new(Sfx::HitArg).variants(3)
                };
                ctx.sounds.push(sound.at(at).into());
            };

            // the client's blood, unless a teammate was hit without friendly fire
            let owner_team = ctx.soldiers.get(owner).map_or(Team::None, |s| s.team);
            let victim_team = ctx.soldiers[id].team;
            let bleeds = ctx.config.friendly_fire
                || owner_team == Team::None
                || owner_team != victim_team
                || id == owner;

            match self.style {
                BulletStyle::Bullet
                | BulletStyle::GaugeBullet
                | BulletStyle::Fist
                | BulletStyle::Blade
                | BulletStyle::M2Bullet => {
                    self.particle.pos = pos;
                    ctx.outcomes.push(BulletOutcome::Hit {
                        kind: HitKind::Blood,
                        pos,
                    });
                    if bleeds {
                        self.hit_sparks(HitKind::Blood, ctx);
                    }
                    self.puff_and_shreds(ctx, bullet_velocity, pos, id, where_);
                    hit_sound(ctx, pos);

                    let speed = bullet_velocity.length();
                    damage(ctx, speed * self.hit_multiply * modifier);
                    // HitSpray (deathmatch: every hit counts)
                    ctx.soldiers[id].hit_spray();

                    // drop weapon when punched (deathmatch: everyone is solo)
                    // TODO: team modes only for enemies
                    if self.style == BulletStyle::Fist
                        && !ctx.soldiers[id]
                            .primary_weapon()
                            .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
                    {
                        ctx.soldiers[id].body_apply_animation(Anim::ThrowWeapon, 11);
                    }

                    self.hit_body = Some(id);

                    // pierce: keep flying with less speed and check the next soldier
                    let pierce = if target_dead {
                        Some(0.9)
                    } else if ctx.soldiers[id].dead_meat || speed > 23.0 {
                        Some(0.75)
                    } else if speed > 5.0 && speed / weapon.speed >= 0.9 {
                        Some(0.66)
                    } else {
                        None
                    };

                    if let Some(factor) = pierce {
                        self.particle.velocity = bullet_velocity * factor;
                        bullet_velocity = self.particle.velocity;
                        ctx.outcomes.push(BulletOutcome::Hit {
                            kind: HitKind::BodyHit,
                            pos,
                        });
                        self.hit_sparks(HitKind::BodyHit, ctx);
                        continue;
                    }

                    self.kill();
                }
                BulletStyle::FragGrenade => {
                    if !target_dead {
                        self.hit_sprite(HitKind::FragGrenade, Some((id, where_)), ctx);
                        self.kill();
                    }
                }
                BulletStyle::Arrow => {
                    if self.timeout > ARROW_RESIST {
                        self.particle.pos = pos - self.particle.velocity;
                        let gravity = self.particle.gravity;
                        self.particle.force.y -= gravity;
                        ctx.outcomes.push(BulletOutcome::Hit {
                            kind: HitKind::Blood,
                            pos,
                        });
                        let victim = &ctx.soldiers[id];
                        let spared = (!ctx.config.friendly_fire
                            && owner_team != Team::None
                            && owner_team == victim_team
                            && !victim.local_player)
                            || victim.bonus_style == Bonus::Flamegod;
                        if !spared {
                            self.hit_sparks(HitKind::Blood, ctx);
                        }
                        hit_sound(ctx, self.particle.pos);

                        let speed = self.particle.velocity.length();
                        damage(ctx, speed * self.hit_multiply * modifier);
                        // Soldat moves the hit body part to `a`, which is zero on the server
                        if !ctx.soldiers[id].dead_meat {
                            *ctx.soldiers[id].skeleton.pos_mut(where_) = Vec2::ZERO;
                        }
                        self.kill();
                    }
                }
                BulletStyle::M79Grenade | BulletStyle::FlameArrow | BulletStyle::LAWMissile => {
                    if !target_dead {
                        self.hit_sprite(HitKind::Explode, Some((id, where_)), ctx);
                        self.particle.pos = pos;
                        self.kill();
                        let speed = self.particle.velocity.length();
                        damage(ctx, speed * self.hit_multiply);
                        if !ctx.soldiers[id].dead_meat {
                            *ctx.soldiers[id].skeleton.pos_mut(where_) = Vec2::ZERO;
                        }
                    }
                }
                BulletStyle::Flame => {
                    if self.owner != id {
                        // the flame sticks to the body
                        let target = &ctx.soldiers[id];
                        let part_pos = target.skeleton.pos(where_);
                        self.particle.pos = part_pos;
                        self.particle.velocity = if target.dead_meat {
                            Vec2::ZERO
                        } else {
                            target.particle.velocity
                        };

                        // server thresholds (clients use < 2 and < 1)
                        if self.timeout < 3 && self.ricochet_count < 2 {
                            let flamer = ctx.config.weapons.get(WeaponKind::Flamer);
                            if self.hit_multiply >= flamer.hit_multiply / 3.0 {
                                // keeps burning and spreads a weaker flame
                                self.timeout = FLAMER_TIMEOUT as i16 - 1;
                                self.ricochet_count += 1;
                                ctx.outcomes.push(BulletOutcome::Spawn(BulletParams {
                                    style: flamer.bullet_style,
                                    weapon: flamer.kind,
                                    position: part_pos,
                                    velocity: -target.particle.velocity,
                                    timeout: flamer.timeout as i16,
                                    hit_multiply: 2.0 * self.hit_multiply / 3.0,
                                    team: self.team,
                                    sprite: flamer.bullet_sprite,
                                    seed: None,
                                    must_create: false,
                                    net: false,
                                    owner_immune: false,
                                }));
                            }

                            if ctx.soldiers[id].health > -1.0 {
                                damage(ctx, self.hit_multiply);
                            }
                        }
                    }
                }
                BulletStyle::Cluster => {
                    self.hit_sprite(HitKind::Cluster, Some((id, where_)), ctx);
                    if !ctx.soldiers[id].dead_meat {
                        *ctx.soldiers[id].skeleton.pos_mut(where_) = Vec2::ZERO;
                    }
                    self.kill();
                }
                BulletStyle::ThrownKnife => {
                    ctx.outcomes.push(BulletOutcome::Hit {
                        kind: HitKind::Blood,
                        pos,
                    });
                    if bleeds {
                        self.hit_sparks(HitKind::Blood, ctx);
                    }
                    let velocity = self.particle.velocity;
                    self.puff_and_shreds(ctx, velocity, pos, id, where_);
                    // the client sounds once per soldier; a knife passing through corpses
                    // only sounds where it stops
                    if !target_dead || ctx.config.realistic_mode {
                        hit_sound(ctx, self.particle.pos);
                    }
                    let speed = self.particle.velocity.length();
                    damage(ctx, fpc(ext(speed * self.hit_multiply) * 0.01));
                    if !ctx.soldiers[id].dead_meat {
                        *ctx.soldiers[id].skeleton.pos_mut(where_) = Vec2::ZERO;
                    }
                    if !target_dead || ctx.config.realistic_mode {
                        ctx.outcomes
                            .push(BulletOutcome::KnifeThing(self.particle.pos));
                        self.kill();
                    }
                }
                BulletStyle::ClusterGrenade => {}
            }

            return Some(pos);
        }

        None
    }

    /// Port of `TBullet.Hit`: gameplay side only, effects are left to the client.
    fn hit(&mut self, kind: HitKind, ctx: &mut BulletCtx) {
        self.hit_sprite(kind, None, ctx);
    }

    /// `TBullet.Hit` with the soldier and body part that were hit directly.
    fn hit_sprite(
        &mut self,
        kind: HitKind,
        sprite_hit: Option<(SoldierId, usize)>,
        ctx: &mut BulletCtx,
    ) {
        ctx.outcomes.push(BulletOutcome::Hit {
            kind,
            pos: self.particle.pos,
        });

        let pos = self.particle.pos;
        let sound = match kind {
            HitKind::Wall if self.timeout < BULLET_TIMEOUT as i16 - 5 => {
                Some(Sound::new(Sfx::Ric).variants(4))
            }
            HitKind::Explode => Some(Sound::new(Sfx::M79Explosion)),
            HitKind::FragGrenade => Some(Sound::new(Sfx::GrenadeExplosion)),
            HitKind::Thing => Some(Sound::new(Sfx::Bodyfall)),
            HitKind::ClusterGrenade => Some(Sound::new(Sfx::Clustergrenade)),
            HitKind::Cluster => Some(Sound::new(Sfx::ClusterExplosion)),
            HitKind::Flak => Some(Sound::new(Sfx::M2explode)),
            HitKind::Ricochet => Some(Sound::new(Sfx::Ric5).variants(3)),
            _ => None,
        };
        if let Some(sound) = sound {
            ctx.sounds.push(sound.at(pos).into());
        }
        self.hit_sparks(kind, ctx);

        if matches!(
            kind,
            HitKind::Explode | HitKind::FragGrenade | HitKind::Cluster | HitKind::Flak
        ) {
            self.explosion_hit(kind, sprite_hit, ctx);
        }

        if kind == HitKind::ClusterGrenade {
            let origin = self.particle.pos - self.particle.velocity;
            let frag = ctx.config.weapons.get(WeaponKind::FragGrenade);
            let cluster = ctx.config.weapons.get(WeaponKind::Cluster);

            for _ in 0..5 {
                let mut b = self.particle.velocity * -0.75;
                b.x = fpc(ext(-b.x) - 2.5 + f64::from(ctx.rng.below(50)) / 10.0);
                b.y = fpc(ext(b.y) - 2.5 + f64::from(ctx.rng.below(25)) / 10.0);

                ctx.outcomes.push(BulletOutcome::Spawn(BulletParams {
                    style: BulletStyle::Cluster,
                    weapon: WeaponKind::Cluster,
                    position: origin,
                    velocity: b,
                    timeout: cluster.timeout as i16,
                    hit_multiply: frag.hit_multiply / 2.0,
                    team: self.team,
                    sprite: cluster.bullet_sprite,
                    seed: None,
                    must_create: false,
                    net: true,
                    owner_immune: false,
                }));
            }
        }
    }
}

impl Bullet {
    /// A puff where the bullet went in, and now and then a shred of the clothes there
    /// (client sparks of `CheckSpriteCollision`).
    fn puff_and_shreds(
        &self,
        ctx: &mut BulletCtx,
        velocity: Vec2,
        hit: Vec2,
        victim: SoldierId,
        where_: usize,
    ) {
        if !many_sparks(ctx.config) {
            return;
        }
        let owner = SparkOwner::Soldier(victim);
        // (Soldat gives the puff its position for a velocity)
        let a = self.particle.pos + vec2normalize(velocity) * 3.0;
        ctx.sparks.push(SparkSpawn::new(a, a, 50, owner, 31));

        let style = match where_ {
            0..=4 => 49,
            5..=11 => 48,
            _ => return,
        };
        for _ in 0..2 {
            if fx::random(8) == 0 {
                let a = vec2(
                    (fx::random(100) as f32).sin(),
                    (fx::random(100) as f32).cos(),
                );
                ctx.sparks.push(SparkSpawn::new(hit, a, style, owner, 120));
            }
        }
    }

    /// The sparks of `TBullet.Hit` (client side).
    fn hit_sparks(&self, kind: HitKind, ctx: &mut BulletCtx) {
        let owner = SparkOwner::Soldier(self.owner);
        let pos = self.particle.pos;
        let velocity = self.particle.velocity;
        let r = |n: i32| fx::random(n) as f32;
        let mut spark = |pos: Vec2, b: Vec2, style: u8, life: i32| {
            ctx.sparks.push(SparkSpawn::new(pos, b, style, owner, life));
        };
        let config = ctx.config;

        match kind {
            HitKind::Wall => {
                let a = pos + velocity;
                let mut b = velocity * -0.06;
                b.y -= 1.0;
                b.x *= 0.6 + r(8) / 10.0;
                b.y *= 0.8 + r(4) / 10.0;
                spark(a, b, 3, 60);
                b.x *= 0.8 + r(4) / 10.0;
                b.y *= 0.6 + r(8) / 10.0;
                spark(a, b, 3, 65);
                b *= 0.4 + r(4) / 10.0;
                spark(a, b, 1, 60);
                b.x *= 0.5 + r(4) / 10.0;
                b.y *= 0.7 + r(8) / 10.0;
                spark(a, b, 3, 50);
                if config.max_sparks > SPARK_SLOTS - 5 {
                    spark(a, Vec2::ZERO, 56, 22);
                }
            }
            HitKind::Blood => {
                let mut b = velocity * 0.025;
                let steps: [(f32, f32, u8, i32, bool); 6] = [
                    (1.2, 0.85, 4, 70, false),
                    (0.745, 1.1, 4, 75, false),
                    (0.9, 0.85, 4, 75, true),
                    (1.2, 0.85, 5, 80, false),
                    (1.0, 1.0, 5, 85, false),
                    (0.5, 1.05, 5, 75, true),
                ];
                for (sx, sy, style, life, maybe) in steps {
                    b.x *= sx;
                    b.y *= sy;
                    if !maybe || fx::random(2) == 0 {
                        spark(pos, b, style, life);
                    }
                }
                for _ in 0..7 {
                    if fx::random(6) == 0 {
                        let b = vec2(r(100).sin(), r(100).cos()) * 1.6;
                        spark(pos, b, 4, 55);
                    }
                }
            }
            HitKind::Explode | HitKind::FragGrenade => {
                let (smoke_life, style) = if kind == HitKind::Explode {
                    (255, 12)
                } else {
                    (190, 17)
                };
                if many_sparks(config) {
                    spark(pos, Vec2::ZERO, 60, smoke_life);
                    spark(pos, Vec2::ZERO, 54, SMOKE_ANIMS * 4 + 10);
                }
                spark(pos, Vec2::ZERO, style, EXPLOSION_ANIMS * 3);
            }
            HitKind::Thing => {
                let a = pos + velocity;
                let b = velocity * -0.02 * (0.4 + r(4) / 10.0);
                spark(a, b, 1, 70);
            }
            HitKind::ClusterGrenade | HitKind::Flak => spark(pos, Vec2::ZERO, 29, 55),
            HitKind::Cluster => spark(pos, Vec2::ZERO, 28, EXPLOSION_ANIMS * 3),
            HitKind::BodyHit => {
                if few_sparks(config) {
                    return;
                }
                let mut b = velocity * 0.075;
                let steps: [(f32, f32, u8, i32); 4] = [
                    (1.2, 0.85, 4, 60),
                    (0.745, 1.1, 4, 65),
                    (1.5, 0.4, 5, 70),
                    (1.0, 1.0, 5, 75),
                ];
                for (sx, sy, style, life) in steps {
                    b.x *= sx;
                    b.y *= sy;
                    spark(pos, b, style, life);
                }
                for _ in 0..4 {
                    if fx::random(6) == 0 {
                        let b = vec2(r(100).sin(), r(100).cos()) * 1.2;
                        spark(pos, b, 4, 50);
                    }
                }
            }
            HitKind::Ricochet => {
                let a = pos + velocity;
                for (spread, style) in [
                    (2.0, 26),
                    (2.0, 26),
                    (3.0, 26),
                    (3.0, 26),
                    (3.0, 26),
                    (3.0, 27),
                ] {
                    let n = (spread * 20.0) as i32;
                    let b = vec2(-spread + r(n) / 10.0, -spread + r(n) / 10.0);
                    spark(a, b, style, 35);
                }
            }
        }
    }

    /// Port of `TBullet.ExplosionHit`: radius damage and knock-back, corpse pushing and
    /// chain explosions of nearby grenades/rockets.
    fn explosion_hit(
        &mut self,
        kind: HitKind,
        sprite_hit: Option<(SoldierId, usize)>,
        ctx: &mut BulletCtx,
    ) {
        let (gun, radius) = match kind {
            HitKind::FragGrenade => (WeaponKind::FragGrenade, FRAGGRENADE_EXPLOSION_RADIUS),
            HitKind::Explode => (WeaponKind::M79, M79GRENADE_EXPLOSION_RADIUS),
            HitKind::Cluster | HitKind::Flak => {
                (WeaponKind::FragGrenade, CLUSTERGRENADE_EXPLOSION_RADIUS)
            }
            _ => return,
        };
        let gun = ctx.config.weapons.get(gun);
        let radius2 = radius * radius;
        let pos = self.particle.pos;
        let owner = self.owner;
        let len2 = |v: Vec2| v.x * v.x + v.y * v.y;

        let config = ctx.config;
        let bullet_weapon = self.weapon;
        let damage = |ctx: &mut BulletCtx, id: SoldierId, amount: f32, impact: Vec2| {
            let weapon = Some(bullet_weapon);
            let hurt = health_hit(
                ctx.soldiers,
                id,
                owner,
                amount,
                1,
                impact,
                config,
                weapon,
                ctx.rng,
            );
            outcomes_of_hurt(ctx, hurt, id, owner, bullet_weapon);
        };

        let flags = ctx.config.weapons.get(self.weapon).no_collision;
        let owner_team = ctx.soldiers.get(owner).map(|o| o.team);
        let ids: Vec<SoldierId> = ctx.soldiers.keys().collect();
        for id in ids {
            let soldier = &mut ctx.soldiers[id];
            if !soldier.active
                || soldier.is_spectator()
                || no_collision(flags, 3, soldier.team, owner_team, id == owner)
            {
                continue;
            }

            if !soldier.dead_meat {
                // GetSpriteCollisionPoint is the soldier's position on the server
                let col = soldier.particle.pos;
                let part_pos =
                    |part: usize| col + (soldier.skeleton.pos(part) - soldier.particle.pos);

                // if the hit point is not given find the closest one
                let mut w = 0;
                match sprite_hit {
                    Some((hit, where_)) if hit == id && where_ != 0 => w = where_,
                    _ => {
                        let mut s = f32::MAX;
                        for part in BODY_PARTS_PRIORITY {
                            let s2 = len2(pos - part_pos(part));
                            if s2 < s {
                                s = s2;
                                w = part;
                            }
                        }
                    }
                }

                let mut modifier = if w <= 4 {
                    gun.modifier_legs
                } else if w <= 11 {
                    gun.modifier_chest
                } else {
                    gun.modifier_head
                };

                let mut a = pos - part_pos(w);
                let s = len2(a);

                if s < radius2 {
                    let s = s.sqrt();
                    let at = soldier.particle.pos;
                    ctx.sounds.push(SoundEvent::at(Sfx::ExplosionErg, at));
                    let owner = SparkOwner::Soldier(self.owner);
                    ctx.sparks
                        .push(SparkSpawn::new(at, vec2(0.0, -0.01), 5, owner, 80));

                    // collision respond
                    a.x = a.x * (1.0 / (s + 1.0)) * EXPLOSION_IMPACT_MULTIPLY;
                    a.y = a.y * (1.0 / (s + 1.0)) * EXPLOSION_IMPACT_MULTIPLY;

                    if matches!(kind, HitKind::FragGrenade | HitKind::Explode) {
                        a.y *= 2.0;
                    } else {
                        // cluster/flak is halved
                        modifier *= 0.5;
                    }

                    soldier.next_push -= a;

                    if soldier.ceasefire_counter < 0 {
                        let amount = (1.0 / (s + 1.0)) * gun.hit_multiply * modifier;
                        damage(ctx, id, amount, a);
                    }

                    ctx.soldiers[id].hit_spray();
                }
            }

            let soldier = &mut ctx.soldiers[id];
            if soldier.dead_meat {
                let mut a = Vec2::ZERO;
                let mut s2 = None;

                for part in 1..=16 {
                    a = pos - soldier.skeleton.pos(part);
                    let s = len2(a);

                    if s < radius2 {
                        let s = s.sqrt();
                        a *= (1.0 / (s + 1.0)) * EXPLOSION_DEADIMPACT_MULTIPLY;
                        *soldier.skeleton.old_pos_mut(part) += a;
                        s2 = Some(s);
                    }
                }

                if let Some(mut s2) = s2 {
                    let mut modifier = 1.0;
                    match kind {
                        // Max(s2, 20.0000001) in extended, stored back into a Single
                        HitKind::Explode => s2 = s2.max(20.0),
                        HitKind::Cluster | HitKind::Flak => modifier = 0.5,
                        _ => {}
                    }

                    let amount = (1.0 / (s2 + 1.0)) * gun.hit_multiply * modifier;
                    damage(ctx, id, amount, a);
                }
            }
        }

        // push things
        for thing in ctx
            .things
            .iter_mut()
            .filter(|t| t.active && t.collide_with_bullets)
        {
            for j in 1..=thing.skeleton.particles().len().min(4) {
                let mut a = pos - thing.skeleton.pos(j);
                let s = len2(a);
                if s < radius2 {
                    let s = s.sqrt();
                    a *= 0.5 * (1.0 / (s + 1.0)) * EXPLOSION_IMPACT_MULTIPLY;
                    *thing.skeleton.old_pos_mut(j) += a;
                    thing.static_type = false;
                }
            }
        }

        // Soldat's `if not Typ in [...]` never exits, so every explosion sets off nearby
        // grenades and rockets
        self.active = false;
        for i in 0..ctx.bullets.len() {
            let other = &ctx.bullets[i];
            let kind = match other.style {
                BulletStyle::FragGrenade => HitKind::FragGrenade,
                BulletStyle::M79Grenade | BulletStyle::LAWMissile => HitKind::Explode,
                _ => continue,
            };

            if other.active && len2(pos - other.particle.pos) < AFTER_EXPLOSION_RADIUS2 {
                let mut other = std::mem::take(&mut ctx.bullets[i]);
                other.hit(kind, ctx);
                other.kill();
                ctx.bullets[i] = other;
            }
        }

        self.explosion_debris(kind, ctx);
    }

    /// Dirt, sparks and flames flying from an explosion (client sparks).
    fn explosion_debris(&self, kind: HitKind, ctx: &mut BulletCtx) {
        if !many_sparks(ctx.config) {
            return;
        }
        let frag = kind == HitKind::FragGrenade;
        let velocity = self.particle.velocity;
        let mut a = self.particle.pos - velocity;
        let owner = SparkOwner::Soldier(self.owner);
        let r = |n: i32| fx::random(n) as f32;
        let fly = |scale: f32| {
            let b = velocity * scale;
            vec2(-b.x - 3.5 + r(70) / 10.0, b.y - 3.5 + r(65) / 10.0)
        };
        let mut spark = |a: Vec2, b: Vec2, style: u8, life: i32| {
            ctx.sparks.push(SparkSpawn::new(a, b, style, owner, life));
        };

        // dirt
        let (n, s) = if frag { (6, -0.2) } else { (7, -0.15) };
        for _ in 0..n {
            let b = fly(s);
            for style in 40..=43 {
                if fx::random(4) == 0 {
                    spark(a, b, style, 180 + fx::random(50));
                }
            }
        }
        // smaller dirt
        let (n, rnd) = if frag { (7, 4) } else { (5, 3) };
        for _ in 0..n {
            let b = fly(s);
            for style in 44..=47 {
                if fx::random(rnd) == 0 {
                    spark(a, b, style, 120);
                }
            }
        }
        // sparks
        let (n, rnd) = if frag { (3, 23) } else { (4, 22) };
        for _ in 0..n {
            let b = fly(-0.3);
            for _ in 0..3 {
                if fx::random(rnd) == 0 {
                    spark(a, b, 2, 120);
                }
            }
        }
        // little flames
        let (n, j, rnd, s) = if frag {
            (3, 25.0, 50, -0.05)
        } else {
            (4, 20.0, 40, -0.1)
        };
        for _ in 0..n {
            a.x = a.x - j + r(rnd);
            a.y = a.y - j + r(rnd);
            let b = fly(s);
            spark(a, b, 64, 35);
        }
    }
}

/// What the world hears of a bullet's hit on soldier `id`.
fn outcomes_of_hurt(
    ctx: &mut BulletCtx,
    hurt: Option<Hurt>,
    id: SoldierId,
    owner: SoldierId,
    weapon: WeaponKind,
) {
    let Some(hurt) = hurt else { return };
    if hurt.was_alive && id != owner {
        ctx.outcomes.push(BulletOutcome::Hurt { victim: id });
    }
    if let Some(kill) = hurt.kill {
        ctx.outcomes.push(BulletOutcome::Killed {
            victim: id,
            killer: owner,
            kill,
            weapon,
        });
    }
}

/// weapons.ini `NoCollision` flags (`WEAPON_NOCOLLISION_*`): bits 0-2 for bullets
/// (`shift` 0), 3-5 for explosions (`shift` 3): enemy, team, self. Teams compare like
/// `IsInSameTeam`, so in deathmatch everyone is in the same team.
fn no_collision(
    flags: u8,
    shift: u8,
    team: Team,
    owner_team: Option<Team>,
    is_owner: bool,
) -> bool {
    let bits = flags >> shift;
    let same_team = owner_team == Some(team);
    (bits & 1 != 0 && !same_team)
        || (bits & 2 != 0 && same_team && !is_owner)
        || (bits & 4 != 0 && is_owner)
}

/// Port of `TeamCollides` (including its Bravo/yellow typo, faithfully).
pub fn team_collides(polytype: PolyType, team: Team, bullet: bool) -> bool {
    use PolyType::*;

    if bullet {
        match polytype {
            AlphaBullets | AlphaPlayers => team == Team::Alpha && polytype == AlphaBullets,
            // Soldat compares Bravo against the yellow (Charlie) bullet type, so bravo
            // bullets never pass through blue polygons
            BravoBullets | BravoPlayers => false,
            CharlieBullets | CharliePlayers => team == Team::Charlie && polytype == CharlieBullets,
            DeltaBullets | DeltaPlayers => team == Team::Delta && polytype == DeltaBullets,
            NonFlaggersCollide => false,
            _ => true,
        }
    } else {
        match polytype {
            AlphaBullets | AlphaPlayers => team == Team::Alpha && polytype == AlphaPlayers,
            BravoBullets | BravoPlayers => team == Team::Bravo && polytype == BravoPlayers,
            CharlieBullets | CharliePlayers => team == Team::Charlie && polytype == CharliePlayers,
            DeltaBullets | DeltaPlayers => team == Team::Delta && polytype == DeltaPlayers,
            NonFlaggersCollide => false,
            _ => true,
        }
    }
}

/// Whether a soldier collides with a polygon (`TSprite.CheckMapCollision` and
/// `CheckMapVerticesCollision` conditions).
pub fn soldier_collides(polytype: PolyType, team: Team, holding_thing: bool) -> bool {
    use PolyType::*;

    (!matches!(
        polytype,
        NoCollide | OnlyBulletsCollide | OnlyFlaggers | NotFlaggers
    ) && team_collides(polytype, team, false))
        || (holding_thing && polytype == OnlyFlaggers)
        || (!holding_thing && polytype == NotFlaggers)
}

/// The variant used by `TSprite.CheckRadiusMapCollision`.
pub fn soldier_radius_collides(polytype: PolyType, team: Team, holding_thing: bool) -> bool {
    use PolyType::*;

    let mut teamcol = team_collides(polytype, team, false);
    if (!holding_thing && polytype == OnlyFlaggers) || (holding_thing && polytype == NotFlaggers) {
        teamcol = false;
    }

    teamcol && !matches!(polytype, NoCollide | OnlyBulletsCollide)
}

/// `LineCircleCollision` (Calc.pas): where segment start-end first enters the circle.
pub fn line_circle_collision(start: Vec2, end: Vec2, center: Vec2, radius: f32) -> Option<Vec2> {
    let r2 = radius * radius;

    if sqr_dist(start, center) <= r2 {
        return Some(start);
    }
    if sqr_dist(end, center) <= r2 {
        return Some(end);
    }

    let points = line_circle_intersections(start, end, center, radius);
    match points.as_slice() {
        [] => None,
        [p] => Some(*p),
        [p0, p1, ..] => Some(if sqr_dist(*p0, start) > sqr_dist(*p1, start) {
            *p1
        } else {
            *p0
        }),
    }
}

/// `SqrDist` (Calc.pas)
fn sqr_dist(a: Vec2, b: Vec2) -> f32 {
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)
}

/// `IsLineIntersectingCircle` (Calc.pas), single precision like the original.
fn line_circle_intersections(
    mut line1: Vec2,
    mut line2: Vec2,
    mut center: Vec2,
    radius: f32,
) -> Vec<Vec2> {
    let mut result = Vec::new();
    let mut diffx = line2.x - line1.x;
    let mut diffy = line2.y - line1.y;

    if diffx.abs() < 0.00001 && diffy.abs() < 0.00001 {
        return result;
    }

    // flip the coordinate system for steep lines
    let flipped = diffy.abs() > diffx.abs();
    if flipped {
        line1 = vec2(line1.y, line1.x);
        line2 = vec2(line2.y, line2.x);
        center = vec2(center.y, center.x);
        std::mem::swap(&mut diffx, &mut diffy);
    }

    let a = diffy / diffx;
    let b = line1.y - a * line1.x;
    let a1 = a * a + 1.0;
    let b1 = 2.0 * (a * b - a * center.y - center.x);
    let c1 =
        center.y * center.y - radius * radius + center.x * center.x - 2.0 * b * center.y + b * b;
    let delta = b1 * b1 - 4.0 * a1 * c1;

    if delta < 0.0 {
        return result;
    }

    let (minx, maxx) = (line1.x.min(line2.x), line1.x.max(line2.x));
    let (miny, maxy) = (line1.y.min(line2.y), line1.y.max(line2.y));
    let sqrt_delta = delta.sqrt();
    let a2 = 2.0 * a1;

    for x in [(-b1 - sqrt_delta) / a2, (-b1 + sqrt_delta) / a2] {
        let y = a * x + b;
        if (minx..=maxx).contains(&x) && (miny..=maxy).contains(&y) {
            result.push(if flipped { vec2(y, x) } else { vec2(x, y) });
        }
    }

    result
}
