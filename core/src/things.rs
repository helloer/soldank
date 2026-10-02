//! Things (`Things.pas`): dropped weapons, kits, flags, the bow, parachutes and
//! stationary guns, each a small particle skeleton in a fixed slot table.

use super::*;
use slotmap::SlotMap;

/// Number of thing slots (`MAX_THINGS`).
pub const MAX_THINGS: usize = 90;

const SECOND: i32 = 60;
const GUNRESISTTIME: i32 = SECOND * 20;
const FLAG_TIMEOUT: i32 = SECOND * 25;
const DEFAULT_INTEREST_TIME: i32 = SECOND * 5 + 50;
const FLAG_INTEREST_TIME: i32 = SECOND * 25;
const BOW_INTEREST_TIME: i32 = SECOND * 41 + 40;
const GUN_RADIUS: f32 = 10.0;
const BOW_RADIUS: f32 = 20.0;
const KIT_RADIUS: f32 = 12.0;
const STAT_RADIUS: f32 = 15.0;
// untyped Pascal constant compared against a Single: extended precision
const MINMOVEDELTA: f64 = 0.63;
const SPAWNRANDOMVELOCITY: f32 = 25.0;
pub(crate) const DEFAULTVEST: f32 = 100.0;
const CLUSTER_GRENADES: u8 = 3;
/// `sv_healthcooldown` default: seconds between medikit pickups.
pub(crate) const HEALTH_COOLDOWN: u64 = 2;

/// Thing styles (`OBJECT_*`), numbered like Soldat.
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Default)]
pub enum ThingKind {
    AlphaFlag = 1,
    BravoFlag,
    PointmatchFlag,
    Ussocom,
    #[default]
    DesertEagle,
    HkMp5,
    Ak74,
    SteyrAug,
    Spas12,
    Ruger77,
    M79,
    BarrettM82A1,
    Minimi,
    Minigun,
    RamboBow,
    MedicalKit,
    GrenadeKit,
    FlamerKit,
    PredatorKit,
    VestKit,
    BerserkKit,
    ClusterKit,
    Parachute,
    CombatKnife,
    Chainsaw,
    Law,
    StationaryGun,
}

impl ThingKind {
    /// From Soldat's `OBJECT_*` number.
    pub fn from_num(n: u8) -> Option<ThingKind> {
        use ThingKind::*;
        [
            AlphaFlag,
            BravoFlag,
            PointmatchFlag,
            Ussocom,
            DesertEagle,
            HkMp5,
            Ak74,
            SteyrAug,
            Spas12,
            Ruger77,
            M79,
            BarrettM82A1,
            Minimi,
            Minigun,
            RamboBow,
            MedicalKit,
            GrenadeKit,
            FlamerKit,
            PredatorKit,
            VestKit,
            BerserkKit,
            ClusterKit,
            Parachute,
            CombatKnife,
            Chainsaw,
            Law,
            StationaryGun,
        ]
        .into_iter()
        .find(|k| *k as u8 == n)
    }

    /// The thing a dropped weapon becomes (`TSprite.DropWeapon`).
    pub fn for_weapon(weapon: WeaponKind) -> Option<ThingKind> {
        use ThingKind::*;
        Some(match weapon {
            WeaponKind::USSOCOM => Ussocom,
            WeaponKind::DesertEagles => DesertEagle,
            WeaponKind::MP5 => HkMp5,
            WeaponKind::Ak74 => Ak74,
            WeaponKind::SteyrAUG => SteyrAug,
            WeaponKind::Spas12 => Spas12,
            WeaponKind::Ruger77 => Ruger77,
            WeaponKind::M79 => M79,
            WeaponKind::Barrett => BarrettM82A1,
            WeaponKind::Minimi => Minimi,
            WeaponKind::Minigun => Minigun,
            WeaponKind::Knife => CombatKnife,
            WeaponKind::Chainsaw => Chainsaw,
            WeaponKind::LAW => Law,
            _ => return None,
        })
    }

