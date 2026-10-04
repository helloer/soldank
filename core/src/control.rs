use super::*;

// untyped Pascal constants: extended precision, see `ext`/`fpc`
const RUNSPEED: f64 = 0.118;
const RUNSPEEDUP: f64 = RUNSPEED / 6.0;
const FLYSPEED: f64 = 0.03;
const JUMPSPEED: f64 = 0.66;
const CROUCHRUNSPEED: f64 = RUNSPEED / 0.6;
const PRONESPEED: f64 = RUNSPEED * 4.0;
const ROLLSPEED: f64 = RUNSPEED / 1.2;
const JUMPDIRSPEED: f64 = 0.30;
const JETSPEED: f64 = 0.10;
pub(crate) const M2GUN_OVERAIM: i16 = 4;
pub(crate) const M2GUN_OVERHEAT: i16 = 18;
const SECOND: i32 = 60;

pub(crate) const DEFAULT_IDLETIME: i32 = SECOND * 8;
const LONGER_IDLETIME: i32 = SECOND * 30;

#[derive(Default, Debug, Clone, Copy)]
pub struct Control {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub fire: bool,
    pub jets: bool,
    pub throw_nade: bool,
    pub change_weapon: bool,
    pub throw_weapon: bool,
    pub reload: bool,
    pub prone: bool,
    pub flag_throw: bool,
    pub mouse_aim_x: i32,
    pub mouse_aim_y: i32,
    pub mouse_dist: i32,
    pub was_running_left: bool,
    pub was_jumping: bool,
    pub was_throwing_weapon: bool,
    pub was_changing_weapon: bool,
    pub was_throwing_grenade: bool,
    pub was_reloading_weapon: bool,
}

impl Soldier {
    /// `AimDistCoef` (sniper view): aiming far with a ready Barrett while prone or crouched
    /// lets the camera look farther, with the scope's sounds.
    fn sniper_view(&mut self, tick: u64) {
        let weapon = *self.primary_weapon();
        let body = self.body_animation.id;
        if weapon.kind != WeaponKind::Barrett {
            self.aim_dist_coef = DEFAULT_AIM_DIST;
            self.control.mouse_dist = 150;
            return;
        }
        if weapon.fire_interval_count != 0 || !matches!(body, Anim::Prone | Anim::Aim) {
            if self.aim_dist_coef != DEFAULT_AIM_DIST {
                self.play(Sfx::Scopeback);
            }
            self.aim_dist_coef = DEFAULT_AIM_DIST;
            self.control.mouse_dist = 150;
            return;
        }

        let aim = vec2(
            self.control.mouse_aim_x as f32,
            self.control.mouse_aim_y as f32,
        );
        let d = (aim - self.particle.pos).abs();
        let run_sound = tick.is_multiple_of(27);
        let step = |coef: f32, by: f64| fpc(ext(coef) + by);

        if f64::from(d.x) >= 640.0 / 1.035 || f64::from(d.y) >= 480.0 / 1.035 {
            if self.aim_dist_coef == DEFAULT_AIM_DIST {
                self.play(Sfx::Scope);
            }
            let (limit, by) = if body == Anim::Prone {
                (SNIPER_AIM_DIST, AIM_DIST_INCR)
            } else {
                (CROUCH_AIM_DIST, 2.0 * AIM_DIST_INCR)
            };
            if self.aim_dist_coef > limit {
                self.aim_dist_coef = step(self.aim_dist_coef, -by);
                if run_sound {
                    self.play(Sfx::Scoperun);
                }
            }
        }

        if f64::from(d.x) < 640.0 / 1.5
            && f64::from(d.y) < 480.0 / 1.5
            && self.aim_dist_coef < DEFAULT_AIM_DIST
        {
            self.aim_dist_coef = step(self.aim_dist_coef, AIM_DIST_INCR);
            if self.aim_dist_coef == DEFAULT_AIM_DIST {
                self.play(Sfx::Scope);
            }
            if run_sound {
                self.play(Sfx::Scoperun);
            }
        }
    }

    /// A shell flying out of the gun (client sparks): `spin` turns it with the aim.
    fn shell(&mut self, spin: f32, style: u8) {
        let dir = f32::from(self.direction);
        let mut b = self.hands_aim_direction() * self.primary_weapon().speed;
        let v = self.particle.velocity;
        b.x = dir * spin * b.y + v.x;
        b.y = -dir * spin * b.x + v.y;
        let hand = self.skeleton.pos(15);
        let a = vec2(
            hand.x + 2.0 - dir * 0.015 * b.x,
            hand.y - 2.0 - dir * 0.015 * b.y,
        );
        self.spark(a, b, style, 255);
    }

    /// A puff of cigar smoke.
    fn puff(&mut self) {
        let dir = f32::from(self.direction);
        let a = self.skeleton.pos(12) + vec2(dir * 4.0, 0.0);
        self.spark(a, vec2(0.0, -0.7), 31, 65);
    }

    /// Not firing: a spinning weapon stops, and if it had spun up it winds down (the
    /// client's sounds).
    fn wind_down(&mut self, kind: WeaponKind) {
        self.stop_sound(Channel::Gattling);
        if std::mem::take(&mut self.spin) {
            let legs = &self.legs_animation;
            let crouched = (legs.id == Anim::Crouch && legs.frame > 13)
                || legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
                || (legs.id == Anim::Prone && legs.frame > 23);
            if kind == WeaponKind::Minigun {
                self.play_on(Sfx::MinigunEnd, Channel::Gattling2);
            }
            if kind == WeaponKind::LAW && self.on_ground && crouched {
                self.play_on(Sfx::LawEnd, Channel::Gattling2);
            }
        }
    }

