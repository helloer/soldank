//! Fuzzing the network messages' decoding: the server decodes whatever a client sends it, and
//! clients whatever comes from a server. Mutated and random bytes must come out as `None` or
//! a message, never a panic, a huge allocation or a long wait. Deterministic (its own random
//! numbers), so a failure repeats; `SOLDANK_FUZZ_ROUNDS` runs more rounds.

use soldank_core::net::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Counts the largest allocation since the last reset.
struct Largest;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Largest {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        // SAFETY: the system allocator's contract, passed on
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as above
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Largest = Largest;

/// No message decoding may ask for more memory than this at once (renet's messages are far
/// smaller).
const MAX_ALLOCATION: usize = 16 * 1024 * 1024;
/// Nor take longer than this.
const MAX_TIME: Duration = Duration::from_millis(100);

/// xorshift64*
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn soldier(num: PlayerNum) -> SoldierState {
    SoldierState {
        num,
        buttons: 0x11,
        aim: [100.0, -20.0],
        pos: [f32::from(num) * 10.0, 5.0],
        velocity: [0.5, -1.0],
        health: 150.0,
        vest: 0.0,
        dead: false,
        jets: 120,
        weapons: [(1, 7), (11, 12), (13, 2)],
        active_weapon: 0,
        kills: 3,
        deaths: 1,
        flags: 0,
        bonus: 0,
        bonus_time: 0,
        ping: 80,
        stat: None,
        weapon_sel: 0x3fff,
    }
}

fn thing(slot: u8) -> ThingState {
    ThingState {
        slot,
        kind: 1,
        points: vec![[1.0, 2.0], [3.0, 4.0], [5.0, 6.0], [7.0, 8.0]],
        holder: Some(2),
        in_base: true,
        ammo: 0,
    }
}

fn snapshot(tick: u64) -> Snapshot {
    Snapshot {
        tick,
        soldiers: (1..=4).map(soldier).collect(),
        things: (0..3).map(thing).collect(),
        team_scores: [0, 3, 1, 0, 0, 0],
        time_left: 36000,
    }
}

/// Encoded messages of every kind to start from.
fn seeds() -> Vec<Vec<u8>> {
    let control = ControlState {
        tick: 42,
        buttons: 0x13,
        aim: [100.5, -20.0],
        pos: [1.25, 2.5],
        velocity: [0.1, -0.2],
        snapshot: 40,
        resets: 1,
    };
    let bullet = BulletState {
        weapon: 3,
        pos: [10.0, 20.0],
        velocity: [5.0, -1.0],
        seed: 77,
    };
    let looks = NetLooks {
        shirt: 0x8f8f8f,
        pants: 0x8f8f8f,
        skin: 0xe6b478,
        hair: 0,
        jet: 0,
        hair_style: 1,
        chain: 0,
        head_cap: 0,
    };
    let clients = [
        ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            password: "secret".into(),
            name: "Major".into(),
            looks: looks.clone(),
            team: Some(1),
        },
        ClientMessage::Ready { mod_hash: 1 },
        ClientMessage::Download("maps/ctf_Ash.pms".into()),
        ClientMessage::JoinTeam(2),
        ClientMessage::Loadout {
            primary: 1,
            secondary: 11,
        },
        ClientMessage::Control(control),
        ClientMessage::Chat {
            text: "hello there".into(),
            team: true,
        },
        ClientMessage::Command("kick 3".into()),
        ClientMessage::Bullet(bullet.clone()),
        ClientMessage::Vote {
            kind: VoteKind::Kick(3),
            reason: "camping".into(),
        },
        ClientMessage::Vote {
            kind: VoteKind::Map("ctf_Run".into()),
            reason: String::new(),
        },
        ClientMessage::MapList,
    ];
    let base = snapshot(40);
    let mut now = snapshot(42);
    now.soldiers[1].pos = [99.0, 98.0];
    now.things.pop();
    let servers = [
        ServerMessage::Welcome {
            you: 2,
            map: "ctf_Ash".into(),
            map_hash: 12345,
            cvars: vec![
                ("sv_gamemode".into(), "3".into()),
                ("sv_gravity".into(), "0.06".into()),
            ],
            weapons_mods: [Some("[Info]\nName=Mod\n".into()), None],
            game_mod: Some(GameMod {
                name: "mod".into(),
                hash: 99,
            }),
        },
        ServerMessage::PlayerJoined(PlayerInfo {
            num: 3,
            name: "Kruger".into(),
            team: 1,
            looks,
            bot: true,
        }),
        ServerMessage::Snapshot(now.clone()),
        ServerMessage::SnapshotDelta(SnapshotDelta::between(&base, &now)),
        ServerMessage::FileChunk {
            request: "maps/ctf_Ash.pms".into(),
            path: "maps/ctf_Ash.pms".into(),
            size: 4096,
            offset: 1024,
            data: (0..255).collect(),
        },
        ServerMessage::Bullet { owner: 2, bullet },
        ServerMessage::ForcePosition {
            pos: [1.0, 2.0],
            velocity: [0.0, 0.5],
        },
        ServerMessage::VoteOn {
            kind: VoteKind::Kick(4),
            starter: 0,
            reason: "Server: Possible cheating".into(),
        },
        ServerMessage::MapList(vec!["ctf_Ash".into(), "ctf_Run".into()]),
        ServerMessage::ServerText("Welcome".into()),
    ];
    let mut seeds: Vec<Vec<u8>> = clients.iter().map(encode).collect();
    seeds.extend(servers.iter().map(encode));
    seeds
}