    /// The weapon a soldier gets from picking up this thing.
    pub fn weapon(self) -> Option<WeaponKind> {
        use ThingKind::*;
        Some(match self {
            Ussocom => WeaponKind::USSOCOM,
            DesertEagle => WeaponKind::DesertEagles,
            HkMp5 => WeaponKind::MP5,
            Ak74 => WeaponKind::Ak74,
            SteyrAug => WeaponKind::SteyrAUG,
            Spas12 => WeaponKind::Spas12,
            Ruger77 => WeaponKind::Ruger77,
            M79 => WeaponKind::M79,
            BarrettM82A1 => WeaponKind::Barrett,
            Minimi => WeaponKind::Minimi,
            Minigun => WeaponKind::Minigun,
            CombatKnife => WeaponKind::Knife,
            Chainsaw => WeaponKind::Chainsaw,
            Law => WeaponKind::LAW,
            _ => return None,
        })
    }

    pub fn is_flag(self) -> bool {
        self < ThingKind::Ussocom
    }

    /// Guns that despawn after `GUNRESISTTIME` (everything a soldier can drop).
    pub fn is_gun(self) -> bool {
        self.weapon().is_some()
    }

    pub fn is_kit(self) -> bool {
        (ThingKind::MedicalKit..=ThingKind::ClusterKit).contains(&self)
    }
}

/// Template skeletons of things (`Anims.pas`: karabin.po at several scales, kit.po, ...).
#[derive(Debug, Clone)]
pub struct ThingSkeletons {
    rifles: Vec<(u32, ParticleSystem)>,
    pub kit: ParticleSystem,
    pub flag: ParticleSystem,
    pub para: ParticleSystem,
    pub stat: ParticleSystem,
}

impl ThingSkeletons {
    pub fn load(vfs: &assets::Vfs) -> Result<ThingSkeletons, DataError> {
        let load = |file: &str, scale: f32| -> Result<ParticleSystem, DataError> {
            ParticleSystem::parse(file, &vfs.read_to_string(file)?, scale, 1.0, 0.0, 0.0, 0.0)
        };
        let rifles = [10, 11, 18, 22, 28, 36, 37, 39, 43, 50, 55]
            .into_iter()
            .map(|tenths| Ok((tenths, load("objects/karabin.po", tenths as f32 / 10.0)?)))
            .collect::<Result<_, DataError>>()?;

        Ok(ThingSkeletons {
            rifles,
            kit: load("objects/kit.po", 2.15)?,
            flag: load("objects/flag.po", 4.0)?,
            para: load("objects/para.po", 5.0)?,
            stat: load("objects/stat.po", 4.0)?,
        })
    }

    /// `RifleSkeletonNN`: karabin.po scaled by `tenths / 10`.
    fn rifle(&self, tenths: u32) -> &ParticleSystem {
        &self.rifles.iter().find(|(t, _)| *t == tenths).unwrap().1
    }
}

#[derive(Debug, Clone, Default)]
pub struct Thing {
    pub active: bool,
    pub kind: ThingKind,
    pub owner: Option<SoldierId>,
    pub holding: Option<SoldierId>,
    pub ammo_count: u8,
    pub radius: f32,
    pub timeout: i32,
    pub static_type: bool,
    pub interest: i32,
    pub collide_with_bullets: bool,
    pub in_base: bool,
    /// Spawn point index (1-based) the thing last spawned at (`SpawnBoxes`).
    pub last_spawn: usize,
    pub team: u8,
    pub skeleton: ParticleSystem,
    pub collide_count: [u8; 4],
}

/// A weapon a soldier lets go of, turned into a thing by the world (`TSprite.DropWeapon`).
#[derive(Debug, Copy, Clone)]
pub struct WeaponDrop {
    pub kind: ThingKind,
    pub ammo_count: u8,
    /// Hand position (skeleton point 16).
    pub pos: Vec2,
    /// The soldier's velocity and aim direction, for the throw.
    pub velocity: Vec2,
    pub aim: Vec2,
    /// Whether the soldier was already dead (weaker throw).
    pub dead: bool,
}