    pub fn control(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        tick: u64,
        rng: &mut PascalRandom,
        emitter: &mut Vec<EmitterItem>,
    ) {
        let mut player_pressed_left_right = false;

        if self.legs_animation.speed < 1 {
            self.legs_animation.speed = 1;
        }

        if self.body_animation.speed < 1 {
            self.body_animation.speed = 1;
        }

        // mouse_aim_x/y are set from the tick input (see Soldier::apply_input)

        let (mut cleft, mut cright) = (self.control.left, self.control.right);
        // the key handling of a client's own soldier; the server takes bots' keys as they are
        let human = self.brain.is_none();

        // If both left and right directions are pressed, then decide which direction to go in
        if !human {
        } else if cleft && cright {
            // Remember that both directions were pressed, as it's useful for some moves
            player_pressed_left_right = true;

            if self.control.was_jumping {
                // If jumping, keep going in the old direction
                if self.control.was_running_left {
                    cright = false;
                } else {
                    cleft = false;
                }
            } else {
                // If not jumping, instead go in the new direction
                if self.control.was_running_left {
                    cleft = false;
                } else {
                    cright = false;
                }
            }
        } else {
            self.control.was_running_left = cleft;
            self.control.was_jumping = self.control.up;
        }

        let conflicting_keys_pressed = |c: &Control| {
            (c.throw_nade as u8 + c.change_weapon as u8 + c.throw_weapon as u8 + c.reload as u8) > 1
        };

        // Handle simultaneous key presses that would conflict
        if !human {
        } else if conflicting_keys_pressed(&self.control) {
            // At least two buttons pressed, so deactivate any previous one
            if self.control.was_throwing_grenade {
                self.control.throw_nade = false;
            } else if self.control.was_changing_weapon {
                self.control.change_weapon = false;
            } else if self.control.was_throwing_weapon {
                self.control.throw_weapon = false;
            } else if self.control.was_reloading_weapon {
                self.control.reload = false;
            }

            // If simultaneously pressing two or more new buttons, then deactivate them in order
            // of least preference
            while conflicting_keys_pressed(&self.control) {
                if self.control.reload {
                    self.control.reload = false;
                } else if self.control.change_weapon {
                    self.control.change_weapon = false;
                } else if self.control.throw_weapon {
                    self.control.throw_weapon = false;
                } else if self.control.throw_nade {
                    self.control.throw_nade = false;
                }
            }
        } else {
            self.control.was_throwing_grenade = self.control.throw_nade;
            self.control.was_changing_weapon = self.control.change_weapon;
            self.control.was_throwing_weapon = self.control.throw_weapon;
            self.control.was_reloading_weapon = self.control.reload;
        }

        if self.dead_meat {
            self.control.free_controls();
            // Soldat resolves left/right in Control itself, so this clears them too
            (cleft, cright) = (false, false);
        }

        //self.fired = 0;
        self.control.mouse_aim_x =
            (self.control.mouse_aim_x as f32 + self.particle.velocity.x).round_ties_even() as i32;
        self.control.mouse_aim_y =
            (self.control.mouse_aim_y as f32 + self.particle.velocity.y).round_ties_even() as i32;

        if self.control.jets
            && (((self.legs_animation.id == Anim::JumpSide)
                && (((self.direction == -1) && cright)
                    || ((self.direction == 1) && cleft)
                    || player_pressed_left_right))
                || ((self.legs_animation.id == Anim::RollBack) && self.control.up))
        {
            self.body_apply_animation(Anim::RollBack, 1);
            self.legs_apply_animation(Anim::RollBack, 1);
        } else if self.control.jets && (self.jets_count > 0) {
            if self.on_ground {
                // Iif returns a Variant: double precision arithmetic
                let jet = iif!(
                    ext(config.gravity) > 0.05,
                    JETSPEED,
                    ext(config.gravity * 2.0)
                );
                self.particle.force.y = fpc(-2.5 * jet);
            } else if self.position != POS_PRONE {
                let jet = iif!(
                    ext(config.gravity) > 0.05,
                    JETSPEED,
                    ext(config.gravity * 2.0)
                );
                self.particle.force.y = fpc(ext(self.particle.force.y) - jet);
            } else {
                let jet = iif!(
                    ext(config.gravity) > 0.05,
                    JETSPEED,
                    ext(config.gravity * 2.0)
                );
                self.particle.force.x =
                    fpc(ext(self.particle.force.x) + f64::from(self.direction) * jet / 2.0);
            }

            if (self.legs_animation.id != Anim::GetUp)
                && (self.body_animation.id != Anim::Roll)
                && (self.body_animation.id != Anim::RollBack)
            {
                self.legs_apply_animation(Anim::Fall, 1);
            }

            // smoke and sparks out of both feet
            for (foot, from, to) in [(1, 4, 5), (2, 3, 6)] {
                let a = self.skeleton.pos(foot) + vec2(-1.0, 3.0);
                let b = vec2normalize(self.skeleton.pos(to) - self.skeleton.pos(from)) * -0.5;
                if fx::random(8) == 0 {
                    let v = self.particle.velocity;
                    self.spark(a, v, 1, 75);
                }
                if fx::random(7) == 0 {
                    self.spark(a, b, 62, 40);
                }
            }

            self.jets_count -= 1;
            self.play_on(Sfx::Rocketz, Channel::Jets);
        } else {
            self.stop_sound(Channel::Jets);
        }

        // KOLBA: buttstock a soldier standing right next to you
        if self.position == POS_STAND
            && self.control.fire
            && self.ceasefire_counter < 0
            && !self.primary_weapon().is_any(&[
                WeaponKind::NoWeapon,
                WeaponKind::Knife,
                WeaponKind::Chainsaw,
            ])
            && self.melee_reach
            && self.stat.is_none()
        {
            self.body_apply_animation(Anim::Melee, 1);
        }

        // FIRE!!!! (not while on a stationary gun)
        if self.stat.is_none() {
            let kind = self.primary_weapon().kind;
            let body = self.body_animation.id;

            if kind == WeaponKind::Chainsaw
                || !matches!(
                    body,
                    Anim::Roll | Anim::RollBack | Anim::Melee | Anim::Change
                )
            {
                if (body == Anim::HandsUpAim && self.body_animation.frame == 11)
                    || body != Anim::HandsUpAim
                {
                    if self.control.fire && self.ceasefire_counter < 0 {
                        if kind == WeaponKind::NoWeapon || kind == WeaponKind::Knife {
                            self.body_apply_animation(Anim::Punch, 1);
                        } else {
                            let weapon = *self.primary_weapon();

                            if weapon.fire_interval_count == 0 && weapon.ammo_count > 0 {
                                if weapon.start_up_time > 0 {
                                    self.stop_sound(Channel::Gattling2);
                                }
                                if weapon.start_up_time > 0 && weapon.start_up_time_count > 0 {
                                    let legs = &self.legs_animation;
                                    let crouched = (legs.id == Anim::Crouch && legs.frame > 13)
                                        || legs.is_any(&[Anim::CrouchRun, Anim::CrouchRunBack])
                                        || (legs.id == Anim::Prone && legs.frame > 23);
                                    let law_ready =
                                        (self.on_ground || self.on_ground_permanent) && crouched;

                                    // wind up (the client starts over each time the trigger is
                                    // pulled)
                                    if !self.spin {
                                        let sound = match kind {
                                            WeaponKind::Barrett => Some(Sfx::LawStart),
                                            WeaponKind::Minigun => Some(Sfx::MinigunStart),
                                            WeaponKind::LAW if self.on_ground && crouched => {
                                                Some(Sfx::LawStart)
                                            }
                                            _ => None,
                                        };
                                        if let Some(sfx) = sound {
                                            self.play_on(sfx, Channel::Gattling);
                                        }
                                    }

                                    if kind != WeaponKind::LAW || law_ready {
                                        self.weapons[self.active_weapon].start_up_time_count -= 1;
                                        self.spin = true;
                                    }
                                } else {
                                    self.fire(map, config, rng, emitter);
                                }
                            }
                        }
                    } else {
                        self.wind_down(kind);
                        if human {
                            // the local player's weapon winds down when not firing
                            let weapon = &mut self.weapons[self.active_weapon];
                            weapon.start_up_time_count = weapon.start_up_time;
                        }
                    }
                }
            } else {
                let weapon = &mut self.weapons[self.active_weapon];
                if weapon.start_up_time_count < weapon.start_up_time {
                    weapon.start_up_time_count = weapon.start_up_time;
                }
                self.burst_count = 0;
                self.spin = false;
            }
        }

        if human && !self.control.fire {
            self.burst_count = 0;
        }

        // Fire mode 2: single shot, holding the trigger doesn't refire (humans only)
        let weapon = &mut self.weapons[self.active_weapon];
        if human
            && weapon.fire_mode == 2
            && self.control.fire
            && (self.burst_count > 0 || self.control.reload)
            && weapon.fire_interval_count < 2
        {
            weapon.fire_interval_count += 1;
        }

        // ThrowFlag (server)
        if !self.body_animation.is_any(&[Anim::Roll, Anim::RollBack])
            && self.control.flag_throw
            && self.holded_thing.is_some()
        {
            self.flag_throw = Some(FlagThrow {
                hand: self.skeleton.pos(15),
                aim: self.cursor_aim_direction(),
                velocity: self.particle.velocity,
                direction: self.direction,
            });
        }

        self.throw_grenade(map, emitter);

        // change weapon animation
        let flamegod = self.bonus_style == Bonus::Flamegod;
        if (self.body_animation.id != Anim::Roll)
            && (self.body_animation.id != Anim::RollBack)
            && !flamegod
            && self.control.change_weapon
        {
            self.body_apply_animation(Anim::Change, 1);
            self.pause_sound(Channel::Reload, true);
        }

        // change weapon
        if self.body_animation.id == Anim::Change {
            if self.body_animation.frame == 2 {
                let sfx = match self.secondary_weapon().kind {
                    WeaponKind::USSOCOM => Sfx::Changespin,
                    WeaponKind::Knife => Sfx::Knife,
                    WeaponKind::Chainsaw => Sfx::ChainsawD,
                    _ => Sfx::Changeweapon,
                };
                self.play(sfx);
                self.body_animation.frame += 1;
            } else if self.body_animation.frame == 25 && !flamegod {
                self.switch_weapon();
            } else if (self.body_animation.frame == self.anims.get(Anim::Change).num_frames())
                && !flamegod
                && (self.primary_weapon().ammo_count == 0)
            {
                self.body_apply_animation(Anim::Stand, 1);
                self.pause_sound(Channel::Reload, false);
            }
        }

        // throw weapon
        if self.control.throw_weapon
            && !self.control.throw_nade
            && (self.body_animation.id != Anim::Change || self.body_animation.frame > 25)
            && !self
                .body_animation
                .is_any(&[Anim::Roll, Anim::RollBack, Anim::ThrowWeapon])
            && !flamegod
            && !self.primary_weapon().is_any(&[
                WeaponKind::Bow,
                WeaponKind::FlameBow,
                WeaponKind::NoWeapon,
            ])
        {
            self.body_apply_animation(Anim::ThrowWeapon, 1);

            if self.primary_weapon().kind == WeaponKind::Knife {
                self.body_animation.speed = 2;
            }
            self.stop_sound(Channel::Reload);
        }

        // reload
        if self.primary_weapon().kind == WeaponKind::Chainsaw
            || !self
                .body_animation
                .is_any(&[Anim::Roll, Anim::RollBack, Anim::Change])
        {
            let weapon = *self.primary_weapon();
            if self.control.reload && weapon.ammo_count != weapon.ammo {
                if weapon.kind == WeaponKind::Spas12 {
                    if weapon.ammo_count < weapon.ammo {
                        if weapon.fire_interval_count == 0 {
                            self.body_apply_animation(Anim::Reload, 1);
                        } else {
                            self.auto_reload_when_can_fire = true;
                        }
                    }
                } else {
                    let w = &mut self.weapons[self.active_weapon];
                    w.ammo_count = 0;
                    w.fire_interval_prev = w.fire_interval;
                    w.fire_interval_count = w.fire_interval;
                }
                self.burst_count = 0;
            }
        }

        // reload shotgun / reload spas, a shell at a time
        if self.body_animation.id == Anim::Reload && self.body_animation.frame == 7 {
            self.play_on(Sfx::Spas12Reload, Channel::Reload);
            self.body_animation.frame += 1;
        }

        if (!self.control.fire || self.primary_weapon().ammo_count == 0)
            && self.body_animation.id == Anim::Reload
            && self.body_animation.frame == 14
        {
            let w = &mut self.weapons[self.active_weapon];
            w.ammo_count += 1;
            if w.ammo_count < w.ammo {
                self.body_animation.frame = 1;
            }
        }

        // throw away weapon
        if self.body_animation.id == Anim::ThrowWeapon && self.body_animation.frame == 2 {
            self.play(Sfx::Throwgun);
        }
        if self.primary_weapon().kind != WeaponKind::Knife
            && self.body_animation.id == Anim::ThrowWeapon
            && self.body_animation.frame == 19
            && self.primary_weapon().kind != WeaponKind::NoWeapon
        {
            self.drop_weapon(config);
            self.body_apply_animation(Anim::Stand, 1);
        }

        // throw knife
        if self.body_animation.id == Anim::ThrowWeapon
            && self.primary_weapon().kind == WeaponKind::Knife
            && (!self.control.throw_weapon || self.body_animation.frame == 16)
        {
            let weapon = config.weapons.get(WeaponKind::ThrownKnife);
            let frame = self.body_animation.frame.clamp(8, 16);
            let d = fpc(frame as f64 / 16.0);
            let speed = fpc(ext(weapon.speed) * 1.5 * ext(d));
            let velocity = self.cursor_aim_direction() * speed
                + self.particle.velocity * weapon.inherited_velocity;

            emitter.push(EmitterItem::Bullet(BulletParams {
                style: weapon.bullet_style,
                weapon: weapon.kind,
                position: self.skeleton.pos(16),
                velocity,
                timeout: weapon.timeout as i16,
                hit_multiply: weapon.hit_multiply,
                team: self.team,
                sprite: weapon.bullet_sprite,
                seed: None,
                must_create: false,
                net: true,
                owner_immune: false,
            }));

            self.weapons[self.active_weapon] = config.weapons.get(WeaponKind::NoWeapon);
            self.body_apply_animation(Anim::Stand, 1);
        }

        // Punch!
        if !self.dead_meat
            && (self.body_animation.id == Anim::Punch)
            && (self.body_animation.frame == 11)
            && !self
                .primary_weapon()
                .is_any(&[WeaponKind::LAW, WeaponKind::M79])
        {
            let weapon = *self.primary_weapon();
            self.melee_attack(&weapon, emitter);
            if weapon.kind == WeaponKind::Knife {
                self.play(Sfx::Slash);
            }
            self.body_animation.frame += 1;
        }

        // Buttstock!
        if !self.dead_meat
            && (self.body_animation.id == Anim::Melee)
            && (self.body_animation.frame == 12)
        {
            let fist = config.weapons.get(WeaponKind::NoWeapon);
            self.melee_attack(&fist, emitter);
            self.play(Sfx::Slash);
        }

        if self.body_animation.id == Anim::Melee && self.body_animation.frame > 20 {
            self.body_apply_animation(Anim::Stand, 1);
        }

        // Shotgun luska
        if self.body_animation.id == Anim::Shotgun && self.body_animation.frame == 24 {
            self.shell(0.025, 51);
            self.body_animation.frame += 1;
        }

        // M79 luska
        let weapon = self.weapons[self.active_weapon];
        if weapon.kind == WeaponKind::M79 && weapon.reload_time_count == weapon.clip_out_time {
            self.shell(0.08, 52);
        }
        let weapon = &mut self.weapons[self.active_weapon];
        if weapon.kind == WeaponKind::M79 && weapon.reload_time_count == weapon.clip_out_time {
            weapon.reload_time_count = weapon.reload_time_count.saturating_sub(1);
        }

        // Prone
        if self.control.prone
            && (self.legs_animation.id != Anim::GetUp)
            && (self.legs_animation.id != Anim::Prone)
            && (self.legs_animation.id != Anim::ProneMove)
        {
            self.play(Sfx::Goprone);
            self.legs_apply_animation(Anim::Prone, 1);
            if (self.body_animation.id != Anim::Reload)
                && (self.body_animation.id != Anim::Change)
                && (self.body_animation.id != Anim::ThrowWeapon)
            {
                self.body_apply_animation(Anim::Prone, 1);
            }
            self.old_direction = self.direction;
            self.control.prone = false;
        }

        // Get up
        if self.position == POS_PRONE
            && (self.control.prone || (self.direction != self.old_direction))
            && (((self.legs_animation.id == Anim::Prone) && (self.legs_animation.frame > 23))
                || (self.legs_animation.id == Anim::ProneMove))
        {
            if self.legs_animation.id != Anim::GetUp {
                self.legs_animation = self.anims.state(Anim::GetUp);
                self.legs_animation.frame = 9;
                self.control.prone = false;
                self.play(Sfx::Standup);
            }
            if (self.body_animation.id != Anim::Reload)
                && (self.body_animation.id != Anim::Change)
                && (self.body_animation.id != Anim::ThrowWeapon)
            {
                self.body_apply_animation(Anim::GetUp, 9);
            }
        }

        let mut unprone = false;
        // Immediately switch from unprone to jump/sidejump, because the end of the unprone
        // animation can be seen as the "wind up" for the jump
        if (self.legs_animation.id == Anim::GetUp)
            && (self.legs_animation.frame > 23 - (4 - 1))
            && self.on_ground
            && self.control.up
            && (cright || cleft)
        {
            // Set sidejump frame 1 to 4 depending on which unprone frame we're in
            let id = self.legs_animation.frame - (23 - (4 - 1));
            self.legs_apply_animation(Anim::JumpSide, id);
            unprone = true;
        } else if (self.legs_animation.id == Anim::GetUp)
            && (self.legs_animation.frame > 23 - (4 - 1))
            && self.on_ground
            && self.control.up
            && !(cright || cleft)
        {
            // Set jump frame 6 to 9 depending on which unprone frame we're in
            let id = self.legs_animation.frame - (23 - (9 - 1));
            self.legs_apply_animation(Anim::Jump, id);
            unprone = true;
        } else if (self.legs_animation.id == Anim::GetUp) && (self.legs_animation.frame > 23) {
            if cright || cleft {
                if (self.direction == 1) ^ cleft {
                    self.legs_apply_animation(Anim::Run, 1);
                } else {
                    self.legs_apply_animation(Anim::RunBack, 1);
                }
            } else if !self.on_ground && self.control.up {
                self.legs_apply_animation(Anim::Run, 1);
            } else {
                self.legs_apply_animation(Anim::Stand, 1);
            }
            unprone = true;
        }

        if unprone {
            self.position = POS_STAND;

            if (self.body_animation.id != Anim::Reload)
                && (self.body_animation.id != Anim::Change)
                && (self.body_animation.id != Anim::ThrowWeapon)
            {
                self.body_apply_animation(Anim::Stand, 1);
            }
        }

        // stationary gun overheat cools down
        if !self.control.fire {
            if self.use_time > M2GUN_OVERHEAT + 1 {
                self.use_time = 0;
            }
            if self.use_time > 0 && tick.is_multiple_of(8) {
                self.use_time -= 1;
            }
        }

        // Fondle Barrett?!
        if self.primary_weapon().kind == WeaponKind::Barrett
            && self.primary_weapon().fire_interval_count > 0
            && self
                .body_animation
                .is_any(&[Anim::Stand, Anim::Crouch, Anim::Prone])
        {
            self.body_apply_animation(Anim::Barret, 1);
        }

        if self.stat.is_none() {
            if ((self.body_animation.id == Anim::Stand)
                && (self.legs_animation.id == Anim::Stand)
                && !self.dead_meat
                && (self.idle_time > 0))
                || (self.idle_time > DEFAULT_IDLETIME)
            {
                // a network client only counts down while an idle animation is picked
                if !config.client || self.idle_random >= 0 {
                    self.idle_time -= 1;
                }
            } else {
                self.idle_time = DEFAULT_IDLETIME;
            }

            // the server picks the idle animation (and tells: `ServerIdleAnimation`)
            if !config.client && self.idle_time == 1 && self.idle_random < 0 {
                self.idle_time = 0;
                self.idle_random = rng.below(4) as i8;
            }
        }

        self.idle_animations(map, config, rng, emitter);

        {
            // *CHEAT*
            if self.legs_animation.speed > 1 {
                if (self.legs_animation.id == Anim::Jump)
                    || (self.legs_animation.id == Anim::JumpSide)
                    || (self.legs_animation.id == Anim::Roll)
                    || (self.legs_animation.id == Anim::RollBack)
                    || (self.legs_animation.id == Anim::Prone)
                    || (self.legs_animation.id == Anim::Run)
                    || (self.legs_animation.id == Anim::RunBack)
                {
                    self.particle.velocity.x /= self.legs_animation.speed as f32;
                    self.particle.velocity.y /= self.legs_animation.speed as f32;
                }

                if self.legs_animation.speed > 2
                    && ((self.legs_animation.id == Anim::ProneMove)
                        || (self.legs_animation.id == Anim::CrouchRun))
                {
                    self.particle.velocity.x /= self.legs_animation.speed as f32;
                    self.particle.velocity.y /= self.legs_animation.speed as f32;
                }
            }

            // stat gun deactivate if needed (the world lets go of the thing)
            if (self.control.up || self.control.jets)
                && let Some(gun) = self.stat.take()
            {
                self.stat_release = Some(gun);
            }

            self.sniper_view(tick);

            // Check if near collider
            if tick.is_multiple_of(10) {
                self.collider_distance = 255; // not near

                let look = vec2normalize(self.skeleton.pos(15) - self.skeleton.pos(16)) * 8.0;
                let start = self.skeleton.pos(12) - vec2(0.0, 5.0);
                let lookpoint = start + look;

                for collider in map.colliders.iter().filter(|c| c.active) {
                    let d = vec2length(lookpoint - vec2(collider.x, collider.y));
                    if d < collider.radius {
                        self.collider_distance = d.min(253.0).round_ties_even() as u8;
                        break;
                    }
                }
                // (raising the weapon above crouching teammates is client-side)
            }

            // TODO if targetmode > freecontrols
            // End any ongoing idle animations if a key is pressed
            if ((self.body_animation.id == Anim::Cigar)
                || (self.body_animation.id == Anim::Match)
                || (self.body_animation.id == Anim::Smoke)
                || (self.body_animation.id == Anim::Wipe)
                || (self.body_animation.id == Anim::Groin))
                && (cleft
                    || cright
                    || self.control.up
                    || self.control.down
                    || self.control.fire
                    || self.control.jets
                    || self.control.throw_nade
                    || self.control.change_weapon
                    || self.control.throw_weapon
                    || self.control.reload
                    || self.control.prone)
            {
                self.body_animation.frame = self.body_animation.num_frames();
            }

            // make anims out of controls
            // rolling
            if (self.body_animation.id != Anim::TakeOff)
                && (self.body_animation.id != Anim::Piss)
                && (self.body_animation.id != Anim::Mercy)
                && (self.body_animation.id != Anim::Mercy2)
                && (self.body_animation.id != Anim::Victory)
                && (self.body_animation.id != Anim::Own)
            {
                if (self.body_animation.id == Anim::Roll)
                    || (self.body_animation.id == Anim::RollBack)
                {
                    if self.legs_animation.id == Anim::Roll {
                        if self.on_ground {
                            self.particle.force.x = fpc(f64::from(self.direction) * ROLLSPEED);
                        } else {
                            self.particle.force.x = fpc(f64::from(self.direction) * 2.0 * FLYSPEED);
                        }
                    } else if self.legs_animation.id == Anim::RollBack {
                        if self.on_ground {
                            self.particle.force.x = fpc(-f64::from(self.direction) * ROLLSPEED);
                        } else {
                            self.particle.force.x =
                                fpc(-f64::from(self.direction) * 2.0 * FLYSPEED);
                        }
                        // if appropriate frames to move
                        if (self.legs_animation.frame > 1)
                            && (self.legs_animation.frame < 8)
                            && self.control.up
                        {
                            self.particle.force.y =
                                fpc(ext(self.particle.force.y) - JUMPDIRSPEED * 1.5);
                            self.particle.force.x *= 0.5;
                            self.particle.velocity.x = fpc(ext(self.particle.velocity.x) * 0.8);
                        }
                    }
                // downright
                } else if (cright) && (self.control.down) {
                    if self.on_ground {
                        // roll to the side
                        if (self.legs_animation.id == Anim::Run)
                            || (self.legs_animation.id == Anim::RunBack)
                            || (self.legs_animation.id == Anim::Fall)
                            || (self.legs_animation.id == Anim::ProneMove)
                            || ((self.legs_animation.id == Anim::Prone)
                                && (self.legs_animation.frame >= 24))
                        {
                            if (self.legs_animation.id == Anim::ProneMove)
                                || ((self.legs_animation.id == Anim::Prone)
                                    && (self.legs_animation.frame
                                        == self.legs_animation.num_frames()))
                            {
                                self.control.prone = false;
                                self.position = POS_STAND;
                            }

                            if !self.legs_animation.is_any(&[Anim::RollBack, Anim::Roll]) {
                                self.play(Sfx::Roll);
                            }
                            self.pause_sound(Channel::Reload, true);

                            if self.direction == 1 {
                                self.body_apply_animation(Anim::Roll, 1);
                                self.legs_animation = self.anims.state(Anim::Roll);
                                self.legs_animation.frame = 1;
                            } else {
                                self.body_apply_animation(Anim::RollBack, 1);
                                self.legs_animation = self.anims.state(Anim::RollBack);
                                self.legs_animation.frame = 1;
                            }
                        } else {
                            if self.direction == 1 {
                                self.legs_apply_animation(Anim::CrouchRun, 1);
                            } else {
                                self.legs_apply_animation(Anim::CrouchRunBack, 1);
                            }
                        }

                        if (self.legs_animation.id == Anim::CrouchRun)
                            || (self.legs_animation.id == Anim::CrouchRunBack)
                        {
                            self.particle.force.x = fpc(CROUCHRUNSPEED);
                        } else if (self.legs_animation.id == Anim::Roll)
                            || (self.legs_animation.id == Anim::RollBack)
                        {
                            self.particle.force.x = fpc(2.0 * CROUCHRUNSPEED);
                        }
                    }
                // downleft
                } else if cleft && self.control.down {
                    if self.on_ground {
                        // roll to the side
                        if (self.legs_animation.id == Anim::Run)
                            || (self.legs_animation.id == Anim::RunBack)
                            || (self.legs_animation.id == Anim::Fall)
                            || (self.legs_animation.id == Anim::ProneMove)
                            || ((self.legs_animation.id == Anim::Prone)
                                && (self.legs_animation.frame >= 24))
                        {
                            if (self.legs_animation.id == Anim::ProneMove)
                                || ((self.legs_animation.id == Anim::Prone)
                                    && (self.legs_animation.frame
                                        == self.legs_animation.num_frames()))
                            {
                                self.control.prone = false;
                                self.position = POS_STAND;
                            }

                            if !self.legs_animation.is_any(&[Anim::RollBack, Anim::Roll]) {
                                self.play(Sfx::Roll);
                            }
                            self.pause_sound(Channel::Reload, true);

                            if self.direction == 1 {
                                self.body_apply_animation(Anim::RollBack, 1);
                                self.legs_animation = self.anims.state(Anim::RollBack);
                                self.legs_animation.frame = 1;
                            } else {
                                self.body_apply_animation(Anim::Roll, 1);
                                self.legs_animation = self.anims.state(Anim::Roll);
                                self.legs_animation.frame = 1;
                            }
                        } else {
                            if self.direction == 1 {
                                self.legs_apply_animation(Anim::CrouchRunBack, 1);
                            } else {
                                self.legs_apply_animation(Anim::CrouchRun, 1);
                            }
                        }

                        if (self.legs_animation.id == Anim::CrouchRun)
                            || (self.legs_animation.id == Anim::CrouchRunBack)
                        {
                            self.particle.force.x = fpc(-CROUCHRUNSPEED);
                        } else if (self.legs_animation.id == Anim::Roll)
                            || (self.legs_animation.id == Anim::RollBack)
                        {
                            self.particle.force.x = fpc(2.0 * -CROUCHRUNSPEED);
                        }
                    }
                // Proning
                } else if (self.legs_animation.id == Anim::Prone)
                    || (self.legs_animation.id == Anim::ProneMove)
                    || ((self.legs_animation.id == Anim::GetUp)
                        && (self.body_animation.id != Anim::Throw)
                        && (self.body_animation.id != Anim::Punch))
                {
                    if self.on_ground
                        && (((self.legs_animation.id == Anim::Prone)
                            && (self.legs_animation.frame > 25))
                            || (self.legs_animation.id == Anim::ProneMove))
                    {
                        if cleft || cright {
                            if (self.legs_animation.frame < 4) || (self.legs_animation.frame > 14) {
                                self.particle.force.x =
                                    fpc(if cleft { -PRONESPEED } else { PRONESPEED })
                            }

                            self.legs_apply_animation(Anim::ProneMove, 1);

                            if (self.body_animation.id != Anim::ClipIn)
                                && (self.body_animation.id != Anim::ClipOut)
                                && (self.body_animation.id != Anim::SlideBack)
                                && (self.body_animation.id != Anim::Reload)
                                && (self.body_animation.id != Anim::Change)
                                && (self.body_animation.id != Anim::Throw)
                                && (self.body_animation.id != Anim::ThrowWeapon)
                            {
                                self.body_apply_animation(Anim::ProneMove, 1);
                            }

                            if self.legs_animation.id != Anim::ProneMove {
                                self.legs_animation = self.anims.state(Anim::ProneMove);
                            }
                        } else {
                            if self.legs_animation.id != Anim::Prone {
                                self.legs_animation = self.anims.state(Anim::Prone);
                            }
                            self.legs_animation.frame = 26;
                        }
                    }
                } else if cright && self.control.up {
                    if self.on_ground {
                        if (self.legs_animation.id == Anim::Run)
                            || (self.legs_animation.id == Anim::RunBack)
                            || (self.legs_animation.id == Anim::Stand)
                            || (self.legs_animation.id == Anim::Crouch)
                            || (self.legs_animation.id == Anim::CrouchRun)
                            || (self.legs_animation.id == Anim::CrouchRunBack)
                        {
                            self.legs_apply_animation(Anim::JumpSide, 1);
                            self.play(Sfx::Jump);
                        }

                        if self.legs_animation.frame == self.legs_animation.num_frames() {
                            self.legs_apply_animation(Anim::Run, 1);
                        }
                    } else if (self.legs_animation.id == Anim::Roll)
                        || (self.legs_animation.id == Anim::RollBack)
                    {
                        if self.direction == 1 {
                            self.legs_apply_animation(Anim::Run, 1);
                        } else {
                            self.legs_apply_animation(Anim::RunBack, 1);
                        }
                    }
                    if self.legs_animation.id == Anim::Jump && self.legs_animation.frame < 10 {
                        self.legs_apply_animation(Anim::JumpSide, 1);
                    }

                    if self.legs_animation.id == Anim::JumpSide
                        && (self.legs_animation.frame > 3)
                        && (self.legs_animation.frame < 11)
                    {
                        self.particle.force.x = fpc(JUMPDIRSPEED);
                        self.particle.force.y = fpc(-JUMPDIRSPEED / 1.2);
                    }
                } else if cleft && self.control.up {
                    if self.on_ground {
                        if (self.legs_animation.id == Anim::Run)
                            || (self.legs_animation.id == Anim::RunBack)
                            || (self.legs_animation.id == Anim::Stand)
                            || (self.legs_animation.id == Anim::Crouch)
                            || (self.legs_animation.id == Anim::CrouchRun)
                            || (self.legs_animation.id == Anim::CrouchRunBack)
                        {
                            self.legs_apply_animation(Anim::JumpSide, 1);
                            self.play(Sfx::Jump);
                        }

                        if self.legs_animation.frame == self.legs_animation.num_frames() {
                            self.legs_apply_animation(Anim::Run, 1);
                        }
                    } else if (self.legs_animation.id == Anim::Roll)
                        || (self.legs_animation.id == Anim::RollBack)
                    {
                        if self.direction == -1 {
                            self.legs_apply_animation(Anim::Run, 1);
                        } else {
                            self.legs_apply_animation(Anim::RunBack, 1);
                        }
                    }

                    if self.legs_animation.id == Anim::Jump && self.legs_animation.frame < 10 {
                        self.legs_apply_animation(Anim::JumpSide, 1);
                    }

                    if self.legs_animation.id == Anim::JumpSide
                        && (self.legs_animation.frame > 3)
                        && (self.legs_animation.frame < 11)
                    {
                        self.particle.force.x = fpc(-JUMPDIRSPEED);
                        self.particle.force.y = fpc(-JUMPDIRSPEED / 1.2);
                    }
                } else if self.control.up {
                    if self.on_ground {
                        if self.legs_animation.id != Anim::Jump {
                            self.legs_apply_animation(Anim::Jump, 1);
                            self.play(Sfx::Jump);
                        }
                        if self.legs_animation.frame == self.legs_animation.num_frames() {
                            self.legs_apply_animation(Anim::Stand, 1);
                        }
                    }
                    if self.legs_animation.id == Anim::Jump {
                        if (self.legs_animation.frame > 8) && (self.legs_animation.frame < 15) {
                            self.particle.force.y = fpc(-JUMPSPEED);
                        }
                        if self.legs_animation.frame == self.legs_animation.num_frames() {
                            self.legs_apply_animation(Anim::Fall, 1);
                        }
                    }
                } else if self.control.down {
                    if self.on_ground {
                        if !self.legs_animation.is_any(&[
                            Anim::CrouchRun,
                            Anim::CrouchRunBack,
                            Anim::Crouch,
                        ]) {
                            self.play(Sfx::Crouch);
                        }
                        self.legs_apply_animation(Anim::Crouch, 1);
                    }
                } else if cright {
                    if !self.para {
                        if self.direction == 1 {
                            self.legs_apply_animation(Anim::Run, 1);
                        } else {
                            self.legs_apply_animation(Anim::RunBack, 1);
                        }
                    } else if let Some(t) = self.holded_thing {
                        self.parachute_bend = Some((t, 1));
                    }

                    if self.on_ground {
                        self.particle.force.x = fpc(RUNSPEED);
                        self.particle.force.y = fpc(-RUNSPEEDUP);
                    } else {
                        self.particle.force.x = fpc(FLYSPEED);
                    }
                } else if cleft {
                    if !self.para {
                        if self.direction == -1 {
                            self.legs_apply_animation(Anim::Run, 1);
                        } else {
                            self.legs_apply_animation(Anim::RunBack, 1);
                        }
                    } else if let Some(t) = self.holded_thing {
                        self.parachute_bend = Some((t, -1));
                    }

                    if self.on_ground {
                        self.particle.force.x = fpc(-RUNSPEED);
                        self.particle.force.y = fpc(-RUNSPEEDUP);
                    } else {
                        self.particle.force.x = fpc(-FLYSPEED);
                    }
                } else {
                    if self.on_ground {
                        if !self.dead_meat && self.legs_animation.id != Anim::Stand {
                            self.play(Sfx::Stop);
                        }
                        self.legs_apply_animation(Anim::Stand, 1);
                    } else {
                        self.legs_apply_animation(Anim::Fall, 1);
                    }
                }
            }
            // Body animations

            // reloading
            let weapon = *self.primary_weapon();
            if weapon.reload_time_count == weapon.clip_out_time
                && !self.body_animation.is_any(&[
                    Anim::Reload,
                    Anim::ReloadBow,
                    Anim::Roll,
                    Anim::RollBack,
                ])
            {
                self.body_apply_animation(Anim::ClipIn, 1);
            }
            if weapon.reload_time_count == weapon.clip_in_time {
                self.body_apply_animation(Anim::SlideBack, 1);
            }

            if (self.legs_animation.id == Anim::Roll) && (self.body_animation.id != Anim::Roll) {
                self.body_apply_animation(Anim::Roll, 1)
            }
            if (self.body_animation.id == Anim::Roll) && (self.legs_animation.id != Anim::Roll) {
                self.legs_apply_animation(Anim::Roll, 1)
            }
            if (self.legs_animation.id == Anim::RollBack)
                && (self.body_animation.id != Anim::RollBack)
            {
                self.body_apply_animation(Anim::RollBack, 1)
            }
            if (self.body_animation.id == Anim::RollBack)
                && (self.legs_animation.id != Anim::RollBack)
            {
                self.legs_apply_animation(Anim::RollBack, 1)
            }

            if ((self.body_animation.id == Anim::Roll)
                || (self.body_animation.id == Anim::RollBack))
                && self.legs_animation.frame != self.body_animation.frame
            {
                if self.legs_animation.frame > self.body_animation.frame {
                    self.body_animation.frame = self.legs_animation.frame;
                } else {
                    self.legs_animation.frame = self.body_animation.frame;
                }
            }

            // Gracefully end a roll animation
            if ((self.body_animation.id == Anim::Roll)
                || (self.body_animation.id == Anim::RollBack))
                && (self.body_animation.frame == self.body_animation.num_frames())
            {
                // Was probably a roll
                if self.on_ground {
                    if self.control.down {
                        if cleft || cright {
                            if self.body_animation.id == Anim::Roll {
                                self.legs_apply_animation(Anim::CrouchRun, 1);
                            } else {
                                self.legs_apply_animation(Anim::CrouchRunBack, 1);
                            }
                        } else {
                            self.legs_apply_animation(Anim::Crouch, 15);
                        }
                    }
                // Was probably a backflip
                } else if (self.body_animation.id == Anim::RollBack) && self.control.up {
                    if cleft || cright {
                        // Run back or forward depending on facing direction and direction key pressed
                        if (self.direction == 1) ^ (cleft) {
                            self.legs_apply_animation(Anim::Run, 1);
                        } else {
                            self.legs_apply_animation(Anim::RunBack, 1);
                        }
                    } else {
                        self.legs_apply_animation(Anim::Fall, 1);
                    }
                // Was probably a roll (that ended mid-air)
                } else if self.control.down {
                    if cleft || cright {
                        if self.body_animation.id == Anim::Roll {
                            self.legs_apply_animation(Anim::CrouchRun, 1);
                        } else {
                            self.legs_apply_animation(Anim::CrouchRunBack, 1);
                        }
                    } else {
                        self.legs_apply_animation(Anim::Crouch, 15);
                    }
                }
                self.body_apply_animation(Anim::Stand, 1);
            }

            if self.primary_weapon().ammo_count > 0
                && ((!self.control.throw_nade
                    && (self.body_animation.id != Anim::Recoil)
                    && (self.body_animation.id != Anim::SmallRecoil)
                    && (self.body_animation.id != Anim::AimRecoil)
                    && (self.body_animation.id != Anim::HandsUpRecoil)
                    && (self.body_animation.id != Anim::Shotgun)
                    && (self.body_animation.id != Anim::Barret)
                    && (self.body_animation.id != Anim::Change)
                    && (self.body_animation.id != Anim::ThrowWeapon)
                    && (self.body_animation.id != Anim::WeaponNone)
                    && (self.body_animation.id != Anim::Punch)
                    && (self.body_animation.id != Anim::Roll)
                    && (self.body_animation.id != Anim::RollBack)
                    && (self.body_animation.id != Anim::ReloadBow)
                    && (self.body_animation.id != Anim::Cigar)
                    && (self.body_animation.id != Anim::Match)
                    && (self.body_animation.id != Anim::Smoke)
                    && (self.body_animation.id != Anim::Wipe)
                    && (self.body_animation.id != Anim::TakeOff)
                    && (self.body_animation.id != Anim::Groin)
                    && (self.body_animation.id != Anim::Piss)
                    && (self.body_animation.id != Anim::Mercy)
                    && (self.body_animation.id != Anim::Mercy2)
                    && (self.body_animation.id != Anim::Victory)
                    && (self.body_animation.id != Anim::Own)
                    && (self.body_animation.id != Anim::Reload)
                    && (self.body_animation.id != Anim::Prone)
                    && (self.body_animation.id != Anim::GetUp)
                    && (self.body_animation.id != Anim::ProneMove)
                    && (self.body_animation.id != Anim::Melee))
                    || ((self.body_animation.frame == self.body_animation.num_frames())
                        && (self.body_animation.id != Anim::Prone))
                    || (self.primary_weapon().fire_interval_count == 0
                        && self.body_animation.id == Anim::Barret))
            {
                if self.position != POS_PRONE {
                    if self.position == POS_STAND {
                        self.body_apply_animation(Anim::Stand, 1);
                    }

                    if self.position == POS_CROUCH {
                        if self.collider_distance < 255 {
                            if self.body_animation.id == Anim::HandsUpRecoil {
                                self.body_apply_animation(Anim::HandsUpAim, 11);
                            } else {
                                self.body_apply_animation(Anim::HandsUpAim, 1);
                            }
                        } else {
                            if self.body_animation.id == Anim::AimRecoil {
                                self.body_apply_animation(Anim::Aim, 6);
                            } else {
                                self.body_apply_animation(Anim::Aim, 1);
                            }
                        }
                    }
                } else {
                    self.body_apply_animation(Anim::Prone, 26);
                }
            }

            if (self.legs_animation.id == Anim::Crouch)
                || (self.legs_animation.id == Anim::CrouchRun)
                || (self.legs_animation.id == Anim::CrouchRunBack)
            {
                self.position = POS_CROUCH;
            } else {
                self.position = POS_STAND;
            }
            if (self.legs_animation.id == Anim::Prone)
                || (self.legs_animation.id == Anim::ProneMove)
            {
                self.position = POS_PRONE;
            }
        }
    }
}

