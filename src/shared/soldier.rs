use glutin;
use shared::anims::*;
use shared::calc::*;
use shared::control::Control;
use shared::mapfile::PolyType;
use shared::parts;
use shared::parts::ParticleSystem;
use shared::state::MainState;
use shared::weapons::*;

const SLIDELIMIT: f32 = 0.2;
const GRAV: f32 = 0.06;
const SURFACECOEFX: f32 = 0.970;
const SURFACECOEFY: f32 = 0.970;
const CROUCHMOVESURFACECOEFX: f32 = 0.85;
const CROUCHMOVESURFACECOEFY: f32 = 0.97;
const STANDSURFACECOEFX: f32 = 0.00;
const STANDSURFACECOEFY: f32 = 0.00;

const POS_STAND: u8 = 1;
const POS_CROUCH: u8 = 2;
const POS_PRONE: u8 = 3;

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
    pub skeleton: parts::ParticleSystem,
    pub legs_animation: AnimState,
    pub body_animation: AnimState,
    pub control: Control,
    pub active_weapon: usize,
    pub weapons: [Weapon; 3],
    pub fired: u8,
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

    pub fn update_keys(&mut self, input: &glutin::KeyboardInput) {
        match input.state {
            glutin::ElementState::Pressed => match input.virtual_keycode {
                Some(glutin::VirtualKeyCode::A) => self.control.left = true,
                Some(glutin::VirtualKeyCode::D) => self.control.right = true,
                Some(glutin::VirtualKeyCode::W) => self.control.up = true,
                Some(glutin::VirtualKeyCode::S) => self.control.down = true,
                Some(glutin::VirtualKeyCode::Q) => self.control.change = true,
                Some(glutin::VirtualKeyCode::E) => self.control.throw = true,
                Some(glutin::VirtualKeyCode::X) => self.control.prone = true,
                _ => {}
            },
            glutin::ElementState::Released => match input.virtual_keycode {
                Some(glutin::VirtualKeyCode::A) => self.control.left = false,
                Some(glutin::VirtualKeyCode::D) => self.control.right = false,
                Some(glutin::VirtualKeyCode::W) => self.control.up = false,
                Some(glutin::VirtualKeyCode::S) => self.control.down = false,
                Some(glutin::VirtualKeyCode::Q) => self.control.change = false,
                Some(glutin::VirtualKeyCode::E) => self.control.throw = false,
                Some(glutin::VirtualKeyCode::X) => self.control.prone = false,
                _ => {}
            },
        }
    }

    pub fn update_mouse_button(&mut self, input: &(glutin::ElementState, glutin::MouseButton)) {
        let pressed = match input.0 {
            glutin::ElementState::Pressed => true,
            glutin::ElementState::Released => false,
        };
        match input.1 {
            glutin::MouseButton::Left => self.control.fire = pressed,
            glutin::MouseButton::Right => self.control.jets = pressed,
            _ => (),
        }
    }

    pub fn new(state: &mut MainState) -> Soldier {
        let control: Control = Default::default();
        let mut gostek = ParticleSystem::new();

        gostek.load_from_file(&String::from("gostek.po"), 4.50);
        gostek.timestep = 1.00;
        gostek.gravity = 1.06 * GRAV;
        gostek.v_damping = 0.9945;

        state.soldier_parts.create_part(
            vec2(
                state.map.spawnpoints[0].x as f32,
                state.map.spawnpoints[0].y as f32,
            ),
            vec2(0.0f32, 0.0f32),
            1.00,
            1,
        );

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
            has_cigar: 1,
            vest: 0.0,
            idle_time: 0,
            idle_random: 0,
            position: 0,
            on_fire: 0,
            collider_distance: 255,
            half_dead: false,
            skeleton: gostek,
            legs_animation: AnimState::new(Anim::Stand),
            body_animation: AnimState::new(Anim::Stand),
            control,
            active_weapon: 0,
            weapons: [
                Weapon::new(WeaponKind::DesertEagles, false),
                Weapon::new(WeaponKind::Chainsaw, false),
                Weapon::new(WeaponKind::FragGrenade, false),
            ],
            fired: 0,
        }
    }

    pub fn legs_apply_animation(&mut self, id: Anim, frame: usize) {
        if !self.legs_animation.is_any(&[Anim::Prone, Anim::ProneMove])
            && self.legs_animation.id != id
        {
            self.legs_animation = AnimState::new(id);
            self.legs_animation.frame = frame;
        }
    }

    pub fn body_apply_animation(&mut self, id: Anim, frame: usize) {
        if self.body_animation.id != id {
            self.body_animation = AnimState::new(id);
            self.body_animation.frame = frame;
        }
    }

    pub fn handle_special_polytypes(
        &mut self,
        state: &mut MainState,
        polytype: PolyType,
        _pos: Vec2,
    ) {
        if polytype == PolyType::Deadly || polytype == PolyType::BloodyDeadly
            || polytype == PolyType::Explosive
        {
            state.soldier_parts.pos[self.num] = vec2(
                state.map.spawnpoints[0].x as f32,
                state.map.spawnpoints[0].y as f32,
            );
        }
    }

    pub fn update(&mut self, state: &mut MainState) {
        let mut body_y = 0.0;
        let mut arm_s;

        self.control(state);

        self.skeleton.old_pos[21] = self.skeleton.pos[21];
        self.skeleton.old_pos[23] = self.skeleton.pos[23];
        self.skeleton.old_pos[25] = self.skeleton.pos[25];
        self.skeleton.pos[21] = self.skeleton.pos[9];
        self.skeleton.pos[23] = self.skeleton.pos[12];
        self.skeleton.pos[25] = self.skeleton.pos[5];

        if !self.dead_meat {
            self.skeleton.pos[21] += state.soldier_parts.velocity[self.num];
            self.skeleton.pos[23] += state.soldier_parts.velocity[self.num];
            self.skeleton.pos[25] += state.soldier_parts.velocity[self.num];
        }

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

        if self.control.mouse_aim_x as f32 >= state.soldier_parts.pos[self.num].x {
            self.direction = 1;
        } else {
            self.direction = -1;
        }

        for i in 1..21 {
            if self.skeleton.active[i] && !self.dead_meat {
                self.skeleton.old_pos[i] = self.skeleton.pos[i];

                // legs
                if !self.half_dead && ((i >= 1 && i <= 6) || (i == 17) || (i == 18)) {
                    self.skeleton.pos[i].x = state.soldier_parts.pos[self.num].x
                        + f32::from(self.direction) * self.legs_animation.pos(i).x;
                    self.skeleton.pos[i].y =
                        state.soldier_parts.pos[self.num].y + self.legs_animation.pos(i).y;
                }

                // body
                if i >= 7 && i <= 16 || i == 19 || i == 20 {
                    self.skeleton.pos[i].x = state.soldier_parts.pos[self.num].x
                        + f32::from(self.direction) * self.body_animation.pos(i).x;

                    if !self.half_dead {
                        self.skeleton.pos[i].y = (self.skeleton.pos[6].y
                            - (state.soldier_parts.pos[self.num].y - body_y))
                            + state.soldier_parts.pos[self.num].y
                            + self.body_animation.pos(i).y;
                    } else {
                        self.skeleton.pos[i].y = 9.00 + state.soldier_parts.pos[self.num].y
                            + self.body_animation.pos(i).y;
                    }
                }
            }
        }

        let mut i = 12;

        if !self.dead_meat {
            let p = vec2(self.skeleton.pos[i].x, self.skeleton.pos[i].y);

            let mouse_aim = vec2(
                self.control.mouse_aim_x as f32,
                self.control.mouse_aim_y as f32,
            );
            let mut r_norm = p - mouse_aim;
            r_norm = vec2normalize(r_norm, r_norm);
            r_norm *= 0.1;
            self.skeleton.pos[i].x = self.skeleton.pos[9].x - f32::from(self.direction) * r_norm.y;
            self.skeleton.pos[i].y = self.skeleton.pos[9].y + f32::from(self.direction) * r_norm.x;

            r_norm *= 50.0;

            self.skeleton.pos[23].x = self.skeleton.pos[9].x - f32::from(self.direction) * r_norm.y;
            self.skeleton.pos[23].y = self.skeleton.pos[9].y + f32::from(self.direction) * r_norm.x;
        }

        let not_aiming_anims = [
            Anim::Reload,
            Anim::ReloadBow,
            Anim::ClipIn,
            Anim::ClipOut,
            Anim::SlideBack,
            Anim::Change,
            Anim::ThrowWeapon,
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

        i = 15;

        if !self.body_animation.is_any(&not_aiming_anims) {
            let p = vec2(self.skeleton.pos[i].x, self.skeleton.pos[i].y);
            let mouse_aim = vec2(
                self.control.mouse_aim_x as f32,
                self.control.mouse_aim_y as f32,
            );
            let mut r_norm = p - mouse_aim;
            r_norm = vec2normalize(r_norm, r_norm);
            r_norm *= arm_s;
            let m = vec2(self.skeleton.pos[16].x, self.skeleton.pos[16].y);
            let p = m + r_norm;
            self.skeleton.pos[i].x = p.x;
            self.skeleton.pos[i].y = p.y;
        }

        if self.body_animation.id == Anim::Throw {
            arm_s = -6.00;
        } else {
            arm_s = -8.00;
        }

        i = 19;

        if !self.body_animation.is_any(&not_aiming_anims) {
            let p = vec2(self.skeleton.pos[i].x, self.skeleton.pos[i].y);
            let mouse_aim = vec2(
                self.control.mouse_aim_x as f32,
                self.control.mouse_aim_y as f32,
            );
            let mut r_norm = p - mouse_aim;
            r_norm = vec2normalize(r_norm, r_norm);
            r_norm *= arm_s;
            let m = vec2(self.skeleton.pos[16].x, self.skeleton.pos[16].y - 4.0);
            let p = m + r_norm;
            self.skeleton.pos[i].x = p.x;
            self.skeleton.pos[i].y = p.y;
        }

        for i in 1..21 {
            if (self.dead_meat || self.half_dead) && (i < 17) && (i != 7) && (i != 8) {
                let mut position = vec2(
                    state.soldier_parts.pos[self.num].x,
                    state.soldier_parts.pos[self.num].y,
                );
                self.on_ground =
                    self.check_skeleton_map_collision(state, i, position.x, position.y);
                println!("ok");
            }
        }

        if !self.dead_meat {
            self.body_animation.do_animation();
            self.legs_animation.do_animation();

            self.on_ground = false;

            let position = vec2(
                state.soldier_parts.pos[self.num].x,
                state.soldier_parts.pos[self.num].y,
            );

            self.check_map_collision(state, position.x - 3.5, position.y - 12.0, 1);
            let mut position = vec2(
                state.soldier_parts.pos[self.num].x,
                state.soldier_parts.pos[self.num].y,
            );
            self.check_map_collision(state, position.x + 3.5, position.y - 12.0, 1);

            body_y = 0.0;
            arm_s = 0.0;

            // Walking either left or right (though only one can be active at once)
            if self.control.left ^ self.control.right {
                if self.control.left ^ (self.direction == 1) {
                    // WRONG
                    arm_s = 0.25;
                } else {
                    body_y = 0.25;
                }
            }
            // If a leg is inside a polygon, caused by the modification of ArmS and
            // BodyY, this is there to not lose contact to ground on slope polygons
            if body_y == 0.0 {
                //let leg_vector = vec2(
                //  state.soldier_parts.pos[self.num].x + 2.0,
                //  state.soldier_parts.pos[self.num].y + 1.9,
                //);
                //    if Map.RayCast(LegVector, LegVector, LegDistance, 10) {
                body_y = 0.25;
                // }
            }
            if arm_s == 0.0 {
                //let leg_vector = vec2(
                //  state.soldier_parts.pos[self.num].x - 2.0,
                //  state.soldier_parts.pos[self.num].y + 1.9,
                //);
                //    if Map.RayCast(LegVector, LegVector, LegDistance, 10) {
                arm_s = 0.25;
                // }
            }
            position = vec2(
                state.soldier_parts.pos[self.num].x,
                state.soldier_parts.pos[self.num].y,
            );
            self.on_ground =
                self.check_map_collision(state, position.x + 2.0, position.y + 2.0 - body_y, 0);
            position = vec2(
                state.soldier_parts.pos[self.num].x,
                state.soldier_parts.pos[self.num].y,
            );
            self.on_ground = self.on_ground
                || self.check_map_collision(state, position.x - 2.0, position.y + 2.0 - arm_s, 0);
            position = vec2(
                state.soldier_parts.pos[self.num].x,
                state.soldier_parts.pos[self.num].y,
            );
            let grounded = self.on_ground;
            self.on_ground_for_law =
                self.check_radius_map_collision(state, position.x, position.y, grounded);

            let grounded = self.on_ground || self.on_ground_for_law;
            self.on_ground =
                self.check_map_vertices_collision(state, position.x, position.y, 3.00, grounded)
                    || self.on_ground;
            //    OnGround or OnGroundForLaw) or OnGround;
            if !(self.on_ground ^ self.on_ground_last_frame) {
                self.on_ground_permanent = self.on_ground;
            }

            self.on_ground_last_frame = self.on_ground;

            if (self.jets_count < state.map.start_jet) && !(self.control.jets) {
                //if self.on_ground
                /* (MainTickCounter mod 2 = 0) */
                {
                    self.jets_count += 1;
                }
            }

            self.alpha = 255;

            self.skeleton.do_verlet_timestep_for(22, 29);
            self.skeleton.do_verlet_timestep_for(24, 30);
        }

        if self.dead_meat {
            self.skeleton.do_verlet_timestep();

            state.soldier_parts.pos[self.num] = self.skeleton.pos[12];

            //CheckSkeletonOutOfBounds;
        }

        if state.soldier_parts.velocity[self.num].x > MAX_VELOCITY {
            state.soldier_parts.velocity[self.num].x = MAX_VELOCITY;
        }
        if state.soldier_parts.velocity[self.num].x < -MAX_VELOCITY {
            state.soldier_parts.velocity[self.num].x = -MAX_VELOCITY;
        }
        if state.soldier_parts.velocity[self.num].y > MAX_VELOCITY {
            state.soldier_parts.velocity[self.num].y = MAX_VELOCITY;
        }
        if state.soldier_parts.velocity[self.num].y < -MAX_VELOCITY {
            state.soldier_parts.velocity[self.num].y = MAX_VELOCITY;
        }
    }

    pub fn check_map_collision(
        &mut self,
        state: &mut MainState,
        x: f32,
        y: f32,
        area: i32,
    ) -> bool {
        let s_pos = vec2(x, y);

        let pos = vec2(
            s_pos.x + state.soldier_parts.velocity[self.num].x,
            s_pos.y + state.soldier_parts.velocity[self.num].y,
        );
        let rx = ((pos.x / state.map.sectors_division as f32).round()) as i32 + 25;
        let ry = ((pos.y / state.map.sectors_division as f32).round()) as i32 + 25;

        if (rx > 0) && (rx < state.map.sectors_num + 25) && (ry > 0)
            && (ry < state.map.sectors_num + 25)
        {
            for j in 0..state.map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = state.map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                let polytype = state.map.polygons[poly].polytype;

                if polytype != PolyType::NoCollide && polytype != PolyType::OnlyBulletsCollide {
                    let mut polygons = state.map.polygons[poly];
                    if state.map.point_in_poly(pos, &mut polygons) {
                        self.handle_special_polytypes(state, polytype, pos);

                        let mut dist = 0.0;
                        let mut k = 0;

                        let mut perp =
                            state
                                .map
                                .closest_perpendicular(poly as i32, pos, &mut dist, &mut k);

                        let step = perp;

                        perp = vec2normalize(perp, perp);
                        perp *= dist;
                        dist = vec2length(state.soldier_parts.velocity[self.num]);

                        if vec2length(perp) > dist {
                            perp = vec2normalize(perp, perp);
                            perp *= dist;
                        }
                        if (area == 0)
                            || ((area == 1)
                                && ((state.soldier_parts.velocity[self.num].y < 0.0)
                                    || (state.soldier_parts.velocity[self.num].x > SLIDELIMIT)
                                    || (state.soldier_parts.velocity[self.num].x < -SLIDELIMIT)))
                        {
                            state.soldier_parts.old_pos[self.num] =
                                state.soldier_parts.pos[self.num];
                            state.soldier_parts.pos[self.num] -= perp;
                            if state.map.polygons[poly].polytype == PolyType::Bouncy {
                                perp = vec2normalize(perp, perp);
                                perp *= state.map.polygons[poly].bounciness * dist;
                            }
                            state.soldier_parts.velocity[self.num] -= perp;
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
                                if (state.soldier_parts.velocity[self.num].x < SLIDELIMIT)
                                    && (state.soldier_parts.velocity[self.num].x > -SLIDELIMIT)
                                    && (step.y > SLIDELIMIT)
                                {
                                    state.soldier_parts.pos[self.num] =
                                        state.soldier_parts.old_pos[self.num];
                                    state.soldier_parts.forces[self.num].y -= GRAV;
                                }

                                if (step.y > SLIDELIMIT) && (polytype != PolyType::Ice)
                                    && (polytype != PolyType::Bouncy)
                                {
                                    if (self.legs_animation.id == Anim::Stand)
                                        || (self.legs_animation.id == Anim::Fall)
                                        || (self.legs_animation.id == Anim::Crouch)
                                    {
                                        state.soldier_parts.velocity[self.num].x *=
                                            STANDSURFACECOEFX;
                                        state.soldier_parts.velocity[self.num].y *=
                                            STANDSURFACECOEFY;

                                        state.soldier_parts.forces[self.num].x -=
                                            state.soldier_parts.velocity[self.num].x;
                                    } else if self.legs_animation.id == Anim::Prone {
                                        if self.legs_animation.frame > 24 {
                                            if !(self.control.down
                                                && (self.control.left || self.control.right))
                                            {
                                                state.soldier_parts.velocity[self.num].x *=
                                                    STANDSURFACECOEFX;
                                                state.soldier_parts.velocity[self.num].y *=
                                                    STANDSURFACECOEFY;

                                                state.soldier_parts.forces[self.num].x -=
                                                    state.soldier_parts.velocity[self.num].x;
                                            }
                                        } else {
                                            state.soldier_parts.velocity[self.num].x *=
                                                SURFACECOEFX;
                                            state.soldier_parts.velocity[self.num].y *=
                                                SURFACECOEFY;
                                        }
                                    } else if self.legs_animation.id == Anim::GetUp {
                                        state.soldier_parts.velocity[self.num].x *= SURFACECOEFX;
                                        state.soldier_parts.velocity[self.num].y *= SURFACECOEFY;
                                    } else if self.legs_animation.id == Anim::ProneMove {
                                        state.soldier_parts.velocity[self.num].x *=
                                            STANDSURFACECOEFX;
                                        state.soldier_parts.velocity[self.num].y *=
                                            STANDSURFACECOEFY;
                                    }
                                }
                            } else if (self.legs_animation.id == Anim::CrouchRun)
                                || (self.legs_animation.id == Anim::CrouchRunBack)
                            {
                                state.soldier_parts.velocity[self.num].x *= CROUCHMOVESURFACECOEFX;
                                state.soldier_parts.velocity[self.num].y *= CROUCHMOVESURFACECOEFY;
                            } else {
                                state.soldier_parts.velocity[self.num].x *= SURFACECOEFX;
                                state.soldier_parts.velocity[self.num].y *= SURFACECOEFY;
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
        state: &mut MainState,
        x: f32,
        y: f32,
        r: f32,
        has_collided: bool,
    ) -> bool {
        let s_pos = vec2(x, y);

        let pos = vec2(
            s_pos.x + state.soldier_parts.velocity[self.num].x,
            s_pos.y + state.soldier_parts.velocity[self.num].y,
        );
        let rx = ((pos.x / state.map.sectors_division as f32).round()) as i32 + 25;
        let ry = ((pos.y / state.map.sectors_division as f32).round()) as i32 + 25;

        if (rx > 0) && (rx < state.map.sectors_num + 25) && (ry > 0)
            && (ry < state.map.sectors_num + 25)
        {
            for j in 0..state.map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = state.map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                let polytype = state.map.polygons[poly].polytype;

                if polytype != PolyType::NoCollide && polytype != PolyType::OnlyBulletsCollide {
                    for i in 0..3 {
                        let vert = vec2(
                            state.map.polygons[poly].vertices[i].x,
                            state.map.polygons[poly].vertices[i].y,
                        );

                        let dist = distance(vert, pos);
                        if dist < r {
                            if !has_collided {
                                self.handle_special_polytypes(state, polytype, pos);
                            }
                            let mut dir = pos - vert;
                            dir = vec2normalize(dir, dir);
                            state.soldier_parts.pos[self.num] += dir;
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
        state: &mut MainState,
        x: f32,
        y: f32,
        has_collided: bool,
    ) -> bool {
        let mut s_pos = vec2(x, y - 3.0);

        let mut det_acc = vec2length(state.soldier_parts.velocity[self.num]).trunc() as i32;
        if det_acc == 0 {
            det_acc = 1;
        }

        let step = state.soldier_parts.velocity[self.num] * (1 / det_acc) as f32;

        for _z in 0..det_acc {
            s_pos.x += step.x;
            s_pos.y += step.y;

            let rx = ((s_pos.x / state.map.sectors_division as f32).round()) as i32 + 25;
            let ry = ((s_pos.y / state.map.sectors_division as f32).round()) as i32 + 25;

            if (rx > 0) && (rx < state.map.sectors_num + 25) && (ry > 0)
                && (ry < state.map.sectors_num + 25)
            {
                for j in 0..state.map.sectors_poly[rx as usize][ry as usize].polys.len() {
                    let poly =
                        state.map.sectors_poly[rx as usize][ry as usize].polys[j] as usize - 1;
                    let polytype = state.map.polygons[poly].polytype;

                    if polytype != PolyType::NoCollide && polytype != PolyType::OnlyBulletsCollide {
                        for k in 0..2 {
                            let mut norm = state.map.perps[poly][k];
                            norm *= -SOLDIER_COL_RADIUS;

                            let mut pos = s_pos + norm;

                            if state.map.point_in_poly_edges(pos.x, pos.y, poly as i32) {
                                if !has_collided {
                                    self.handle_special_polytypes(state, polytype, pos);
                                }
                                let mut d = 0.0;
                                let mut b = 0;
                                let mut perp = state.map.closest_perpendicular(
                                    poly as i32,
                                    pos,
                                    &mut d,
                                    &mut b,
                                );

                                let mut p1 = vec2(0.0, 0.0);
                                let mut p2 = vec2(0.0, 0.0);
                                match b {
                                    1 => {
                                        p1 = vec2(
                                            state.map.polygons[poly].vertices[0].x,
                                            state.map.polygons[poly].vertices[0].y,
                                        );
                                        p2 = vec2(
                                            state.map.polygons[poly].vertices[1].x,
                                            state.map.polygons[poly].vertices[1].y,
                                        );
                                    }
                                    2 => {
                                        p1 = vec2(
                                            state.map.polygons[poly].vertices[1].x,
                                            state.map.polygons[poly].vertices[1].y,
                                        );
                                        p2 = vec2(
                                            state.map.polygons[poly].vertices[2].x,
                                            state.map.polygons[poly].vertices[2].y,
                                        );
                                    }
                                    3 => {
                                        p1 = vec2(
                                            state.map.polygons[poly].vertices[2].x,
                                            state.map.polygons[poly].vertices[2].y,
                                        );
                                        p2 = vec2(
                                            state.map.polygons[poly].vertices[0].x,
                                            state.map.polygons[poly].vertices[0].y,
                                        );
                                    }
                                    _ => {}
                                }

                                let p3 = pos;
                                let d = point_line_distance(p1, p2, p3);
                                perp *= d;

                                state.soldier_parts.pos[self.num] =
                                    state.soldier_parts.old_pos[self.num];
                                state.soldier_parts.velocity[self.num] =
                                    state.soldier_parts.forces[self.num] - perp;

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
        state: &mut MainState,
        i: i32,
        x: f32,
        y: f32,
    ) -> bool {
        let mut result = false;
        let pos = vec2(x - 1.0, y + 4.0);
        let rx = ((pos.x / state.map.sectors_division as f32).round()) as i32 + 25;
        let ry = ((pos.y / state.map.sectors_division as f32).round()) as i32 + 25;

        if (rx > 0) && (rx < state.map.sectors_num + 25) && (ry > 0)
            && (ry < state.map.sectors_num + 25)
        {
            for j in 0..state.map.sectors_poly[rx as usize][ry as usize].polys.len() {
                let poly = state.map.sectors_poly[rx as usize][ry as usize].polys[j] - 1;

                if state.map.point_in_poly_edges(pos.x, pos.y, i32::from(poly)) {
                    let mut dist = 0.0;
                    let mut b = 0;
                    let mut perp =
                        state
                            .map
                            .closest_perpendicular(i32::from(poly), pos, &mut dist, &mut b);
                    perp = vec2normalize(perp, perp);
                    perp *= dist;

                    self.skeleton.pos[i as usize] = self.skeleton.old_pos[i as usize];
                    self.skeleton.pos[i as usize] -= perp;
                    result = true;
                }
            }
        }

        if result {
            let pos = vec2(x, y + 1.0);
            let rx = ((pos.x / state.map.sectors_division as f32).round()) as i32 + 25;
            let ry = ((pos.y / state.map.sectors_division as f32).round()) as i32 + 25;

            if (rx > 0) && (rx < state.map.sectors_num + 25) && (ry > 0)
                && (ry < state.map.sectors_num + 25)
            {
                for j in 0..state.map.sectors_poly[rx as usize][ry as usize].polys.len() {
                    let poly = state.map.sectors_poly[rx as usize][ry as usize].polys[j] - 1;
                    //if (Map.PolyType[poly] <> POLY_TYPE_DOESNT) and (Map.PolyType[poly] <> POLY_TYPE_ONLY_BULLETS) then
                    if state.map.point_in_poly_edges(pos.x, pos.y, i32::from(poly)) {
                        let mut dist = 0.0;
                        let mut b = 0;
                        let mut perp = state.map.closest_perpendicular(
                            i32::from(poly),
                            pos,
                            &mut dist,
                            &mut b,
                        );
                        perp = vec2normalize(perp, perp);
                        perp *= dist;

                        self.skeleton.pos[i as usize] = self.skeleton.old_pos[i as usize];
                        self.skeleton.pos[i as usize] -= perp;
                        result = true;
                    }
                }
            }
        }

        result
    }
}