/// What thing code needs from the world.
pub struct ThingCtx<'a> {
    pub data: &'a GameData,
    pub map: &'a MapFile,
    pub config: &'a WorldConfig,
    pub soldiers: &'a mut SlotMap<SoldierId, Soldier>,
    pub rng: &'a mut PascalRandom,
}

impl Thing {
    fn particle_active(&self, num: usize) -> bool {
        self.skeleton
            .particles()
            .get(num - 1)
            .is_some_and(|p| p.active)
    }

    fn pos(&self, num: usize) -> Vec2 {
        self.skeleton
            .particles()
            .get(num - 1)
            .map_or(Vec2::ZERO, |p| p.pos)
    }

    /// `TThing.Kill`
    pub fn kill(&mut self) {
        self.skeleton = ParticleSystem::default();
        self.active = false;
    }

    /// `TThing.MoveSkeleton(x, y, False)`
    fn move_skeleton(&mut self, offset: Vec2) {
        for num in 1..=self.skeleton.particles().len() {
            if self.particle_active(num) {
                let pos = self.skeleton.pos(num) + offset;
                *self.skeleton.pos_mut(num) = pos;
                *self.skeleton.old_pos_mut(num) = pos;
            }
        }
    }
}

/// `CreateThing`: fills slot `slot` (or the first free one) and returns its index.
pub fn create_thing(
    things: &mut [Thing],
    ctx: &mut ThingCtx,
    pos: Vec2,
    owner: Option<SoldierId>,
    kind: ThingKind,
    slot: Option<usize>,
    drop: Option<&WeaponDrop>,
) -> Option<usize> {
    use ThingKind::*;

    // only one flag of each kind
    if kind.is_flag() {
        for thing in things.iter_mut().filter(|t| t.active && t.kind == kind) {
            thing.kill();
        }
    }

    let i = match slot {
        Some(i) => i,
        None => things.iter().position(|t| !t.active)?,
    };

    let skeletons = &ctx.data.thing_skeletons;
    let gravity = ctx.config.gravity;
    let thing = &mut things[i];
    thing.active = true;
    thing.kind = kind;
    thing.holding = None;
    thing.owner = owner;
    thing.timeout = 0;
    thing.static_type = false;
    thing.in_base = false;
    thing.collide_count = [0; 4];

    // (v_damping, gravity multiplier, skeleton, radius, timeout, interest, collides with bullets)
    let (v_damping, gravity_multiplier, template, radius, timeout, interest, collides) = match kind
    {
        AlphaFlag | BravoFlag | PointmatchFlag => (
            0.991,
            1.0,
            &skeletons.flag,
            19.0,
            FLAG_TIMEOUT,
            FLAG_INTEREST_TIME,
            true,
        ),
        Ussocom => gun(0.994, 1.05, skeletons.rifle(10)),
        DesertEagle => gun(0.996, 1.09, skeletons.rifle(11)),
        HkMp5 => gun(0.995, 1.11, skeletons.rifle(22)),
        Ak74 | SteyrAug => gun(0.994, 1.16, skeletons.rifle(37)),
        Spas12 => gun(0.993, 1.15, skeletons.rifle(36)),
        Ruger77 => gun(0.993, 1.13, skeletons.rifle(36)),
        M79 | Chainsaw | Law => gun(0.994, 1.15, skeletons.rifle(28)),
        BarrettM82A1 => gun(0.993, 1.18, skeletons.rifle(43)),
        Minimi => gun(0.993, 1.2, skeletons.rifle(39)),
        Minigun => gun(0.991, 1.4, skeletons.rifle(55)),
        CombatKnife => {
            let (v, g, s, _, t, i, c) = gun(0.994, 1.15, skeletons.rifle(18));
            (v, g, s, GUN_RADIUS * 1.5, t, i, c)
        }
        RamboBow => (
            0.996,
            0.65,
            skeletons.rifle(50),
            BOW_RADIUS,
            FLAG_TIMEOUT,
            BOW_INTEREST_TIME,
            true,
        ),
        MedicalKit => kit(
            1.05,
            &skeletons.kit,
            ctx.config.respawn_time * GUNRESISTTIME,
        ),
        GrenadeKit | ClusterKit => kit(1.07, &skeletons.kit, FLAG_TIMEOUT),
        FlamerKit | PredatorKit | VestKit | BerserkKit => kit(1.17, &skeletons.kit, FLAG_TIMEOUT),
        Parachute => (0.993, 1.15, &skeletons.para, 0.0, 3600, 0, false),
        StationaryGun => (0.99, 0.2, &skeletons.stat, STAT_RADIUS, 60, 0, false),
    };

    let mut skeleton = template.clone();
    skeleton.set_physics(1.0, gravity_multiplier * gravity, v_damping);
    thing.skeleton = skeleton;
    thing.radius = radius;
    thing.timeout = timeout;
    thing.interest = interest;
    thing.collide_with_bullets = collides;

    if kind == CombatKnife {
        // the knife skeleton is reversed, with a little random tilt
        let (p1, p2) = (thing.skeleton.pos(1), thing.skeleton.pos(2));
        *thing.skeleton.pos_mut(2) = p1;
        *thing.skeleton.old_pos_mut(2) = p1;
        *thing.skeleton.pos_mut(1) = p2;
        *thing.skeleton.old_pos_mut(1) = p2;
        let x1 = thing.skeleton.pos(1).x;
        thing.skeleton.pos_mut(1).x = fpc(ext(x1) + f64::from(ctx.rng.below(100)) / 100.0);
        let x2 = thing.skeleton.pos(2).x;
        thing.skeleton.pos_mut(2).x = fpc(ext(x2) - f64::from(ctx.rng.below(100)) / 100.0);
    }

    thing.owner = owner;
    thing.move_skeleton(pos);

    // throw weapon (server)
    let thrown = (kind > PointmatchFlag && kind < MedicalKit) || kind == Law || kind == Chainsaw;
    if thrown && let Some(drop) = drop {
        for num in 1..=2 {
            *thing.skeleton.pos_mut(num) += drop.velocity;
        }
        let (speed1, speed2) = if drop.dead { (0.02, 0.64) } else { (0.01, 3.0) };
        *thing.skeleton.pos_mut(1) += drop.aim * speed1;
        *thing.skeleton.pos_mut(2) += drop.aim * speed2;
    }

    Some(i)
}

