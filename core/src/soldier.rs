use super::*;
use std::sync::Arc;

const SLIDELIMIT: f64 = 0.2;
const SURFACECOEFX: f64 = 0.970;
const SURFACECOEFY: f64 = 0.970;
const CROUCHMOVESURFACECOEFX: f64 = 0.85;
const CROUCHMOVESURFACECOEFY: f64 = 0.97;
const STANDSURFACECOEFX: f64 = 0.00;
const STANDSURFACECOEFY: f64 = 0.00;

pub(crate) const POS_STAND: u8 = 1;
pub(crate) const POS_CROUCH: u8 = 2;
pub(crate) const POS_PRONE: u8 = 3;

const MAX_INACCURACY: f32 = 0.5;

const MAX_VELOCITY: f32 = 11.0;
const SOLDIER_COL_RADIUS: f32 = 3.0;

#[allow(dead_code)]
pub struct Soldier {
    pub active: bool,
    pub dead_meat: bool,
    pub style: u8,
    pub num: usize,
    pub visible: u8,
    pub on_ground: bool,
    pub on_ground_for_law: bool,
    pub on_ground_last_frame: bool,
    pub on_ground_permanent: bool,
    pub direction: i8,
    pub old_direction: i8,
    pub health: f32,
    pub alpha: u8,
    pub jets_count: i32,
    pub jets_count_prev: i32,
    pub wear_helmet: u8,
    pub has_cigar: u8,
    pub vest: f32,
    pub idle_time: i32,
    pub idle_random: i8,
    pub position: u8,
    pub on_fire: u8,
    pub collider_distance: u8,
    pub half_dead: bool,
    pub skeleton: ParticleSystem,
    pub legs_animation: AnimState,
    pub body_animation: AnimState,
    pub control: Control,
    pub active_weapon: usize,
    pub weapons: [Weapon; 3],
    pub fired: u8,
    pub particle: Particle,
    pub anims: Arc<Animations>,
    pub burst_count: u8,
    pub can_auto_reload_spas: bool,
    /// Seeds the Desert Eagle / shotgun spread so it can be replayed (`TSprite.BulletCount`).
    pub bullet_count: u16,
    /// Spawn protection ticks left, -1 when inactive.
    pub ceasefire_counter: i32,
    pub team: Team,
    /// Knock-back from hits, applied at the start of the next update (server semantics).
    pub next_push: Vec2,
    pub respawn_counter: i32,
    pub dead_time: i32,
    pub dead_collide_count: i32,
    pub kills: i32,
    pub deaths: i32,
    /// Weapons given on respawn (primary, secondary, grenades).
    pub loadout: [WeaponKind; 3],
    /// The grenade key was released since the last throw.
    pub grenade_can_throw: bool,
    /// Thing slot of the flag or parachute held (`HoldedThing`).
    pub holded_thing: Option<usize>,
    /// Picked up a medikit recently (`sv_healthcooldown`).
    pub has_pack: bool,
    /// Weapon let go of this tick, turned into a thing by the world.
    pub pending_drop: Option<WeaponDrop>,
    /// Died this tick: the world clears the ownership of its things.
    pub release_things: bool,
}

/// What polygon effects need besides the map (`HandleSpecialPolyTypes`).
pub struct PolyEnv<'a> {
    pub config: &'a WorldConfig,
    pub tick: u64,
    pub rng: &'a mut PascalRandom,
    pub emitter: &'a mut Vec<EmitterItem>,
}

impl Soldier {
    pub fn primary_weapon(&self) -> &Weapon {
        &self.weapons[self.active_weapon]
    }

    pub fn secondary_weapon(&self) -> &Weapon {
        &self.weapons[(self.active_weapon + 1) % 2]
    }

    pub fn tertiary_weapon(&self) -> &Weapon {
        &self.weapons[2]
    }

    pub fn switch_weapon(&mut self) {
        let w = (self.active_weapon + 1) % 2;
        self.active_weapon = w;
        self.weapons[w].start_up_time_count = self.weapons[w].start_up_time;
        self.weapons[w].reload_time_prev = self.weapons[w].reload_time_count;
        // burst_count = 0;
    }

    pub fn new(spawn: &MapSpawnpoint, data: &GameData, weapons: &WeaponTable) -> Soldier {
        let particle = Particle {
            active: true,
            pos: vec2(spawn.x as f32, spawn.y as f32),
            old_pos: vec2(spawn.x as f32, spawn.y as f32),
            one_over_mass: 1.0,
            timestep: 1.0,
            gravity: GRAV,
            e_damping: 0.99,
            ..Default::default()
        };

        let weapons = [
            weapons.get(WeaponKind::DesertEagles),
            weapons.get(WeaponKind::Chainsaw),
            weapons.get(WeaponKind::FragGrenade),
        ];

        Soldier {
            active: true,
            dead_meat: false,
            style: 0,
            num: 1,
            visible: 1,
            on_ground: false,
            on_ground_for_law: false,
            on_ground_last_frame: false,
            on_ground_permanent: false,
            direction: 1,
            old_direction: 1,
            health: 150.0,
            alpha: 255,
            jets_count: 0,
            jets_count_prev: 0,
            wear_helmet: 0,
            has_cigar: 0,
            vest: 0.0,
            idle_time: DEFAULT_IDLETIME,
            idle_random: -1,
            position: 0,
            on_fire: 0,
            collider_distance: 255,
            half_dead: false,
            skeleton: data.soldier_skeleton.clone(),
            legs_animation: data.anims.state(Anim::Stand),
            body_animation: data.anims.state(Anim::Stand),
            anims: data.anims.clone(),
            control: Default::default(),
            active_weapon: 0,
            weapons,
            fired: 0,
            burst_count: 0,
            can_auto_reload_spas: true,
            bullet_count: 0,
            ceasefire_counter: -1,
            team: Team::None,
            next_push: Vec2::ZERO,
            respawn_counter: 0,
            dead_time: 0,
            dead_collide_count: 0,
            kills: 0,
            deaths: 0,
            grenade_can_throw: false,
            holded_thing: None,
            has_pack: false,
            pending_drop: None,
            release_things: false,
            loadout: [
                WeaponKind::DesertEagles,
                WeaponKind::Chainsaw,
                WeaponKind::FragGrenade,
            ],
            particle,
        }
    }