impl Control {
    /// `TSprite.FreeControls`: releases the buttons, the cursor stays.
    pub fn free_controls(&mut self) {
        *self = Control {
            mouse_aim_x: self.mouse_aim_x,
            mouse_aim_y: self.mouse_aim_y,
            mouse_dist: self.mouse_dist,
            was_running_left: self.was_running_left,
            was_jumping: self.was_jumping,
            was_throwing_weapon: self.was_throwing_weapon,
            was_changing_weapon: self.was_changing_weapon,
            was_throwing_grenade: self.was_throwing_grenade,
            was_reloading_weapon: self.was_reloading_weapon,
            ..Default::default()
        };
    }
}

impl Soldier {
    /// Idle animations the server picks (`IdleRandom` 0-3); the others are started by
    /// player commands.
    fn idle_animations(
        &mut self,
        map: &MapFile,
        config: &WorldConfig,
        rng: &mut PascalRandom,
        emitter: &mut Vec<EmitterItem>,
    ) {
        match self.idle_random {
            // stuff
            0 => {
                if self.idle_time == 0 {
                    self.body_apply_animation(Anim::Smoke, 1);
                    self.idle_started = Some(0);
                    self.idle_time = DEFAULT_IDLETIME;
                }

                if (self.body_animation.id == Anim::Smoke) && (self.body_animation.frame == 17) {
                    self.play(Sfx::Stuff);
                    self.body_animation.frame += 1;
                }

                if !self.dead_meat
                    && (self.idle_time == 1)
                    && (self.body_animation.id != Anim::Smoke)
                    && (self.legs_animation.id == Anim::Stand)
                {
                    let b = self.hands_aim_direction() * 2.0;
                    self.spark(self.skeleton.pos(12), b, 32, 245);
                    self.play(Sfx::Spit);
                    self.idle_time = DEFAULT_IDLETIME;
                    self.idle_random = -1;
                }
            }
            // cigar
            1 => {
                if self.dead_meat {
                    return;
                }
                let body = self.body_animation.id;
                if self.idle_time == 0 {
                    if self.has_cigar == 0 {
                        if body == Anim::Stand {
                            self.body_apply_animation(Anim::Cigar, 1);
                            self.idle_started = Some(1);
                            self.idle_time = DEFAULT_IDLETIME;
                        }
                    } else if self.has_cigar == 5 {
                        if body != Anim::Smoke && body != Anim::Cigar {
                            // interrupted between lighting and smoking: start over
                            self.has_cigar = 0;
                            self.body_apply_animation(Anim::Cigar, 1);
                            self.idle_time = DEFAULT_IDLETIME;
                        }
                    } else if self.has_cigar == 10 && body != Anim::Smoke {
                        self.body_apply_animation(Anim::Smoke, 1);
                        self.idle_time = DEFAULT_IDLETIME;
                    }
                }

                let cigar_frame = |s: &Soldier, frame| {
                    s.body_animation.id == Anim::Cigar && s.body_animation.frame == frame
                };
                if cigar_frame(self, 37) && self.has_cigar == 5 {
                    self.body_apply_animation(Anim::Stand, 1);
                    self.body_apply_animation(Anim::Cigar, 1);
                }
                if cigar_frame(self, 9) && self.has_cigar == 5 {
                    self.play(Sfx::Match);
                    self.body_animation.frame += 1;
                }
                if cigar_frame(self, 26) {
                    if self.has_cigar == 5 {
                        self.has_cigar = 10;
                        self.puff();
                        self.play(Sfx::Smoke);
                        let dir = f32::from(self.direction);
                        self.spark(self.skeleton.pos(15), vec2(dir / 2.0, 0.15), 33, 245);
                        self.body_animation.frame += 1;
                        self.idle_time = LONGER_IDLETIME;
                    } else if self.has_cigar == 0 {
                        self.has_cigar = 5;
                        self.body_animation.frame += 1;
                    }
                }

                if self.body_animation.id == Anim::Smoke
                    && (self.body_animation.frame == 17 || self.body_animation.frame == 37)
                {
                    self.puff();
                    self.play(Sfx::Smoke);
                    self.body_animation.frame += 1;
                }
                if self.body_animation.id == Anim::Smoke && self.body_animation.frame == 38 {
                    self.has_cigar = 0;
                    let dir = f32::from(self.direction);
                    self.spark(self.skeleton.pos(15), vec2(dir / 1.5, 0.1), 34, 245);
                    self.body_animation.frame += 1;
                    self.idle_time = DEFAULT_IDLETIME;
                    self.idle_random = -1;
                }
            }
            // wipe, eggs
            2 | 3 if self.idle_time == 0 => {
                let anim = if self.idle_random == 2 {
                    Anim::Wipe
                } else {
                    Anim::Groin
                };
                self.body_apply_animation(anim, 1);
                self.idle_started = Some(self.idle_random);
                self.idle_time = DEFAULT_IDLETIME;
                self.idle_random = -1;
            }
            // take off the helmet (or put it back on)
            4 => {
                if self
                    .primary_weapon()
                    .is_any(&[WeaponKind::Bow, WeaponKind::FlameBow])
                {
                    return;
                }
                if self.idle_time == 0 {
                    if self.wear_helmet == 1 {
                        self.body_apply_animation(Anim::TakeOff, 1);
                    }
                    if self.wear_helmet == 2 {
                        self.body_apply_animation(Anim::TakeOff, 10);
                    }
                    self.idle_started = Some(4);
                    self.idle_time = DEFAULT_IDLETIME;
                }

                let takeoff = |s: &Soldier, frame| {
                    s.body_animation.id == Anim::TakeOff && s.body_animation.frame == frame
                };
                if self.wear_helmet == 1 {
                    if takeoff(self, 15) {
                        self.wear_helmet = 2;
                        self.body_animation.frame += 1;
                    }
                } else if self.wear_helmet == 2 {
                    if takeoff(self, 22) {
                        self.body_apply_animation(Anim::Stand, 1);
                        self.idle_random = -1;
                    }
                    if takeoff(self, 15) {
                        self.wear_helmet = 1;
                        self.body_animation.frame += 1;
                    }
                }
            }
            // victory
            5 if self.idle_time == 0 => {
                self.body_apply_animation(Anim::Victory, 1);
                self.idle_started = Some(5);
                self.idle_time = DEFAULT_IDLETIME;
                self.idle_random = -1;
                self.play(Sfx::Roar);
            }
            // piss... (the sparks are client effects, but they draw from Random)
            6 => {
                if self.idle_time == 0 {
                    self.body_apply_animation(Anim::Piss, 1);
                    self.idle_started = Some(6);
                    self.idle_time = DEFAULT_IDLETIME;
                    self.play(Sfx::Piss);
                }
                if self.body_animation.id == Anim::Piss {
                    let frame = self.body_animation.frame;
                    let stream = if frame > 8 && frame < 22 {
                        (rng.below(2) == 0).then_some((1.3, 165))
                    } else if frame > 21 && frame < 34 {
                        (rng.below(3) == 0).then_some((1.9, 120))
                    } else if frame > 33 && frame < 35 {
                        (rng.below(4) == 0).then_some((1.3, 120))
                    } else {
                        None
                    };
                    if let Some((speed, life)) = stream {
                        let a = self.skeleton.pos(20);
                        let look = vec2(
                            self.control.mouse_aim_x as f32,
                            self.control.mouse_aim_y as f32,
                        );
                        let b = vec2normalize(a - look) * -speed;
                        self.spark(a, b, 57, life);
                    }
                    if frame == 37 {
                        self.idle_random = -1;
                    }
                }
            }
            // mercy: shoot yourself
            7 => {
                if self.idle_time == 0 {
                    if self.can_mercy {
                        let anim = if self.primary_weapon().is_any(&[
                            WeaponKind::M79,
                            WeaponKind::Minimi,
                            WeaponKind::Spas12,
                            WeaponKind::LAW,
                            WeaponKind::Chainsaw,
                            WeaponKind::Barrett,
                            WeaponKind::Minigun,
                        ]) {
                            Anim::Mercy2
                        } else {
                            Anim::Mercy
                        };
                        self.body_apply_animation(anim, 1);
                        self.legs_apply_animation(anim, 1);
                        self.play(Sfx::Mercy);
                        if self.primary_weapon().kind == WeaponKind::Minigun {
                            self.play(Sfx::MinigunStart);
                        }
                        self.idle_started = Some(7);
                        self.idle_time = DEFAULT_IDLETIME;
                        self.can_mercy = false;
                    } else {
                        self.idle_random = -1;
                        self.can_mercy = true;
                    }
                }

                if self.body_animation.is_any(&[Anim::Mercy, Anim::Mercy2])
                    && self.body_animation.frame == 20
                {
                    self.fire(map, config, rng, emitter);
                    let sound = match self.primary_weapon().kind {
                        WeaponKind::Knife => Some(Sfx::Slash),
                        WeaponKind::Chainsaw => Some(Sfx::ChainsawR),
                        WeaponKind::NoWeapon => Some(Sfx::DeadHit),
                        _ => None,
                    };
                    if let Some(sfx) = sound {
                        self.play_on(sfx, Channel::Gattling);
                    }
                    // the client's own soldier then sends `kill`
                    if self.local_player {
                        self.mercy_kill = true;
                    }
                    self.body_animation.frame += 1;
                    self.idle_random = -1;
                }
            }
            // pwn!
            8 if self.idle_time == 0 => {
                self.body_apply_animation(Anim::Own, 1);
                self.legs_apply_animation(Anim::Own, 1);
                self.idle_started = Some(8);
                self.idle_time = DEFAULT_IDLETIME;
                self.idle_random = -1;
            }
            _ => {}
        }
    }