type ThingParams<'a> = (f32, f32, &'a ParticleSystem, f32, i32, i32, bool);

fn gun(v_damping: f32, gravity: f32, skeleton: &ParticleSystem) -> ThingParams<'_> {
    // sv_guns_collide is off by default
    (
        v_damping,
        gravity,
        skeleton,
        GUN_RADIUS,
        GUNRESISTTIME,
        0,
        false,
    )
}

fn kit(gravity: f32, skeleton: &ParticleSystem, timeout: i32) -> ThingParams<'_> {
    // sv_kits_collide is off by default
    (
        0.989,
        gravity,
        skeleton,
        KIT_RADIUS,
        timeout,
        DEFAULT_INTEREST_TIME,
        false,
    )
}

impl Thing {
    /// Port of `TThing.Update` (server side). Flags, parachutes, the bow and stationary
    /// guns only get their physics so far.
    pub fn update(&mut self, slot: usize, ctx: &mut ThingCtx) {
        use ThingKind::*;

        let was_static = self.static_type;

        if !self.static_type {
            let (mut collided, mut collided2) = (false, false);

            for i in 1..=4 {
                if !self.particle_active(i) || (self.holding.is_some() && i != 2) {
                    continue;
                }
                let p = self.pos(i);

                let hit = if self.kind.is_flag() && i == 1 {
                    let hit = self.check_map_collision(i, p.x - 10.0, p.y - 8.0, ctx)
                        || self.check_map_collision(i, p.x + 10.0, p.y - 8.0, ctx)
                        || self.check_map_collision(i, p.x - 10.0, p.y, ctx)
                        || self.check_map_collision(i, p.x + 10.0, p.y, ctx);
                    // TODO: FLAG_STAND_FORCEUP
                    hit
                } else {
                    self.check_map_collision(i, p.x, p.y, ctx)
                };

                if hit {
                    if collided {
                        collided2 = true;
                    }
                    collided = true;
                }
            }

            self.skeleton.do_verlet_timestep();

            // TODO: stationary gun turning, flags following their holder

            // make the thing static if not moving much
            let a = self.pos(1) - self.skeleton.old_pos(1);
            let b = self.pos(2) - self.skeleton.old_pos(2);
            if self.kind != StationaryGun
                && collided
                && collided2
                && ext((a.length() + b.length()) / 2.0) < MINMOVEDELTA
            {
                self.static_type = true;
            }
        }

        // TODO: flags in base and captures, stationary guns, the bow, parachutes

        if self.kind != StationaryGun {
            self.check_sprite_collision(slot, ctx);
        }

        // count time out
        self.timeout = (self.timeout - 1).max(-1000);
        if self.timeout == 0 {
            match self.kind {
                AlphaFlag | BravoFlag | PointmatchFlag | RamboBow => {
                    if self.holding.is_some() {
                        self.timeout = FLAG_TIMEOUT;
                    } else {
                        self.respawn(slot, ctx);
                    }
                }
                FlamerKit | PredatorKit | VestKit | BerserkKit | ClusterKit | Parachute => {
                    self.kill()
                }
                kind if kind.is_gun() => self.kill(),
                _ => {}
            }
        }

        self.check_out_of_bounds(slot, ctx);

        if !was_static && self.static_type {
            for num in 1..=self.skeleton.particles().len().min(4) {
                *self.skeleton.old_pos_mut(num) = self.skeleton.pos(num);
            }
        }
    }