    pub fn legs_apply_animation(&mut self, id: Anim, frame: usize) {
        if !self.legs_animation.is_any(&[Anim::Prone, Anim::ProneMove])
            && self.legs_animation.id != id
        {
            self.legs_animation = self.anims.state(id);
            self.legs_animation.frame = frame;
        }
    }

    pub fn body_apply_animation(&mut self, id: Anim, frame: usize) {
        if self.body_animation.id != id {
            self.body_animation = self.anims.state(id);
            self.body_animation.frame = frame;
        }
    }

    /// `TSprite.DropWeapon`: lets go of the primary weapon; the world turns
    /// `pending_drop` into a thing.
    pub fn drop_weapon(&mut self, config: &WorldConfig) {
        let weapon = *self.primary_weapon();
        if let Some(kind) = ThingKind::for_weapon(weapon.kind) {
            self.pending_drop = Some(WeaponDrop {
                kind,
                ammo_count: weapon.ammo_count,
                pos: self.skeleton.pos(16),
                velocity: self.particle.velocity,
                aim: self.cursor_aim_direction(),
                dead: self.dead_meat,
            });
        }
        // TODO: Rambo mode drops the bow
        if weapon.kind != WeaponKind::NoWeapon {
            self.weapons[self.active_weapon] = config.weapons.get(WeaponKind::NoWeapon);
        }
    }

    /// Whether a point is outside the map area soldiers may be in (`CheckOutOfBounds`).
    fn out_of_bounds(&self, map: &MapFile, pos: Vec2) -> bool {
        let bound = (map.sectors_num * map.sectors_division - 50) as f32;
        pos.x.abs() > bound || pos.y.abs() > bound
    }

    /// Port of `TSprite.HandleSpecialPolyTypes` (server side: damage, lava flames,
    /// explosive polygons).
    pub fn handle_special_polytypes(&mut self, polytype: PolyType, pos: Vec2, env: &mut PolyEnv) {
        let velocity = self.particle.velocity;
        let hit = |soldier: &mut Soldier, amount: f32, env: &mut PolyEnv| {
            if let Some(how) = soldier.take_damage(amount, 12, velocity, env.config) {
                env.emitter.push(EmitterItem::Died(how));
            }
        };

        match polytype {
            PolyType::Deadly => hit(self, 50.0 + self.health, env),
            PolyType::BloodyDeadly => hit(self, 450.0 + self.health, env),
            PolyType::Hurts | PolyType::Lava => {
                if !self.dead_meat {
                    if env.rng.below(10) == 0 {
                        self.health -= 5.0;
                    }
                    if self.health < 1.0 {
                        hit(self, 10.0, env);
                    }
                }

                if env.rng.below(3) == 0 && polytype == PolyType::Lava {
                    let a = vec2(pos.x, pos.y - 3.0);
                    if env.rng.below(3) == 0 {
                        let flamer = env.config.weapons.get(WeaponKind::Flamer);
                        env.emitter.push(EmitterItem::Bullet(BulletParams {
                            style: flamer.bullet_style,
                            weapon: flamer.kind,
                            position: a,
                            velocity: -velocity,
                            timeout: flamer.timeout as i16,
                            hit_multiply: flamer.hit_multiply,
                            team: self.team,
                            sprite: flamer.bullet_sprite,
                            seed: None,
                            must_create: true,
                        }));
                    }
                }
            }
            PolyType::Regenerates => {
                if self.health < env.config.start_health() && env.tick.is_multiple_of(12) {
                    hit(self, -2.0, env);
                }
            }
            PolyType::Explosive => {
                if !self.dead_meat {
                    let m79 = env.config.weapons.get(WeaponKind::M79);
                    env.emitter.push(EmitterItem::Bullet(BulletParams {
                        style: m79.bullet_style,
                        weapon: m79.kind,
                        position: vec2(pos.x, pos.y - 3.0),
                        velocity: Vec2::ZERO,
                        timeout: m79.timeout as i16,
                        hit_multiply: m79.hit_multiply,
                        team: self.team,
                        sprite: m79.bullet_sprite,
                        seed: None,
                        must_create: true,
                    }));
                    hit(self, 4000.0, env);
                    self.health = -600.0;
                }
            }
            // TODO: hurts soldiers holding a flag (needs things)
            PolyType::HurtsFlaggers if self.health < 1.0 => hit(self, 10.0, env),
            _ => {}
        }
    }