/// One of the ways `bytes` gets damaged.
fn mutate(rng: &mut Rng, bytes: &mut Vec<u8>) {
    match rng.below(7) {
        // a flipped bit
        0 if !bytes.is_empty() => {
            let at = rng.below(bytes.len());
            bytes[at] ^= 1 << rng.below(8);
        }
        // a random byte
        1 if !bytes.is_empty() => {
            let at = rng.below(bytes.len());
            bytes[at] = rng.next() as u8;
        }
        // cut short
        2 => bytes.truncate(rng.below(bytes.len() + 1)),
        // bytes added
        3 => {
            let at = rng.below(bytes.len() + 1);
            let more: Vec<u8> = (0..rng.below(16) + 1).map(|_| rng.next() as u8).collect();
            bytes.splice(at..at, more);
        }
        // a run of 0xff: lengths and counts as big as they get
        4 => {
            let at = rng.below(bytes.len() + 1);
            let run = vec![0xff; rng.below(12) + 1];
            bytes.splice(at..at, run);
        }
        // a part repeated
        5 if !bytes.is_empty() => {
            let from = rng.below(bytes.len());
            let to = (from + rng.below(32) + 1).min(bytes.len());
            let part = bytes[from..to].to_vec();
            let at = rng.below(bytes.len() + 1);
            bytes.splice(at..at, part);
        }
        // whatever
        _ => {
            let len = rng.below(64);
            *bytes = (0..len).map(|_| rng.next() as u8).collect();
        }
    }
}

/// Decodes `bytes` both ways, within the limits.
fn decode_both(bytes: &[u8]) {
    LARGEST.store(0, Ordering::Relaxed);
    let start = Instant::now();
    let _ = decode::<ClientMessage>(bytes);
    let _ = decode::<ServerMessage>(bytes);
    let took = start.elapsed();
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(
        largest <= MAX_ALLOCATION,
        "{largest} bytes at once for {} bytes: {bytes:02x?}",
        bytes.len()
    );
    assert!(took <= MAX_TIME, "{took:?} for {bytes:02x?}");
}

#[test]
fn damaged_messages_decode_safely() {
    let rounds: usize = std::env::var("SOLDANK_FUZZ_ROUNDS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(30_000);
    let seeds = seeds();
    for seed in &seeds {
        assert!(decode::<ClientMessage>(seed).is_some() || decode::<ServerMessage>(seed).is_some());
    }
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..rounds {
        let mut bytes = seeds[rng.below(seeds.len())].clone();
        for _ in 0..rng.below(4) + 1 {
            mutate(&mut rng, &mut bytes);
        }
        decode_both(&bytes);
    }
}

#[test]
fn claimed_lengths_need_the_bytes() {
    // a chat line, a file chunk and a map list claiming more than they have
    for seed in seeds() {
        for at in 0..seed.len() {
            let mut bytes = seed.clone();
            for byte in bytes.iter_mut().skip(at).take(8) {
                *byte = 0xff;
            }
            decode_both(&bytes);
        }
    }
}