    /// Port of `TThing.CheckMapCollision`.
    fn check_map_collision(&mut self, i: usize, x: f32, y: f32, ctx: &ThingCtx) -> bool {
        use PolyType::*;

        let map = ctx.map;
        let pos = vec2(x, y - 0.5);
        let div = map.sectors_division as f32;
        let (rx, ry) = ((pos.x / div).round() as i32, (pos.y / div).round() as i32);
        let n = map.sectors_num;
        if !(rx > -n && rx < n && ry > -n && ry < n) {
            return false;
        }

        let owner_team = self.owner.and_then(|o| ctx.soldiers.get(o)).map(|s| s.team);
        let mut result = false;

        for &w in map.sector(rx, ry) {
            let w = w as usize - 1;
            let polytype = map.polygons[w].polytype;

            let mut teamcol = owner_team.is_none_or(|team| team_collides(polytype, team, false));
            // flags pass through team polygons
            if self.kind.is_flag()
                && (AlphaBullets as u8..=DeltaPlayers as u8).contains(&(polytype as u8))
            {
                teamcol = false;
            }

            if !teamcol
                || matches!(
                    polytype,
                    OnlyBulletsCollide
                        | OnlyPlayersCollide
                        | NoCollide
                        | OnlyFlaggers
                        | NotFlaggers
                )
                || !map.point_in_poly_edges(pos.x, pos.y, w as i32)
            {
                continue;
            }

            // TODO: TBackgroundState; background polygons don't collide for now
            if matches!(polytype, Background | BackgroundTransition) {
                continue;
            }

            let (mut d, mut b) = (0.0, 0);
            let perp = vec2normalize(map.closest_perpendicular(w as i32, pos, &mut d, &mut b)) * d;

            if self.kind.is_flag() {
                if i == 1 {
                    *self.skeleton.pos_mut(i) = self.skeleton.old_pos(i);
                } else {
                    // bounce back along the perpendicular, keeping the speed
                    let diff = self.pos(i) - self.skeleton.old_pos(i);
                    let diff_perp = vec2normalize(perp) * diff.length();
                    let p = self.pos(i) - perp;
                    *self.skeleton.pos_mut(i) = p;
                    *self.skeleton.old_pos_mut(i) = p + diff_perp;
                    // TODO: upward force on a free flag (needs per-particle forces access)
                }
            } else {
                let p = self.skeleton.old_pos(i) - perp;
                *self.skeleton.pos_mut(i) = p;
            }

            self.collide_count[i - 1] = self.collide_count[i - 1].wrapping_add(1);
            result = true;
        }

        result
    }

