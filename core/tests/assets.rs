//! Tests against the real game assets.
//!
//! Assets are taken from `SOLDANK_ASSETS` or the workspace `assets/` directory. When none
//! are available the tests are skipped, unless `SOLDANK_REQUIRE_ASSETS` is set (CI).
//!
//! Snapshots are regression baselines of the current behaviour. Update them deliberately
//! with `INSTA_UPDATE=always cargo test -p soldank-core` and review the diff.

use soldank_core::assets::Vfs;
use soldank_core::*;
use std::fmt::Write;
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
    vfs.mount(&path).unwrap();
    Some(vfs)
}

#[test]
fn parses_ctf_ash() {
    let Some(vfs) = vfs() else { return };
    let map = MapFile::load(&vfs, "ctf_Ash").unwrap();

    let mut out = String::new();
    writeln!(out, "name: {}", map.mapname).unwrap();
    writeln!(out, "texture: {}", map.texture_name).unwrap();
    writeln!(out, "polygons: {}", map.polygons.len()).unwrap();
    writeln!(
        out,
        "sectors: {} x {}",
        map.sectors_num, map.sectors_division
    )
    .unwrap();
    writeln!(out, "props: {}", map.props.len()).unwrap();
    writeln!(out, "scenery: {}", map.scenery.len()).unwrap();
    writeln!(out, "colliders: {}", map.colliders.len()).unwrap();
    writeln!(out, "spawnpoints: {}", map.spawnpoints.len()).unwrap();
    for s in map.spawnpoints.iter().take(4) {
        writeln!(out, "  ({}, {}) team {}", s.x, s.y, s.team).unwrap();
    }

    insta::assert_snapshot!(out);
}

#[test]
fn loads_every_bundled_map() {
    let Some(vfs) = vfs() else { return };
    let maps: Vec<String> = vfs
        .list("maps")
        .into_iter()
        .filter(|m| m.ends_with(".pms"))
        .collect();
    assert!(!maps.is_empty());

    for file in maps {
        let name = file.trim_end_matches(".pms");
        if let Err(e) = MapFile::load(&vfs, name) {
            panic!("{file}: {e}");
        }
    }
}

#[test]
fn loads_game_data() {
    let Some(vfs) = vfs() else { return };
    let data = GameData::load(&vfs).unwrap();

    let mut out = String::new();
    writeln!(
        out,
        "skeleton particles: {}",
        data.soldier_skeleton.particles().len()
    )
    .unwrap();
    writeln!(
        out,
        "skeleton constraints: {}",
        data.soldier_skeleton.constraints().len()
    )
    .unwrap();
    for anim in [
        Anim::Stand,
        Anim::Run,
        Anim::Jump,
        Anim::Prone,
        Anim::Roll,
        Anim::Change,
    ] {
        writeln!(
            out,
            "{anim:?}: {} frames",
            data.anims.get(anim).num_frames()
        )
        .unwrap();
    }

    insta::assert_snapshot!(out);
}

/// Scripted input sequence: (first tick, buttons, aim offset from the soldier).
const SCRIPT: &[(u64, Buttons, (f32, f32))] = &[
    (0, Buttons::empty(), (100.0, 0.0)),
    (90, Buttons::RIGHT, (100.0, 0.0)),
    (150, Buttons::RIGHT.union(Buttons::JUMP), (100.0, -20.0)),
    (170, Buttons::RIGHT, (100.0, -20.0)),
    (200, Buttons::LEFT.union(Buttons::JETS), (-100.0, -50.0)),
    (260, Buttons::FIRE, (-100.0, 10.0)),
    (300, Buttons::CROUCH, (100.0, 0.0)),
    (330, Buttons::PRONE, (100.0, 0.0)),
    (360, Buttons::empty(), (100.0, 0.0)),
];

/// Regression trace of the simulation (movement, animation, firing) on ctf_Ash.
/// This guards refactors; opensoldat-derived traces will replace it (see PLAN.md §6).
#[test]
fn soldier_trace_ctf_ash() {
    let Some(vfs) = vfs() else { return };
    let data = Arc::new(GameData::load(&vfs).unwrap());
    let map = MapFile::load(&vfs, "ctf_Ash").unwrap();

    let mut world = World::new(data, map, WorldConfig::default());
    let id = world.spawn_soldier();
    let mut out = String::new();
    let mut fired = 0;

    for tick in 0..420u64 {
        let &(_, buttons, (ax, ay)) = SCRIPT
            .iter()
            .rev()
            .find(|(start, ..)| *start <= tick)
            .unwrap();
        let pos = world.soldiers[id].particle.pos;
        let input = Input {
            buttons,
            aim: pos + vec2(ax, ay),
        };

        let events = world.step(&[(id, input)]);
        fired += events
            .iter()
            .filter(|e| matches!(e, GameEvent::BulletFired { .. }))
            .count();

        if tick % 15 == 0 {
            let s = &world.soldiers[id];
            writeln!(
                out,
                "{tick:3}: pos ({:8.2}, {:8.2}) vel ({:6.2}, {:6.2}) legs {:?}/{} body {:?}/{} ground {} bullets {}",
                s.particle.pos.x,
                s.particle.pos.y,
                s.particle.velocity.x,
                s.particle.velocity.y,
                s.legs_animation.id,
                s.legs_animation.frame,
                s.body_animation.id,
                s.body_animation.frame,
                s.on_ground,
                world.bullets.len(),
            )
            .unwrap();
        }
    }

    writeln!(out, "bullets fired: {fired}").unwrap();
    insta::assert_snapshot!(out);
}
