use super::*;
use gfx::{SoldierPart, SpriteData};
use ini::Ini;
use std::str::FromStr;

type BitSet = [bool; 256];

#[derive(Debug, Copy, Clone)]
pub enum SoldierSprite {
    None,
    Soldier(gfx::Soldier),
    Weapon(gfx::Weapon),
}

impl SoldierSprite {
    pub fn is_none(&self) -> bool {
        matches!(self, SoldierSprite::None)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SoldierColor {
    None,
    Main,
    Pants,
    Skin,
    Hair,
    Cygar,
    Headblood,
}

#[derive(Debug, Copy, Clone)]
pub enum SoldierAlpha {
    Base,
    Blood,
    Nades,
}

#[derive(Debug, Copy, Clone)]
pub struct SoldierPartInfo {
    pub name: &'static str,
    pub sprite: SoldierSprite,
    pub point: (usize, usize),
    pub center: (f32, f32),
    pub flexibility: f32,
    pub flip: bool,
    /// Bravo and Delta soldiers use the `team2` version of the image.
    pub team: bool,
    pub color: SoldierColor,
    pub alpha: SoldierAlpha,
    pub visible: bool,
}

pub struct SoldierGraphics {
    pub parts: Vec<SoldierPartInfo>,
    pub base_visibility: BitSet,
}

impl SoldierGraphics {
    pub fn new() -> SoldierGraphics {
        SoldierGraphics {
            parts: SoldierPart::data().to_vec(),
            base_visibility: std::array::from_fn(|i| {
                SoldierPart::data().get(i).is_some_and(|p| p.visible)
            }),
        }
    }

    pub fn load_data(&mut self, cfg: &Ini) {
        self.parts = SoldierPart::data().to_vec();

        if let Some(data) = cfg.section(Some("GOSTEK".to_owned())) {
            let mut key = String::with_capacity(256);

            let copy_and_insert_underscores = |dest: &mut String, source: &str| {
                for (i, ch) in source.chars().enumerate() {
                    if i > 0 && ch.is_uppercase() {
                        dest.push('_')
                    };
                    dest.push(ch);
                }
            };

            for part in &mut self.parts {
                key.clear();
                copy_and_insert_underscores(&mut key, part.name);

                let len = key.len();
                key.push_str("_CenterX");

                if let Some(value) = data.get(&key) {
                    part.center.0 = f32::from_str(value).unwrap_or(part.center.0);
                }

                key.truncate(len);
                key.push_str("_CenterY");

                if let Some(value) = data.get(&key) {
                    part.center.1 = f32::from_str(value).unwrap_or(part.center.1);
                }
            }
        }
    }
}

pub fn render_soldier(
    soldier: &Soldier,
    soldier_graphics: &SoldierGraphics,
    sprites: &[Vec<Sprite>],
    batch: &mut DrawBatch,
    frame_percent: f32,
    realistic_mode: bool,
) {
    let sk = &soldier.skeleton;
    let (colors, alpha) = colors_and_alpha(soldier, realistic_mode);
    let has_blood = alpha[SoldierAlpha::Blood as usize] > 0;
    let visible = parts_visibility(&soldier_graphics.base_visibility, soldier, has_blood);

    let team2_offset = match soldier.team {
        Team::Bravo | Team::Delta => gfx::Soldier::Team2Stopa.id() - gfx::Soldier::Stopa.id(),
        _ => 0,
    };
    let pos = |p: usize| lerp(sk.old_pos(p), sk.pos(p), frame_percent);

    // dreadlocks hang from the head, rotated with it
    let head = soldier_graphics.parts[SoldierPart::Head.id()].point;
    let head_rot = vec2angle(pos(head.1) - pos(head.0)) - std::f32::consts::FRAC_PI_2;
    let dreadlocks = SoldierPart::HairDreadlock1.id()..=SoldierPart::HairDreadlock5.id();

    for (i, part) in soldier_graphics.parts.iter().enumerate() {
        if visible[i] && !part.sprite.is_none() {
            let mut sprite_index: usize = 0;
            let mut cx = part.center.0;
            let mut cy = part.center.1;
            let mut scale = vec2(1.0, 1.0);
            let (p0, p1) = part.point;
            let mut p0 = pos(p0);
            let p1 = pos(p1);
            let rot = vec2angle(p1 - p0);

            if part.team {
                sprite_index += team2_offset;
            }

            if soldier.direction != 1 {
                if part.flip {
                    cy = 1.0 - cy;
                    sprite_index += 1;
                } else {
                    scale.y = -1.0;
                }
            }

            let sprite = match part.sprite {
                SoldierSprite::Soldier(part_sprite) => {
                    let group = gfx::Group::Soldier;
                    let sprite = part_sprite + sprite_index;
                    &sprites[group.id()][sprite.id()]
                }
                SoldierSprite::Weapon(part_sprite) => {
                    let group = gfx::Group::Weapon;
                    let sprite = part_sprite + sprite_index;
                    &sprites[group.id()][sprite.id()]
                }
                SoldierSprite::None => unreachable!(),
            };

            if dreadlocks.contains(&i) {
                let offset = vec2(
                    -cy * sprite.height * f32::from(soldier.direction),
                    cx * sprite.width,
                );
                let (sin, cos) = head_rot.sin_cos();
                p0 += vec2(
                    offset.x * cos - offset.y * sin,
                    offset.x * sin + offset.y * cos,
                );
                cx = 0.0;
                cy = 0.5;
                let n = (i - SoldierPart::HairDreadlock1.id()) as f32;
                scale.x = 0.75 + (1.0 - 0.75) / 5.0 * n;
            } else if part.flexibility > 0.0 {
                scale.x = f32::min(1.5, (p1 - p0).length() / part.flexibility);
            }

            let color = {
                let color = colors[part.color as usize];
                rgba(color.r(), color.g(), color.b(), alpha[part.alpha as usize])
            };

            batch.add_sprite(
                sprite,
                color,
                Transform::WithPivot {
                    pivot: vec2(cx * sprite.width, cy * sprite.height),
                    pos: vec2(p0.x, p0.y + 1.0),
                    scale,
                    rot,
                },
            );
        }
    }
}

fn colors_and_alpha(soldier: &Soldier, realistic_mode: bool) -> ([Color; 7], [u8; 3]) {
    let mut alpha_base = soldier.alpha;
    let mut alpha_blood = (200.0 - soldier.health.round()).clamp(0.0, 255.0) as u8;
    let mut color_cygar = rgb(255, 255, 255);
    let color_none = rgb(255, 255, 255);
    let hex = |c: u32| rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    let color_main = hex(soldier.looks.shirt);
    let color_pants = hex(soldier.looks.pants);
    let color_skin = hex(soldier.looks.skin);
    let color_hair = hex(soldier.looks.hair);
    let color_headblood = rgb(172, 169, 168);

    if soldier.has_cigar == 5 {
        color_cygar = rgb(97, 97, 97);
    }

    if soldier.health > (90.0 - 40.0 * f32::from(realistic_mode as u8)) {
        alpha_blood = 0;
    }

    if realistic_mode && soldier.visible > 0 && soldier.visible < 45 && soldier.alpha > 60 {
        // TODO: if this really needs to change it should be done somewhere during update, not here
        // soldier.alpha = 3 * soldier.visible;
        alpha_base = 3 * soldier.visible;
        alpha_blood = 0;
    }

    let alpha_nades: u8 = (0.75 * f32::from(alpha_base)) as u8;

    (
        [
            color_none,
            color_main,
            color_pants,
            color_skin,
            color_hair,
            color_cygar,
            color_headblood,
        ],
        [alpha_base, alpha_blood, alpha_nades],
    )
}

fn parts_visibility(base_visibility: &BitSet, soldier: &Soldier, blood: bool) -> BitSet {
    let mut visible = *base_visibility;

    if blood {
        visible[SoldierPart::LeftThighDmg.id()] = true;
        visible[SoldierPart::LeftLowerlegDmg.id()] = true;
        visible[SoldierPart::LeftForearmDmg.id()] = true;
        visible[SoldierPart::LeftArmDmg.id()] = true;
        visible[SoldierPart::ChestDmg.id()] = true;
        visible[SoldierPart::HipDmg.id()] = true;
        visible[SoldierPart::HeadDmg.id()] = true;
        visible[SoldierPart::RightThighDmg.id()] = true;
        visible[SoldierPart::RightLowerlegDmg.id()] = true;
        visible[SoldierPart::RightForearmDmg.id()] = true;
        visible[SoldierPart::RightArmDmg.id()] = true;
    }

    if soldier.control.jets && soldier.jets_count > 0 {
        visible[SoldierPart::LeftFoot.id()] = false;
        visible[SoldierPart::RightFoot.id()] = false;
        visible[SoldierPart::LeftJetfoot.id()] = true;
        visible[SoldierPart::RightJetfoot.id()] = true;
    }

    if soldier.vest > 0.0 {
        visible[SoldierPart::Vest.id()] = true;
    }

    let index = if soldier.tertiary_weapon().kind == WeaponKind::FragGrenade {
        SoldierPart::FragGrenade1.id()
    } else {
        SoldierPart::ClusterGrenade1.id()
    };

    let ammo = soldier.tertiary_weapon().ammo_count as i32;

    let n = if soldier.body_animation.id == Anim::Throw {
        i32::min(5, ammo - 1)
    } else {
        i32::min(5, ammo)
    };

    for i in 0..n {
        visible[index + i as usize] = true;
    }

    match soldier.looks.chain {
        1 => {
            visible[SoldierPart::SilverLchain.id()] = true;
            visible[SoldierPart::SilverRchain.id()] = true;
            visible[SoldierPart::SilverPendant.id()] = true;
        }
        2 => {
            visible[SoldierPart::GoldenLchain.id()] = true;
            visible[SoldierPart::GoldenRchain.id()] = true;
            visible[SoldierPart::GoldenPendant.id()] = true;
        }
        _ => {}
    }

    if soldier.has_cigar == 5 || soldier.has_cigar == 10 {
        visible[SoldierPart::Cigar.id()] = true;
    }

    if soldier.dead_meat {
        visible[SoldierPart::Head.id()] = false;
        visible[SoldierPart::HeadDmg.id()] = false;
        visible[SoldierPart::HeadDead.id()] = true;
        visible[SoldierPart::HeadDeadDmg.id()] = true;
    }

    if soldier.primary_weapon().kind == WeaponKind::Bow
        || soldier.primary_weapon().kind == WeaponKind::FlameBow
    {
        visible[SoldierPart::RamboBadge.id()] = true;
    } else {
        let grabbed = match soldier.body_animation.id {
            Anim::Wipe | Anim::TakeOff => soldier.body_animation.frame > 4,
            _ => false,
        };

        if soldier.wear_helmet == 1 {
            match (soldier.head_cap, grabbed) {
                (1, true) => visible[SoldierPart::GrabbedHelmet.id()] = true,
                (1, false) => visible[SoldierPart::Helmet.id()] = true,
                (2, true) => visible[SoldierPart::GrabbedHat.id()] = true,
                (2, false) => visible[SoldierPart::Hat.id()] = true,
                _ => {}
            }
        }

        let hair_style = soldier.looks.hair_style;

        if grabbed || soldier.wear_helmet != 1 || hair_style == 3 {
            match hair_style {
                1 => {
                    for i in 0..6 {
                        visible[SoldierPart::HairDreadlocks.id() + i] = true;
                    }
                }
                2 => visible[SoldierPart::HairPunk.id()] = true,
                3 => visible[SoldierPart::MrT.id()] = true,
                4 => visible[SoldierPart::HairNormal.id()] = true,
                _ => {}
            }
        }
    }

    // secondary weapon (on the back)

    let index = soldier.secondary_weapon().kind.index();

    if index >= WeaponKind::DesertEagles.index() && index <= WeaponKind::Flamer.index() {
        visible[SoldierPart::SecondaryDeagles.id() + index] = true;
    }

    // primary weapon

    let weapon = soldier.primary_weapon();
    let ammo = weapon.ammo_count;
    let reload_count = weapon.reload_time_count;

    if weapon.kind == WeaponKind::Minigun {
        visible[SoldierPart::PrimaryMinigun.id()] = true;

        if ammo > 0 || (ammo == 0 && weapon.reload_time_count < 65) {
            visible[SoldierPart::PrimaryMinigunClip.id()] = true;
        }

        if soldier.fired > 0 {
            visible[SoldierPart::PrimaryMinigunFire.id()] = true;
        }
    } else if weapon.kind == WeaponKind::Bow || weapon.kind == WeaponKind::FlameBow {
        if ammo == 0 {
            visible[SoldierPart::PrimaryBowArrowReload.id()] = true;
        } else {
            visible[SoldierPart::PrimaryBowArrow.id()] = true;
        }

        if soldier.body_animation.id == Anim::ReloadBow {
            visible[SoldierPart::PrimaryBowReload.id()] = true;
            visible[SoldierPart::PrimaryBowStringReload.id()] = true;
        } else {
            visible[SoldierPart::PrimaryBow.id()] = true;
            visible[SoldierPart::PrimaryBowString.id()] = true;
        }

        if soldier.fired > 0 {
            visible[SoldierPart::PrimaryBowFire.id()] = true;
        }
    } else if !soldier.dead_meat {
        let first = SoldierPart::PrimaryDeagles;
        let mut index = weapon.kind.index();

        if index >= WeaponKind::DesertEagles.index() && index <= WeaponKind::Flamer.index() {
            if weapon.kind == WeaponKind::Flamer {
                index = SoldierPart::PrimaryFlamer.id() - first.id();
            } else {
                index *= 3;
            }

            visible[first.id() + index] = true;

            if weapon.clip_sprite.is_some()
                && (ammo > 0
                    || ammo == 0
                        && (reload_count < weapon.clip_in_time
                            || reload_count > weapon.clip_out_time))
            {
                visible[first.id() + index + 1] = true;
            }

            if soldier.fired > 0 {
                visible[first.id() + 2] = true;
            }
        }
    }

    visible
}

pub fn render_skeleton(soldier: &Soldier, batch: &mut DrawBatch, px: f32, frame_percent: f32) {
    let sk = &soldier.skeleton;

    for constraint in sk.constraints() {
        let pa = constraint.particle_num.0;
        let pb = constraint.particle_num.1;

        let a = lerp(sk.old_pos(pa), sk.pos(pa), frame_percent);
        let b = lerp(sk.old_pos(pb), sk.pos(pb), frame_percent);

        let m = Transform::WithPivot {
            pos: a,
            pivot: vec2(0.0, 0.0),
            scale: vec2(distance(a, b), 1.0),
            rot: vec2angle(b - a),
        }
        .matrix();

        batch.add_quad(
            None,
            &[
                vertex(m * vec2(0.0, -0.5 * px), Vec2::ZERO, rgb(255, 255, 0)),
                vertex(m * vec2(1.0, -0.5 * px), Vec2::ZERO, rgb(255, 255, 0)),
                vertex(m * vec2(1.0, 0.5 * px), Vec2::ZERO, rgb(255, 255, 0)),
                vertex(m * vec2(0.0, 0.5 * px), Vec2::ZERO, rgb(255, 255, 0)),
            ],
        );
    }

    for particle in sk.particles() {
        let p = lerp(particle.old_pos, particle.pos, frame_percent);
        let m = Mat2d::translate(p.x, p.y);

        batch.add_quad(
            None,
            &[
                vertex(m * vec2(-px, -px), Vec2::ZERO, rgb(0, 0, 255)),
                vertex(m * vec2(1.0 * px, -px), Vec2::ZERO, rgb(0, 0, 255)),
                vertex(m * vec2(1.0 * px, 1.0 * px), Vec2::ZERO, rgb(0, 0, 255)),
                vertex(m * vec2(-px, 1.0 * px), Vec2::ZERO, rgb(0, 0, 255)),
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_parts_have_team2_images() {
        let offset = gfx::Soldier::Team2Stopa.id() - gfx::Soldier::Stopa.id();
        for part in SoldierPart::data().iter().filter(|p| p.team) {
            let SoldierSprite::Soldier(sprite) = part.sprite else {
                continue;
            };
            for flip in 0..=usize::from(part.flip) {
                let base = (sprite + flip).filename();
                let team2 = (sprite + flip + offset).filename();
                assert_eq!(
                    team2,
                    base.replacen("gostek-gfx/", "gostek-gfx/team2/", 1),
                    "{}",
                    part.name
                );
            }
        }
    }
}