    /// Port of `TThing.CheckOutOfBounds`.
    fn check_out_of_bounds(&mut self, slot: usize, ctx: &mut ThingCtx) {
        use ThingKind::*;

        let bound = (ctx.map.sectors_num * ctx.map.sectors_division - 10) as f32;
        for i in 1..=4 {
            let p = self.pos(i);
            if p.x.abs() > bound || p.y.abs() > bound {
                match self.kind {
                    AlphaFlag | BravoFlag | PointmatchFlag | RamboBow => self.respawn(slot, ctx),
                    kind if kind.is_kit() => self.respawn(slot, ctx),
                    StationaryGun => self.kill(),
                    kind if kind.is_gun() => self.kill(),
                    _ => {}
                }
            }
        }
    }

    /// Port of `TThing.Respawn`: back to a spawn point of its kind.
    pub fn respawn(&mut self, slot: usize, ctx: &mut ThingCtx) {
        use ThingKind::*;

        if let Some(holder) = self.holding.and_then(|h| ctx.soldiers.get_mut(h)) {
            holder.holded_thing = None;
        }
        self.kill();

        let (mut a, _) = randomize_start_team(ctx.map, 0, ctx.rng);
        match self.kind {
            AlphaFlag => a = randomize_start_team(ctx.map, 5, ctx.rng).0,
            BravoFlag => a = randomize_start_team(ctx.map, 6, ctx.rng).0,
            PointmatchFlag => a = randomize_start_team(ctx.map, 14, ctx.rng).0,
            RamboBow => a = randomize_start_team(ctx.map, 15, ctx.rng).0,
            MedicalKit => a = spawn_boxes(ctx.map, 8, &mut self.last_spawn, ctx.rng).0,
            GrenadeKit => a = spawn_boxes(ctx.map, 7, &mut self.last_spawn, ctx.rng).0,
            FlamerKit => a = randomize_start_team(ctx.map, 11, ctx.rng).0,
            PredatorKit => a = randomize_start_team(ctx.map, 13, ctx.rng).0,
            VestKit => a = randomize_start_team(ctx.map, 10, ctx.rng).0,
            BerserkKit => a = randomize_start_team(ctx.map, 12, ctx.rng).0,
            ClusterKit => a = randomize_start_team(ctx.map, 9, ctx.rng).0,
            _ => {}
        }

        let kind = self.kind;
        let mut things = [std::mem::take(self)];
        create_thing(&mut things, ctx, a, None, kind, Some(0), None);
        *self = std::mem::take(&mut things[0]);
        let _ = slot;

        self.timeout = FLAG_TIMEOUT;
        self.interest = DEFAULT_INTEREST_TIME;
        self.static_type = false;
        self.collide_count = [0; 4];
        if kind == RamboBow {
            self.interest = BOW_INTEREST_TIME;
        }
        if kind.is_flag() {
            self.interest = FLAG_INTEREST_TIME;
        }
    }