    pub fn update(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        tick: u64,
        rng: &mut PascalRandom,
        emitter: &mut Vec<EmitterItem>,
    ) {
        let mut body_y = 0.0;
        let mut arm_s;

        self.particle.euler();

        // knock-back from hits last tick (NextPush[0] on the server)
        self.particle.velocity += self.next_push;
        self.next_push = Vec2::ZERO;

        self.control(map, config, rng, emitter);

        *self.skeleton.old_pos_mut(21) = self.skeleton.pos(21);
        *self.skeleton.old_pos_mut(23) = self.skeleton.pos(23);
        // *self.skeleton.old_pos_mut(25) = self.skeleton.pos(25);
        *self.skeleton.pos_mut(21) = self.skeleton.pos(9);
        *self.skeleton.pos_mut(23) = self.skeleton.pos(12);
        // *self.skeleton.pos_mut(25) = self.skeleton.pos(5);

        // Soldat also "adds" the velocity to 21, 23 and 25 here, but with Vec2Add, a function
        // whose result is discarded, so it has no effect. Point 25 doesn't exist in gostek.po.

        match self.position {
            POS_STAND => body_y = 8.0,
            POS_CROUCH => body_y = 9.0,
            POS_PRONE => {
                if self.body_animation.id == Anim::Prone {
                    if self.body_animation.frame > 9 {
                        body_y = -2.0
                    } else {
                        body_y = 14.0 - self.body_animation.frame as f32;
                    }
                } else {
                    body_y = 9.0;
                }

                if self.body_animation.id == Anim::ProneMove {
                    body_y = 0.0;
                }
            }
            _ => {}
        }

        if self.body_animation.id == Anim::GetUp {
            if self.body_animation.frame > 18 {
                body_y = 8.0;
            } else {
                body_y = 4.0;
            }
        }

        if self.control.mouse_aim_x as f32 >= self.particle.pos.x {
            self.direction = 1;
        } else {
            self.direction = -1;
        }

        for i in 1..21 {
            if self.skeleton.active(i) && !self.dead_meat {
                *self.skeleton.old_pos_mut(i) = self.skeleton.pos(i);
                let (dir, p) = (f32::from(self.direction), self.particle.pos);

                if !self.half_dead && ((1..=6).contains(&i) || (i == 17) || (i == 18)) {
                    let anim_pos = self.legs_animation.pos(i);
                    *self.skeleton.pos_mut(i) = vec2(p.x + dir * anim_pos.x, p.y + anim_pos.y);
                }

                if (7..=16).contains(&i) || i == 19 || i == 20 {
                    let anim_pos = self.body_animation.pos(i);
                    // same summation order as Soldat, floats aren't associative
                    let y = if self.half_dead {
                        9.0 + p.y + anim_pos.y
                    } else {
                        (self.skeleton.pos(6).y - (p.y - body_y)) + p.y + anim_pos.y
                    };
                    *self.skeleton.pos_mut(i) = vec2(p.x + dir * anim_pos.x, y);
                }
            }
        }

        let aim = vec2(
            self.control.mouse_aim_x as f32,
            self.control.mouse_aim_y as f32,
        );

        if !self.dead_meat {
            let pos = self.skeleton.pos(9);
            let r_norm = 0.1 * vec2normalize(self.skeleton.pos(12) - aim);
            let dir = f32::from(self.direction);

            *self.skeleton.pos_mut(12) = pos + vec2(-dir * r_norm.y, dir * r_norm.x);
            *self.skeleton.pos_mut(23) = pos + vec2(-dir * r_norm.y, dir * r_norm.x) * 50.0;
        }

        let not_aiming_anims = [
            Anim::Reload,
            Anim::ReloadBow,
            Anim::ClipIn,
            Anim::ClipOut,
            Anim::SlideBack,
            Anim::Change,
            Anim::ThrowWeapon,
            Anim::WeaponNone,
            Anim::Punch,
            Anim::Roll,
            Anim::RollBack,
            Anim::Cigar,
            Anim::Match,
            Anim::Smoke,
            Anim::Wipe,
            Anim::TakeOff,
            Anim::Groin,
            Anim::Piss,
            Anim::Mercy,
            Anim::Mercy2,
            Anim::Victory,
            Anim::Own,
            Anim::Melee,
        ];

        if self.body_animation.id == Anim::Throw {
            arm_s = -5.00;
        } else {
            arm_s = -7.00;
        }

        if !self.dead_meat && !self.body_animation.is_any(&not_aiming_anims) {
            let r_norm = arm_s * vec2normalize(self.skeleton.pos(15) - aim);
            *self.skeleton.pos_mut(15) = self.skeleton.pos(16) + r_norm;
        }

        if self.body_animation.id == Anim::Throw {
            arm_s = -6.00;
        } else {
            arm_s = -8.00;
        }

        if !self.dead_meat && !self.body_animation.is_any(&not_aiming_anims) {
            let r_norm = arm_s * vec2normalize(self.skeleton.pos(19) - aim);
            *self.skeleton.pos_mut(19) = self.skeleton.pos(16) - vec2(0.0, 4.0) + r_norm;
        }

        // dead part
        for i in 1..21 {
            if (self.dead_meat || self.half_dead) && (i < 17) && (i != 7) && (i != 8) {
                let (x, y) = self.skeleton.pos(i).into();
                self.on_ground = self.check_skeleton_map_collision(map, i, x, y);
            }
        }

        if !self.dead_meat {
            self.body_animation.do_animation();
            self.legs_animation.do_animation();

            // CheckOutOfBounds
            if self.out_of_bounds(map, self.particle.pos) {
                self.respawn(map, config, rng);
            }

            self.on_ground = false;

            let (x, y) = self.particle.pos.into();
            self.check_map_collision(
                map,
                x - 3.5,
                y - 12.0,
                1,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            let (x, y) = self.particle.pos.into();
            self.check_map_collision(
                map,
                x + 3.5,
                y - 12.0,
                1,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            body_y = 0.0;
            arm_s = 0.0;

            // Walking either left or right (though only one can be active at once)
            if self.control.left ^ self.control.right {
                if self.control.left ^ (self.direction == 1) {
                    arm_s = 0.25;
                } else {
                    body_y = 0.25;
                }
            }
            // If a leg is inside a polygon, caused by the modification of ArmS and
            // BodyY, this is there to not lose contact to ground on slope polygons
            let (x, y) = self.particle.pos.into();

            if body_y == 0.0 {
                let leg = vec2(x + 2.0, y + 1.9);
                if map.ray_cast(leg, leg, 10.0, RayCast::default()).is_some() {
                    body_y = 0.25;
                }
            }
            if arm_s == 0.0 {
                let leg = vec2(x - 2.0, y + 1.9);
                if map.ray_cast(leg, leg, 10.0, RayCast::default()).is_some() {
                    arm_s = 0.25;
                }
            }

            let (x, y) = self.particle.pos.into();
            self.on_ground = self.check_map_collision(
                map,
                x + 2.0,
                y + 2.0 - body_y,
                0,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            // Legs collision check. If collided then don't check the other side, as a double
            // collision would result in too much of a ground repelling force (Pascal's `or`
            // short-circuits here).
            if !self.on_ground {
                let (x, y) = self.particle.pos.into();
                self.on_ground = self.check_map_collision(
                    map,
                    x - 2.0,
                    y + 2.0 - arm_s,
                    0,
                    &mut PolyEnv {
                        config,
                        tick,
                        rng,
                        emitter,
                    },
                );
            }

            let (x, y) = self.particle.pos.into();
            let grounded = self.on_ground;
            self.on_ground_for_law = self.check_radius_map_collision(
                map,
                x,
                y - 1.0,
                grounded,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            let (x, y) = self.particle.pos.into();
            let grounded = self.on_ground || self.on_ground_for_law;
            self.on_ground |= self.check_map_vertices_collision(
                map,
                x,
                y,
                3.0,
                grounded,
                &mut PolyEnv {
                    config,
                    tick,
                    rng,
                    emitter,
                },
            );

            if !(self.on_ground ^ self.on_ground_last_frame) {
                self.on_ground_permanent = self.on_ground;
            }

            self.on_ground_last_frame = self.on_ground;

            self.weapon_handling();

            // JETS
            if (self.jets_count < map.start_jet)
                && !(self.control.jets)
                && (self.on_ground || tick.is_multiple_of(2))
            {
                self.jets_count += 1;
            }

            if self.ceasefire_counter > -1 {
                // TODO: blink alpha like Soldat (needs SinusCounter)
                self.ceasefire_counter -= 1;
            }

            self.alpha = 255;

            self.skeleton.do_verlet_timestep_for(22, 29);
            self.skeleton.do_verlet_timestep_for(24, 30);
        }

        if self.dead_meat {
            // physically integrate skeleton particles
            self.skeleton.do_verlet_timestep();
            self.particle.pos = self.skeleton.pos(12);

            // CheckSkeletonOutOfBounds
            if (1..=20).any(|i| self.out_of_bounds(map, self.skeleton.pos(i))) {
                self.respawn(map, config, rng);
            }

            // respawn countdown
            if self.respawn_counter < 1 {
                self.respawn(map, config, rng);
            }
            self.respawn_counter -= 1;

            // TODO: parachute
            self.dead_time += 1;
        }

        // safety (Sprites.pas clamps both axes symmetrically)
        self.particle.velocity = self
            .particle
            .velocity
            .clamp(Vec2::splat(-MAX_VELOCITY), Vec2::splat(MAX_VELOCITY));
    }

    pub fn check_map_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        area: i32,
        env: &mut PolyEnv,
    ) -> bool {
        let pos = vec2(x, y) + self.particle.velocity;
        let rx = ((pos.x / map.sectors_division as f32).round()) as i32 + 25;
        let ry = ((pos.y / map.sectors_division as f32).round()) as i32 + 25;

        if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
            for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                let polytype = map.polygons[poly].polytype;

                if soldier_collides(polytype, self.team, false) {
                    let polygons = map.polygons[poly];
                    if map.point_in_poly(pos, &polygons) {
                        self.handle_special_polytypes(polytype, pos, env);

                        // hit the ground hard
                        if env.config.realistic_mode
                            && self.particle.velocity.y > 3.5
                            && polytype != PolyType::Bouncy
                        {
                            let amount = self.particle.velocity.y * 5.0;
                            if let Some(how) = self.take_damage(amount, 12, vec2(x, y), env.config)
                            {
                                env.emitter.push(EmitterItem::Died(how));
                            }
                        }

                        let mut dist = 0.0;
                        let mut k = 0;

                        let mut perp =
                            map.closest_perpendicular(poly as i32, pos, &mut dist, &mut k);

                        let step = perp;

                        perp = vec2normalize(perp);
                        perp *= dist;
                        dist = vec2length(self.particle.velocity);

                        if vec2length(perp) > dist {
                            perp = vec2normalize(perp);
                            perp *= dist;
                        }
                        if (area == 0)
                            || ((area == 1)
                                && ((self.particle.velocity.y < 0.0)
                                    || (ext(self.particle.velocity.x) > SLIDELIMIT)
                                    || (ext(self.particle.velocity.x) < -SLIDELIMIT)))
                        {
                            self.particle.old_pos = self.particle.pos;
                            self.particle.pos -= perp;
                            if map.polygons[poly].polytype == PolyType::Bouncy {
                                perp = vec2normalize(perp);
                                perp *= map.polygons[poly].bounciness * dist;
                            }
                            self.particle.velocity -= perp;
                        }

                        if area == 0 {
                            if (self.legs_animation.id == Anim::Stand)
                                || (self.legs_animation.id == Anim::Crouch)
                                || (self.legs_animation.id == Anim::Prone)
                                || (self.legs_animation.id == Anim::ProneMove)
                                || (self.legs_animation.id == Anim::GetUp)
                                || (self.legs_animation.id == Anim::Fall)
                                || (self.legs_animation.id == Anim::Mercy)
                                || (self.legs_animation.id == Anim::Mercy2)
                                || (self.legs_animation.id == Anim::Own)
                            {
                                if (ext(self.particle.velocity.x) < SLIDELIMIT)
                                    && (ext(self.particle.velocity.x) > -SLIDELIMIT)
                                    && (ext(step.y) > SLIDELIMIT)
                                {
                                    self.particle.pos = self.particle.old_pos;
                                    self.particle.force.y -= GRAV;
                                }

                                if (ext(step.y) > SLIDELIMIT)
                                    && (polytype != PolyType::Ice)
                                    && (polytype != PolyType::Bouncy)
                                {
                                    if (self.legs_animation.id == Anim::Stand)
                                        || (self.legs_animation.id == Anim::Fall)
                                        || (self.legs_animation.id == Anim::Crouch)
                                    {
                                        self.particle.velocity.x =
                                            fpc(ext(self.particle.velocity.x) * STANDSURFACECOEFX);
                                        self.particle.velocity.y =
                                            fpc(ext(self.particle.velocity.y) * STANDSURFACECOEFY);

                                        self.particle.force.x -= self.particle.velocity.x;
                                    } else if self.legs_animation.id == Anim::Prone {
                                        if self.legs_animation.frame > 24 {
                                            if !(self.control.down
                                                && (self.control.left || self.control.right))
                                            {
                                                self.particle.velocity.x =
                                                    fpc(ext(self.particle.velocity.x)
                                                        * STANDSURFACECOEFX);
                                                self.particle.velocity.y =
                                                    fpc(ext(self.particle.velocity.y)
                                                        * STANDSURFACECOEFY);

                                                self.particle.force.x -= self.particle.velocity.x;
                                            }
                                        } else {
                                            self.particle.velocity.x =
                                                fpc(ext(self.particle.velocity.x) * SURFACECOEFX);
                                            self.particle.velocity.y =
                                                fpc(ext(self.particle.velocity.y) * SURFACECOEFY);
                                        }
                                    } else if self.legs_animation.id == Anim::GetUp {
                                        self.particle.velocity.x =
                                            fpc(ext(self.particle.velocity.x) * SURFACECOEFX);
                                        self.particle.velocity.y =
                                            fpc(ext(self.particle.velocity.y) * SURFACECOEFY);
                                    } else if self.legs_animation.id == Anim::ProneMove {
                                        self.particle.velocity.x =
                                            fpc(ext(self.particle.velocity.x) * STANDSURFACECOEFX);
                                        self.particle.velocity.y =
                                            fpc(ext(self.particle.velocity.y) * STANDSURFACECOEFY);
                                    }
                                }
                            } else if (self.legs_animation.id == Anim::CrouchRun)
                                || (self.legs_animation.id == Anim::CrouchRunBack)
                            {
                                self.particle.velocity.x =
                                    fpc(ext(self.particle.velocity.x) * CROUCHMOVESURFACECOEFX);
                                self.particle.velocity.y =
                                    fpc(ext(self.particle.velocity.y) * CROUCHMOVESURFACECOEFY);
                            } else {
                                self.particle.velocity.x =
                                    fpc(ext(self.particle.velocity.x) * SURFACECOEFX);
                                self.particle.velocity.y =
                                    fpc(ext(self.particle.velocity.y) * SURFACECOEFY);
                            }
                        }

                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn check_map_vertices_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        r: f32,
        has_collided: bool,
        env: &mut PolyEnv,
    ) -> bool {
        let pos = vec2(x, y);
        let rx = ((pos.x / map.sectors_division as f32).round()) as i32 + 25;
        let ry = ((pos.y / map.sectors_division as f32).round()) as i32 + 25;

        if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
            for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                let polytype = map.polygons[poly].polytype;

                if soldier_collides(polytype, self.team, false) {
                    for i in 0..3 {
                        let vert = vec2(
                            map.polygons[poly].vertices[i].x,
                            map.polygons[poly].vertices[i].y,
                        );

                        let dist = distance(vert, pos);
                        if dist < r {
                            if !has_collided {
                                self.handle_special_polytypes(polytype, pos, env);
                            }
                            let mut dir = pos - vert;
                            dir = vec2normalize(dir);
                            self.particle.pos += dir;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn check_radius_map_collision(
        &mut self,
        map: &MapFile,
        x: f32,
        y: f32,
        has_collided: bool,
        env: &mut PolyEnv,
    ) -> bool {
        let mut s_pos = vec2(x, y - 3.0);

        let mut det_acc = vec2length(self.particle.velocity).trunc() as i32;
        if det_acc == 0 {
            det_acc = 1;
        }

        let step = self.particle.velocity * (1.0 / det_acc as f32);

        for _z in 0..det_acc {
            s_pos += step;

            let rx = ((s_pos.x / map.sectors_division as f32).round()) as i32 + 25;
            let ry = ((s_pos.y / map.sectors_division as f32).round()) as i32 + 25;

            if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
                for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                    let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                    let polytype = map.polygons[poly].polytype;

                    if soldier_radius_collides(polytype, self.team, false) {
                        for k in 0..3 {
                            let mut norm = map.perps[poly][k];
                            norm *= -SOLDIER_COL_RADIUS;

                            let pos = s_pos + norm;

                            if map.point_in_poly_edges(pos.x, pos.y, poly as i32) {
                                if !has_collided {
                                    self.handle_special_polytypes(polytype, pos, env);
                                }
                                let mut d = 0.0;
                                let mut b = 0;
                                let mut perp =
                                    map.closest_perpendicular(poly as i32, s_pos, &mut d, &mut b);

                                let mut p1 = vec2(0.0, 0.0);
                                let mut p2 = vec2(0.0, 0.0);
                                match b {
                                    1 => {
                                        p1 = vec2(
                                            map.polygons[poly].vertices[0].x,
                                            map.polygons[poly].vertices[0].y,
                                        );
                                        p2 = vec2(
                                            map.polygons[poly].vertices[1].x,
                                            map.polygons[poly].vertices[1].y,
                                        );
                                    }
                                    2 => {
                                        p1 = vec2(
                                            map.polygons[poly].vertices[1].x,
                                            map.polygons[poly].vertices[1].y,
                                        );
                                        p2 = vec2(
                                            map.polygons[poly].vertices[2].x,
                                            map.polygons[poly].vertices[2].y,
                                        );
                                    }
                                    3 => {
                                        p1 = vec2(
                                            map.polygons[poly].vertices[2].x,
                                            map.polygons[poly].vertices[2].y,
                                        );
                                        p2 = vec2(
                                            map.polygons[poly].vertices[0].x,
                                            map.polygons[poly].vertices[0].y,
                                        );
                                    }
                                    _ => {}
                                }

                                let p3 = pos;
                                let d = point_line_distance(p1, p2, p3);
                                perp *= d;

                                self.particle.pos = self.particle.old_pos;
                                self.particle.velocity = self.particle.force - perp;

                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    pub fn check_skeleton_map_collision(
        &mut self,
        map: &MapFile,
        i: usize,
        x: f32,
        y: f32,
    ) -> bool {
        let mut result = false;
        let pos = vec2(x - 1.0, y + 4.0);
        let rx = ((pos.x / map.sectors_division as f32).round()) as i32 + 25;
        let ry = ((pos.y / map.sectors_division as f32).round()) as i32 + 25;

        if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
            for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] - 1;
                let polytype = map.polygons[poly as usize].polytype;

                if soldier_collides(polytype, self.team, false)
                    && map.point_in_poly_edges(pos.x, pos.y, i32::from(poly))
                {
                    let mut dist = 0.0;
                    let mut b = 0;
                    let mut perp =
                        map.closest_perpendicular(i32::from(poly), pos, &mut dist, &mut b);
                    perp = vec2normalize(perp);
                    perp *= dist;

                    *self.skeleton.pos_mut(i) = self.skeleton.old_pos(i) - perp;
                    self.dead_collide_count += 1;
                    result = true;
                }
            }
        }

        if result {
            let pos = vec2(x, y + 1.0);
            let rx = ((pos.x / map.sectors_division as f32).round()) as i32 + 25;
            let ry = ((pos.y / map.sectors_division as f32).round()) as i32 + 25;

            if (rx > 0) && (rx < map.sectors_num + 25) && (ry > 0) && (ry < map.sectors_num + 25) {
                for j in 0..map.sectors_poly[rx as usize][ry as usize].polys.len() {
                    let poly = map.sectors_poly[rx as usize][ry as usize].polys[j] - 1;
                    let polytype = map.polygons[poly as usize].polytype;

                    if !matches!(polytype, PolyType::NoCollide | PolyType::OnlyBulletsCollide)
                        && map.point_in_poly_edges(pos.x, pos.y, i32::from(poly))
                    {
                        let mut dist = 0.0;
                        let mut b = 0;
                        let mut perp =
                            map.closest_perpendicular(i32::from(poly), pos, &mut dist, &mut b);
                        perp = vec2normalize(perp);
                        perp *= dist;

                        *self.skeleton.pos_mut(i) = self.skeleton.old_pos(i) - perp;
                        result = true;
                    }
                }
            }
        }

        result
    }

    /// Fire interval countdown and reloading (`TSprite.Update`, "WEAPON HANDLING"),
    /// as run by the owning client.
    fn weapon_handling(&mut self) {
        let index = self.active_weapon;
        let kind = self.weapons[index].kind;
        let spas = kind == WeaponKind::Spas12;

        {
            let w = &mut self.weapons[index];
            if w.fire_interval_count > 0 && (w.ammo_count > 0 || spas) {
                w.fire_interval_prev = w.fire_interval_count;
                w.fire_interval_count -= 1;
            }
        }

        // If fire button is released, then the reload can begin
        if !self.control.fire {
            self.can_auto_reload_spas = true;
        }

        let busy = [
            Anim::Roll,
            Anim::RollBack,
            Anim::Melee,
            Anim::Change,
            Anim::Throw,
            Anim::ThrowWeapon,
        ];

        // reload
        if self.weapons[index].ammo_count == 0
            && (kind == WeaponKind::Chainsaw || !self.body_animation.is_any(&busy))
        {
            if self.body_animation.id != Anim::GetUp {
                // spas does the fire interval delay and then reloads, other weapons the opposite
                if spas {
                    if self.weapons[index].fire_interval_count == 0 && self.can_auto_reload_spas {
                        self.body_apply_animation(Anim::Reload, 1);
                    }
                } else if kind == WeaponKind::Bow || kind == WeaponKind::FlameBow {
                    self.body_apply_animation(Anim::ReloadBow, 1);
                } else if !self.body_animation.is_any(&[Anim::ClipIn, Anim::SlideBack])
                    && (kind != WeaponKind::Chainsaw || !self.body_animation.is_any(&busy))
                {
                    self.body_apply_animation(Anim::ClipOut, 1);
                }

                self.burst_count = 0;
            }

            // TODO: reload sounds and clip sparks (client effects)

            if !spas {
                let w = &mut self.weapons[index];
                w.reload_time_prev = w.reload_time_count;
                w.reload_time_count = w.reload_time_count.saturating_sub(1);
                w.fire_interval_prev = w.fire_interval;
                w.fire_interval_count = w.fire_interval;

                if w.reload_time_count < 1 {
                    w.refill();
                }
            }
        }

        // weapon jam fix
        let w = &mut self.weapons[index];
        if w.ammo_count == 0 && !spas {
            if w.reload_time_count < 1 {
                w.refill();
            }
            if w.reload_time_count > w.reload_time {
                w.reload_time_prev = w.reload_time;
                w.reload_time_count = w.reload_time;
            }
        }
    }

    /// `TSprite.GetCursorAimDirection`
    pub fn cursor_aim_direction(&self) -> Vec2 {
        let mouse_aim = vec2(
            self.control.mouse_aim_x as f32,
            self.control.mouse_aim_y as f32,
        );
        vec2normalize(mouse_aim - self.skeleton.pos(15))
    }

    /// `TSprite.GetHandsAimDirection`
    pub fn hands_aim_direction(&self) -> Vec2 {
        vec2normalize(self.skeleton.pos(15) - self.skeleton.pos(16))
    }

    /// `TSprite.GetMoveacc`: extra inaccuracy from moving.
    pub fn moveacc(&self) -> f32 {
        let moveacc = self.primary_weapon().movement_acc;
        let legs = &self.legs_animation;

        if moveacc <= 0.0 {
            return 0.0;
        }

        if (self.control.jets && self.jets_count > 0)
            || legs.is_any(&[
                Anim::Jump,
                Anim::JumpSide,
                Anim::Run,
                Anim::RunBack,
                Anim::Roll,
                Anim::RollBack,
            ])
        {
            moveacc * 7.0
        } else if (!self.on_ground_permanent
            && !legs.is_any(&[
                Anim::Prone,
                Anim::ProneMove,
                Anim::Crouch,
                Anim::CrouchRun,
                Anim::CrouchRunBack,
            ]))
            || legs.id == Anim::GetUp
            || (legs.id == Anim::Prone && legs.frame < legs.num_frames())
        {
            moveacc * 3.0
        } else {
            0.0
        }
    }

    /// Port of `TSprite.Fire`.
    pub fn fire(&mut self, map: &MapFile, rng: &mut PascalRandom, emitter: &mut Vec<EmitterItem>) {
        let weapon = *self.primary_weapon();
        let kind = weapon.kind;
        let team = self.team;
        let mercy = self.body_animation.is_any(&[Anim::Mercy, Anim::Mercy2]);
        let legs = self.legs_animation.clone();

        // Create a normalized directional vector
        let aim_direction = if weapon.bullet_style == BulletStyle::Blade || mercy {
            self.hands_aim_direction()
        } else {
            self.cursor_aim_direction()
        };

        let mut b = aim_direction;
        let mut a = vec2(
            self.skeleton.pos(15).x - b.x * 4.0,
            self.skeleton.pos(15).y - b.y * 4.0 - 2.0,
        );

        // TODO: self-bink (HitSprayCounter) for the local player
        let mut inaccuracy = self.moveacc();

        // Bullet spread
        if kind != WeaponKind::DesertEagles
            && kind != WeaponKind::Spas12
            && weapon.bullet_style != BulletStyle::GaugeBullet
            && weapon.bullet_spread > 0.0
        {
            if legs.id == Anim::ProneMove || (legs.id == Anim::Prone && legs.frame > 23) {
                inaccuracy += weapon.bullet_spread / 1.625;
            } else if legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
                || (legs.id == Anim::Crouch && legs.frame > 13)
            {
                inaccuracy += weapon.bullet_spread / 1.3;
            } else {
                inaccuracy += weapon.bullet_spread;
            }
        }

        inaccuracy = f32::min(inaccuracy * 0.25, MAX_INACCURACY);

        // Maximum deviation between 0 and MAX_INACCURACY, scaled like sin(0..pi/2)
        let max_deviation = MAX_INACCURACY * f32::sin((inaccuracy / MAX_INACCURACY) * (PI / 2.0));
        let d = vec2(
            random_spread(rng, max_deviation, 0.0),
            random_spread(rng, max_deviation, 0.0),
        );

        // Add inaccuracies to the direction, re-normalize and scale by speed
        b = vec2normalize(b + d) * weapon.speed;

        // Add some of the player's velocity to the bullet
        b += self.particle.velocity * weapon.inherited_velocity;

        // Head inside a polygon: offset the origin downward slightly
        if map.collision_test(a, false).is_some() {
            a.y += 2.5;
        }

        let mut create = |position: Vec2, velocity: Vec2, seed: Option<u16>| {
            emitter.push(EmitterItem::Bullet(BulletParams {
                style: weapon.bullet_style,
                weapon: kind,
                position,
                velocity,
                timeout: weapon.timeout as i16,
                hit_multiply: weapon.hit_multiply,
                team,
                sprite: weapon.bullet_sprite,
                seed,
                must_create: false,
            }));
        };

        let plain = !matches!(
            kind,
            WeaponKind::DesertEagles
                | WeaponKind::Spas12
                | WeaponKind::Flamer
                | WeaponKind::NoWeapon
                | WeaponKind::Knife
                | WeaponKind::Chainsaw
                | WeaponKind::LAW
        );

        if plain || mercy {
            create(a, b, None);
        }

        if kind == WeaponKind::DesertEagles {
            self.bullet_count = self.bullet_count.wrapping_add(1);
            let saved_seed = rng.rand_seed;
            rng.rand_seed = u32::from(self.bullet_count);

            let d1 = vec2(
                random_spread(rng, weapon.bullet_spread, b.x),
                random_spread(rng, weapon.bullet_spread, b.y),
            );
            create(a, d1, Some(self.bullet_count));

            let d2 = vec2(
                random_spread(rng, weapon.bullet_spread, b.x),
                random_spread(rng, weapon.bullet_spread, b.y),
            );
            rng.rand_seed = saved_seed;

            let b_norm = vec2normalize(b);
            a.x -= pascal_sign(b.x) * b_norm.y.abs() * 3.0;
            a.y += pascal_sign(b.y) * b_norm.x.abs() * 3.0;
            create(a, d2, None);
        }

        if weapon.bullet_style == BulletStyle::GaugeBullet {
            self.bullet_count = self.bullet_count.wrapping_add(1);
            let saved_seed = rng.rand_seed;
            rng.rand_seed = u32::from(self.bullet_count);
            let first = vec2(
                random_spread(rng, weapon.bullet_spread, b.x),
                random_spread(rng, weapon.bullet_spread, b.y),
            );
            rng.rand_seed = saved_seed;
            create(a, first, Some(self.bullet_count));

            // remaining 5 pellets
            for _ in 0..5 {
                let pellet = vec2(
                    random_spread(rng, weapon.bullet_spread, b.x),
                    random_spread(rng, weapon.bullet_spread, b.y),
                );
                create(a, pellet, None);
            }

            self.particle.velocity -= vec2(b.x * 0.0412, b.y * 0.041);
        }

        if kind == WeaponKind::Minigun {
            let mut push = if self.control.jets && self.jets_count > 0 {
                vec2(b.x * 0.0012, b.y * 0.0009)
            } else {
                vec2(b.x * 0.0082, b.y * 0.0078)
            };
            // TODO: halve the push while holding a flag (HoldedThing)
            push.x *= 0.6;
            self.particle.velocity -= push;
        }

        if kind == WeaponKind::Flamer || kind == WeaponKind::Chainsaw {
            a += b * 2.0;
            create(a, b, None);
        }

        if kind == WeaponKind::LAW {
            let low_stance = (legs.id == Anim::Crouch && legs.frame > 13)
                || legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
                || (legs.id == Anim::Prone && legs.frame > 23);

            if (self.on_ground || self.on_ground_permanent || self.on_ground_for_law) && low_stance
            {
                create(a, b, None);
            } else {
                return;
            }
        }

        // TODO: mercy animation makes the bullet hit the shooter's own head

        let weapon = &mut self.weapons[self.active_weapon];
        if weapon.ammo_count > 0 {
            weapon.ammo_count -= 1;
        }
        if kind == WeaponKind::Spas12 {
            self.can_auto_reload_spas = false;
        }
        weapon.fire_interval_prev = weapon.fire_interval;
        weapon.fire_interval_count = weapon.fire_interval;

        self.apply_recoil_animation(kind);

        self.burst_count = self.burst_count.saturating_add(1);

        // TODO: self-bink accumulation and mouse recoil for the local player
    }

    /// Body animation played when a weapon fires (from `TSprite.Fire`).
    fn apply_recoil_animation(&mut self, kind: WeaponKind) {
        let body = self.body_animation.id;
        let free = !matches!(body, Anim::Throw | Anim::GetUp | Anim::Melee);
        let standing = self.position == POS_STAND;
        let crouching = self.position == POS_CROUCH;
        let prone = self.position == POS_PRONE;

        let crouch_recoil = |soldier: &mut Soldier| {
            if crouching {
                if soldier.body_animation.id == Anim::HandsUpAim {
                    soldier.body_apply_animation(Anim::HandsUpRecoil, 1);
                } else {
                    soldier.body_apply_animation(Anim::AimRecoil, 1);
                }
            }
        };

        match kind {
            WeaponKind::Ak74
            | WeaponKind::Minimi
            | WeaponKind::MP5
            | WeaponKind::DesertEagles
            | WeaponKind::SteyrAUG
            | WeaponKind::USSOCOM
            | WeaponKind::Bow
            | WeaponKind::FlameBow => {
                if free && standing {
                    self.body_apply_animation(Anim::SmallRecoil, 1);
                }
                crouch_recoil(self);
            }
            WeaponKind::Ruger77 => {
                if free && standing {
                    self.body_apply_animation(Anim::Recoil, 1);
                }
                crouch_recoil(self);
            }
            WeaponKind::Spas12 => {
                if free && !prone {
                    self.body_apply_animation(Anim::Shotgun, 1);
                }
                // make sure firing interrupts reloading when prone
                if prone && self.body_animation.id == Anim::Reload {
                    self.body_animation.frame = self.body_animation.num_frames();
                }
            }
            WeaponKind::M79 => {
                if free && !prone {
                    self.body_apply_animation(Anim::SmallRecoil, 1);
                }
            }
            WeaponKind::Barrett if free => self.body_apply_animation(Anim::Barret, 1),
            WeaponKind::Minigun if free && standing => {
                self.body_apply_animation(Anim::SmallRecoil, 2)
            }
            _ => {}
        }
    }
}

/// `(Random * 2 - 1) * spread`, evaluated in double precision like Free Pascal's
/// extended arithmetic before rounding to single.
fn random_spread(rng: &mut PascalRandom, spread: f32, offset: f32) -> f32 {
    ((rng.float() * 2.0 - 1.0) * f64::from(spread) + f64::from(offset)) as f32
}

/// Pascal's `Sign`: -1, 0 or 1.
pub(crate) fn pascal_sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}