    /// The bullet of a punch, knife or chainsaw stab, or buttstock hit.
    fn melee_attack(&mut self, weapon: &Weapon, emitter: &mut Vec<EmitterItem>) {
        let dir = f32::from(self.direction);
        let hand = self.skeleton.pos(16);
        emitter.push(EmitterItem::Bullet(BulletParams {
            style: weapon.bullet_style,
            weapon: weapon.kind,
            position: vec2(hand.x + 2.0 * dir, hand.y + 3.0),
            velocity: vec2(dir * 0.1, 0.0),
            timeout: weapon.timeout as i16,
            hit_multiply: weapon.hit_multiply,
            team: self.team,
            sprite: weapon.bullet_sprite,
            seed: None,
            must_create: false,
            net: true,
            owner_immune: false,
        }));

        // the server takes the bullet from a bot's ammo (ServerBulletSnapshot)
        let bot = self.brain.is_some();
        let primary = &mut self.weapons[self.active_weapon];
        if bot && primary.ammo_count > 0 && pays_ammo(primary, weapon.bullet_style) {
            primary.ammo_count -= 1;
        }
    }

    /// Port of `TSprite.ThrowGrenade` (server side: no pin spark).
    fn throw_grenade(&mut self, map: &MapFile, emitter: &mut Vec<EmitterItem>) {
        // Start throw animation
        if !self.control.throw_nade {
            self.grenade_can_throw = true;
        }

        if self.grenade_can_throw
            && self.control.throw_nade
            && !self.body_animation.is_any(&[Anim::Roll, Anim::RollBack])
        {
            self.body_apply_animation(Anim::Throw, 1);
            self.pause_sound(Channel::Reload, true);
        }

        // pull the pin (the client's sound)
        if self.body_animation.id == Anim::Throw
            && self.body_animation.frame == 15
            && self.tertiary_weapon().ammo_count > 0
            && self.ceasefire_counter < 0
        {
            let speed = self.tertiary_weapon().speed;
            let b = self.hands_aim_direction() * (15.0 / speed) * 0.65 + self.particle.velocity;
            let a = self.skeleton.pos(15) + vec2(b.x * 3.0, b.y * 3.0 - 2.0);
            if map.collision_test(a, false).is_none() {
                let hands = self.hands_aim_direction();
                self.spark(a, vec2(hands.x * 0.5, hands.y + 0.4), 30, 255);
                self.play_sound(Sound::new(Sfx::GrenadePullout).at(a));
            }
        }

        if self.body_animation.id != Anim::Throw
            || (self.control.throw_nade && self.body_animation.frame != 36)
        {
            return;
        }

        // Grenade throw
        let frame = self.body_animation.frame;
        let nade = *self.tertiary_weapon();
        if frame > 14 && frame < 37 && nade.ammo_count > 0 && self.ceasefire_counter < 0 {
            let mut b = self.cursor_aim_direction();

            // Add a few degrees of arc to the throw. The arc approaches zero as you aim up or down
            let arc_size = pascal_sign(b.x) / 8.0 * (1.0 - b.y.abs());
            let arc_x = fpc((ext(b.y) * std::f64::consts::PI / 2.0).sin() * ext(arc_size));
            let arc_y = fpc((ext(b.x) * std::f64::consts::PI / 2.0).sin() * ext(arc_size));
            b.x += arc_x;
            b.y -= arc_y;
            b = vec2normalize(b);

            b *= frame as f32 / nade.speed;
            if frame < 24 {
                b *= 0.65;
            }

            b += self.particle.velocity * nade.inherited_velocity;
            let a = vec2(
                self.skeleton.pos(15).x + b.x * 3.0,
                self.skeleton.pos(15).y - 2.0 + b.y * 3.0,
            );
            let e = vec2(self.particle.pos.x, self.particle.pos.y - 12.0);
            let filter = RayCast {
                team: self.team,
                ..Default::default()
            };

            if map.collision_test(a, false).is_none() && map.ray_cast(e, a, 50.0, filter).is_none()
            {
                emitter.push(EmitterItem::Bullet(BulletParams {
                    style: nade.bullet_style,
                    weapon: nade.kind,
                    position: a,
                    velocity: b,
                    timeout: nade.timeout as i16,
                    hit_multiply: nade.hit_multiply,
                    team: self.team,
                    sprite: nade.bullet_sprite,
                    seed: None,
                    must_create: false,
                    net: true,
                    owner_immune: false,
                }));
                self.weapons[2].ammo_count -= 1;

                if self.local_player && nade.bink < 0 {
                    self.hit_spray_counter =
                        calculate_bink(self.hit_spray_counter, -nade.bink as u16);
                }
                self.play_sound(Sound::new(Sfx::GrenadeThrow).at(a));
            }
        }

        if self.control.throw_nade {
            self.grenade_can_throw = false;
        }

        let weapon = *self.primary_weapon();
        if weapon.ammo_count == 0 {
            if weapon.reload_time_count > weapon.clip_out_time {
                self.body_apply_animation(Anim::ClipOut, 1);
            }
            if weapon.reload_time_count < weapon.clip_out_time {
                self.body_apply_animation(Anim::ClipIn, 1);
            }
            if weapon.reload_time_count < weapon.clip_in_time && weapon.reload_time_count > 0 {
                self.body_apply_animation(Anim::SlideBack, 1);
            }
            self.pause_sound(Channel::Reload, false);
        }
    }
}