    /// Port of `TThing.CheckSpriteCollision` (server side): soldiers picking things up.
    fn check_sprite_collision(&mut self, slot: usize, ctx: &mut ThingCtx) {
        use ThingKind::*;

        let config = ctx.config;
        let start_health = config.start_health();
        let max_grenades = config.max_grenades as u8;

        let a = self.pos(1) - self.pos(2);
        let k = a.length() / 2.0;
        let mut pos = self.pos(1) + vec2normalize(a) * -k;

        let mut closest_dist = 9_999_999.0;
        let mut closest = None;

        for (id, soldier) in ctx.soldiers.iter() {
            if !soldier.active || soldier.dead_meat {
                continue;
            }

            // Soldat keeps the last tried point for the next soldier
            let col = soldier.particle.pos;
            let mut norm = pos - col;
            if norm.length() >= self.radius {
                pos = self.pos(1);
                norm = pos - col;
                if norm.length() >= self.radius {
                    pos = self.pos(2);
                    norm = pos - col;
                }
            }

            let dist = norm.length();
            let tertiary = soldier.tertiary_weapon();
            if dist < self.radius
                && dist < closest_dist
                && !(self.kind == MedicalKit && soldier.health == start_health)
                && !(self.kind == GrenadeKit
                    && tertiary.ammo_count == max_grenades
                    && tertiary.kind == WeaponKind::FragGrenade)
                && !(self.kind.is_flag() && soldier.ceasefire_counter > 0)
            {
                closest_dist = dist;
                closest = Some(id);
            }
        }

        let Some(j) = closest else { return };
        let soldier = &ctx.soldiers[j];
        let weapon = soldier.primary_weapon().kind;
        let changing = soldier.body_animation.id == Anim::Change;
        let tertiary = *soldier.tertiary_weapon();
        let bow = matches!(weapon, WeaponKind::Bow | WeaponKind::FlameBow);
        let no_bonus = true; // TODO: bonuses

        let taken = (((self.kind > PointmatchFlag && self.kind < RamboBow && !changing)
            || (self.kind > Parachute && !changing))
            && weapon == WeaponKind::NoWeapon
            && self.timeout < GUNRESISTTIME - 30)
            || (self.kind == RamboBow
                && weapon == WeaponKind::NoWeapon
                && self.timeout < config.respawn_time * FLAG_TIMEOUT - 100)
            || (self.kind == MedicalKit && soldier.health < start_health && !soldier.has_pack)
            || (self.kind == GrenadeKit
                && tertiary.ammo_count < max_grenades
                && (tertiary.kind != WeaponKind::ClusterGrenade || tertiary.ammo_count == 0))
            || ((((self.kind == FlamerKit) && !bow)
                || self.kind == PredatorKit
                || self.kind == BerserkKit)
                && no_bonus
                && soldier.ceasefire_counter < 1)
            || (self.kind == VestKit && soldier.vest < DEFAULTVEST)
            || (self.kind == ClusterKit
                && (tertiary.kind == WeaponKind::FragGrenade || tertiary.ammo_count == 0));

        if taken && self.kind != RamboBow {
            self.kill();
        }

        let soldier = &mut ctx.soldiers[j];
        match self.kind {
            kind if kind.is_gun() => {
                if soldier.primary_weapon().kind == WeaponKind::NoWeapon
                    && soldier.body_animation.id != Anim::Change
                    && self.timeout < GUNRESISTTIME - 30
                {
                    let mut gun = config.weapons.get(kind.weapon().unwrap());
                    gun.ammo_count = self.ammo_count;
                    gun.fire_interval_prev = gun.fire_interval;
                    gun.fire_interval_count = gun.fire_interval;
                    soldier.weapons[soldier.active_weapon] = gun;
                }
            }
            RamboBow => {
                if soldier.primary_weapon().kind == WeaponKind::NoWeapon
                    && soldier.body_animation.id != Anim::Change
                    && self.timeout < FLAG_TIMEOUT - 100
                {
                    let mut bow = config.weapons.get(WeaponKind::Bow);
                    bow.ammo_count = 1;
                    bow.fire_interval_prev = bow.fire_interval;
                    bow.fire_interval_count = bow.fire_interval;
                    let active = soldier.active_weapon;
                    soldier.weapons[active] = bow;
                    soldier.weapons[(active + 1) % 2] = config.weapons.get(WeaponKind::FlameBow);
                    soldier.wear_helmet = 1;
                }
            }
            MedicalKit => {
                if soldier.health < start_health && !soldier.has_pack {
                    self.team = soldier.team as u8;
                    soldier.has_pack = true;
                    soldier.health = start_health;
                    self.respawn(slot, ctx);
                }
            }
            GrenadeKit => {
                if tertiary.ammo_count < max_grenades
                    && (tertiary.kind != WeaponKind::ClusterGrenade || tertiary.ammo_count == 0)
                {
                    self.team = soldier.team as u8;
                    soldier.weapons[2] = config.weapons.get(WeaponKind::FragGrenade);
                    soldier.weapons[2].ammo_count = max_grenades;
                    self.respawn(slot, ctx);
                }
            }
            VestKit => soldier.vest = DEFAULTVEST,
            ClusterKit if tertiary.kind == WeaponKind::FragGrenade || tertiary.ammo_count == 0 => {
                soldier.weapons[2] = config.weapons.get(WeaponKind::ClusterGrenade);
                soldier.weapons[2].ammo_count = CLUSTER_GRENADES;
            }
            // TODO: flags, Flamegod/Predator/Berserker bonuses
            _ => {}
        }
    }
}

