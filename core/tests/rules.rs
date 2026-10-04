//! Rules of the team and Rambo modes: who a punch disarms, the bow's immunity and drop, CTF's
//! kits. Needs the game assets (`SOLDANK_ASSETS` or `../assets`), else the tests are skipped.

use soldank_core::assets::Vfs;
use soldank_core::*;
use std::path::PathBuf;
use std::sync::Arc;

fn vfs() -> Option<Vfs> {
    let path = std::env::var_os("SOLDANK_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets"));
    if !path.exists() {
        if std::env::var_os("SOLDANK_REQUIRE_ASSETS").is_some() {
            panic!("assets required but not found at {}", path.display());
        }
        eprintln!("skipping: no assets at {}", path.display());
        return None;
    }
    let mut vfs = Vfs::new();
    vfs.mount_game_files(&path).unwrap();
    Some(vfs)
}

/// ctf_Ash in game mode `mode` (`sv_gamemode`).
fn ctf_ash(mode: &str) -> Option<World> {
    let vfs = vfs()?;
    let data = Arc::new(GameData::load(&vfs).unwrap());
    let mut cvars = config::Cvars::new();
    register_cvars(&mut cvars);
    cvars.set("sv_gamemode", mode).unwrap();
    let map = MapFile::load(&vfs, "ctf_Ash").unwrap();
    let config = WorldConfig::from_cvars(&cvars, &data);
    Some(World::new(data, map, config))
}

/// A soldier of `team` with Desert Eagles in hand, out of its spawn protection.
fn soldier(world: &mut World, team: Team, weapon: WeaponKind) -> SoldierId {
    let id = world.spawn_soldier();
    let gun = world.config.weapons.get(weapon);
    let soldier = &mut world.soldiers[id];
    soldier.team = team;
    soldier.ceasefire_counter = -1;
    soldier.weapons[soldier.active_weapon] = gun;
    id
}

/// `from` punches `to` in the chest.
fn punch(world: &mut World, from: SoldierId, to: SoldierId) {
    let chest = world.soldiers[to].skeleton.pos(9);
    let params = BulletParams {
        style: BulletStyle::Fist,
        weapon: WeaponKind::NoWeapon,
        position: chest - vec2(6.0, 0.0),
        velocity: vec2(6.0, 0.0),
        timeout: 3,
        hit_multiply: 0.01,
        team: world.soldiers[from].team,
        sprite: None,
        seed: Some(1),
        must_create: true,
        net: false,
        owner_immune: false,
    };
    assert!(world.create_bullet(&params, from));
    world.step(&[]);
}

fn disarmed(world: &World, id: SoldierId) -> bool {
    world.soldiers[id].body_animation.id == Anim::ThrowWeapon
}

#[test]
fn a_punch_disarms_enemies_not_teammates() {
    let Some(mut world) = ctf_ash("2") else {
        return;
    };
    let alice = soldier(&mut world, Team::Alpha, WeaponKind::DesertEagles);
    let ally = soldier(&mut world, Team::Alpha, WeaponKind::DesertEagles);
    world.step(&[]);
    punch(&mut world, alice, ally);
    assert!(!disarmed(&world, ally));

    let Some(mut world) = ctf_ash("2") else {
        return;
    };
    let alice = soldier(&mut world, Team::Alpha, WeaponKind::DesertEagles);
    let enemy = soldier(&mut world, Team::Bravo, WeaponKind::DesertEagles);
    world.step(&[]);
    punch(&mut world, alice, enemy);
    assert!(disarmed(&world, enemy));

    // without teams everyone is on their own
    let Some(mut world) = ctf_ash("0") else {
        return;
    };
    let alice = soldier(&mut world, Team::None, WeaponKind::DesertEagles);
    let other = soldier(&mut world, Team::None, WeaponKind::DesertEagles);
    world.step(&[]);
    punch(&mut world, alice, other);
    assert!(disarmed(&world, other));
}

#[test]
fn while_someone_holds_the_bow_only_rambo_fights() {
    let Some(mut world) = ctf_ash("4") else {
        return;
    };
    let rambo = soldier(&mut world, Team::None, WeaponKind::Bow);
    let alice = soldier(&mut world, Team::None, WeaponKind::DesertEagles);
    let bob = soldier(&mut world, Team::None, WeaponKind::DesertEagles);
    let hit = |world: &mut World, victim, attacker| {
        let World {
            soldiers,
            config,
            rng,
            ..
        } = world;
        health_hit(
            soldiers,
            victim,
            attacker,
            10.0,
            6,
            Vec2::ZERO,
            config,
            None,
            rng,
        )
        .is_some()
    };
    assert!(!hit(&mut world, bob, alice), "others don't hurt each other");
    assert!(hit(&mut world, rambo, alice), "Rambo can be hurt");
    assert!(hit(&mut world, alice, rambo), "Rambo hurts");
    // nobody's Rambo: everyone fights
    let none = world.config.weapons.get(WeaponKind::NoWeapon);
    world.soldiers[rambo].weapons = [none; 3];
    assert!(hit(&mut world, bob, alice));
}

#[test]
fn rambo_drops_the_bow() {
    let Some(mut world) = ctf_ash("4") else {
        return;
    };
    let rambo = soldier(&mut world, Team::None, WeaponKind::Bow);
    world.step(&[]);
    world.player_command(rambo, PlayerCommand::Kill, &mut Vec::new());
    world.step(&[]);
    let bows = world
        .things
        .iter()
        .filter(|t| t.active && t.kind == ThingKind::RamboBow);
    assert_eq!(bows.count(), 1);
}

#[test]
fn ctf_shares_medikits_between_the_teams() {
    let kits = |mode: &str| -> Option<Vec<u8>> {
        let mut world = ctf_ash(mode)?;
        world.spawn_kits();
        let medikits = world.things.iter().filter(|t| t.active);
        Some(
            medikits
                .filter(|t| t.kind == ThingKind::MedicalKit)
                .map(|t| t.team)
                .collect(),
        )
    };
    let Some(ctf) = kits("3") else {
        return;
    };
    assert!(ctf.contains(&1) && ctf.contains(&2), "{ctf:?}");
    let dm = kits("0").unwrap();
    assert!(!dm.is_empty() && dm.iter().all(|&t| t == 0), "{dm:?}");
}