/// `SpawnBoxes`: like `RandomizeStart` but avoids the spawn point used last time.
/// Returns the position and whether a spawn point of `team` existed.
pub fn spawn_boxes(
    map: &MapFile,
    team: i32,
    last_spawn: &mut usize,
    rng: &mut PascalRandom,
) -> (Vec2, bool) {
    let mut found = true;
    let mut previous = 0;
    let mut spawns: Vec<usize> = Vec::new();

    for (i, spawn) in map.spawnpoints.iter().enumerate().take(255) {
        let num = i + 1;
        if spawn.active && spawn.team == team {
            if *last_spawn != num {
                spawns.push(num);
            } else {
                previous = num;
            }
        }
    }

    if spawns.is_empty() {
        if previous != 0 {
            spawns.push(previous);
        } else {
            found = false;
            spawns = (1..=map.spawnpoints.len().min(255))
                .filter(|&num| map.spawnpoints[num - 1].active)
                .collect();
        }
    }

    if spawns.is_empty() {
        return (Vec2::ZERO, found);
    }

    let num = spawns[rng.below(spawns.len() as i32) as usize];
    let spawn = &map.spawnpoints[num - 1];
    let x = spawn.x - 4 + rng.below(8);
    let y = spawn.y - 4 + rng.below(4);
    *last_spawn = num;
    (vec2(x as f32, y as f32), found)
}

/// `SpawnThings` for kits (deathmatch: no team boxes). Stops at the first kit without
/// a spawn point of its own.
pub fn spawn_things(things: &mut [Thing], ctx: &mut ThingCtx, kind: ThingKind, amount: u8) {
    let team = match kind {
        ThingKind::MedicalKit => 8,
        ThingKind::GrenadeKit => 7,
        ThingKind::FlamerKit => 11,
        ThingKind::PredatorKit => 13,
        ThingKind::VestKit => 10,
        ThingKind::BerserkKit => 12,
        ThingKind::ClusterKit => 9,
        _ => 0,
    };

    for _ in 0..amount {
        // TODO: CTF splits medikits and grenade kits between the teams
        things[MAX_THINGS - 2].team = 0;

        let (mut a, found) = randomize_start_team(ctx.map, team, ctx.rng);
        if !found {
            return;
        }

        // Round returns Int64, so this is Random(Int64)
        let r = 2 * 100 * SPAWNRANDOMVELOCITY as i64;
        a.x = fpc(ext(a.x - SPAWNRANDOMVELOCITY) + ctx.rng.below_i64(r) as f64 / 100.0);
        a.y = fpc(ext(a.y - SPAWNRANDOMVELOCITY) + ctx.rng.below_i64(r) as f64 / 100.0);
        if let Some(l) = create_thing(things, ctx, a, None, kind, None, None) {
            things[l].team = 0;
        }
    }
}
