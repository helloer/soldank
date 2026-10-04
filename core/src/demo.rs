//! Demos of network games, like Soldat's `.sdm`: what the server sent each tick and the
//! player's input, to be replayed through the same client code (a client's world follows from
//! exactly those). Single-player demos play the world again from a map's start: the player's
//! input and what the player did to the world besides ([`LocalAction`]) each tick.

use crate::net::*;
use crate::*;
use bitcode::{Decode, Encode};
use std::io::{self, Read, Write};

/// A demo file starts with this.
pub const DEMO_MAGIC: &[u8; 8] = b"SOLDANKD";

/// Demo files end with this.
pub const DEMO_EXTENSION: &str = "sdemo";

/// What a demo was recorded on.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub struct DemoHeader {
    /// [`PROTOCOL_VERSION`] when it was recorded: the messages need the same one.
    pub protocol: u32,
    pub map: String,
    pub map_hash: u64,
    /// The server's synced cvars.
    pub cvars: Vec<(String, String)>,
    /// The server's weapons mods (see [`ServerMessage::Welcome`]).
    pub weapons_mods: [Option<String>; 2],
    /// The server's mod: playback needs the archive too.
    pub game_mod: Option<GameMod>,
    /// The recording player.
    pub you: Option<PlayerNum>,
    /// The match as it was when recording started (players, soldiers, things), as messages.
    pub start: Vec<ServerMessage>,
    /// When it was recorded (Unix time).
    pub date: u64,
    /// Single player: the world plays again from the frames' actions and inputs (`cvars` are
    /// the game's server cvars, the first frame starts the map).
    pub local: bool,
}

/// A tick.
#[derive(Debug, Clone, Default, PartialEq, Encode, Decode)]
pub struct DemoFrame {
    /// The server's messages, in the order they came.
    pub messages: Vec<ServerMessage>,
    /// The player's buttons and aim point.
    pub input: Option<(u16, [f32; 2])>,
    /// Single player: what was done to the world before this tick.
    pub actions: Vec<LocalAction>,
    /// Single player, now and then: [`world_check`] after this tick, to tell a playback that
    /// went another way.
    pub check: Option<u64>,
}

/// What a single player does to the world besides playing (its input), as recorded in a demo
/// and carried out again on playback.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum LocalAction {
    /// Server cvars that changed (the world follows them every tick).
    Cvars(Vec<(String, String)>),
    /// The map starts (over): the world, its flags, kits and stationary guns.
    Map(String),
    /// The player joins a team (or changes to it): who it is, and what it respawns with.
    Join {
        team: u8,
        name: String,
        looks: NetLooks,
        loadout: [u8; 3],
    },
    /// A weapon picked in the weapons menu (`WeaponKind` index).
    Primary(u8),
    Secondary(u8),
    /// `addbot`: the bot file and the team.
    AddBot {
        file: String,
        team: u8,
    },
    /// A player kicked, by soldier id (`KeyData::as_ffi`).
    Kick(u64),
    /// `dummy`: a target to shoot at.
    Dummy,
    /// A player command (`kill`, `mercy`, a taunt, ...).
    Command(String),
    /// `cycleweapon` (a debug command).
    CycleWeapon,
    /// The player said something.
    Say {
        text: String,
        team: bool,
    },
}

/// A soldier's id as a number (for [`LocalAction::Kick`]), the same again on playback.
pub fn soldier_num(id: SoldierId) -> u64 {
    slotmap::Key::data(&id).as_ffi()
}

pub fn soldier_of_num(num: u64) -> SoldierId {
    SoldierId::from(slotmap::KeyData::from_ffi(num))
}

/// A fingerprint of the world: its tick, soldiers, bullets, things and randomness.
pub fn world_check(world: &World) -> u64 {
    let mut bytes = Vec::new();
    bytes.extend(world.tick.to_le_bytes());
    bytes.extend(world.rng.clone().next_u32().to_le_bytes());
    for (id, s) in &world.soldiers {
        bytes.extend(soldier_num(id).to_le_bytes());
        for v in [s.particle.pos, s.particle.velocity] {
            bytes.extend(v.x.to_le_bytes());
            bytes.extend(v.y.to_le_bytes());
        }
        bytes.extend(s.health.to_le_bytes());
        bytes.push(u8::from(s.dead_meat));
    }
    for bullet in world.bullets.iter().filter(|b| b.active) {
        bytes.extend(bullet.particle.pos.x.to_le_bytes());
        bytes.extend(bullet.particle.pos.y.to_le_bytes());
    }
    for thing in world.things.iter().filter(|t| t.active) {
        let pos = thing.skeleton.pos(1);
        bytes.extend(pos.x.to_le_bytes());
        bytes.extend(pos.y.to_le_bytes());
    }
    file_hash(&bytes)
}

impl DemoFrame {
    pub fn input(&self) -> Option<Input> {
        self.input.map(|(buttons, aim)| Input {
            buttons: Buttons::from_bits_truncate(buttons),
            aim: Vec2::from(aim),
        })
    }

    pub fn set_input(&mut self, input: Option<&Input>) {
        self.input = input.map(|i| (i.buttons.bits(), i.aim.into()));
    }
}

/// Writes a demo as it's played: the header, then a frame a tick. Snapshots go into the file
/// as what changed since the one before ([`read_demo`] makes them whole again).
pub struct DemoWriter<W: Write> {
    out: W,
    pub frames: u64,
    /// The last snapshot written.
    last: Option<Snapshot>,
}

impl<W: Write> DemoWriter<W> {
    pub fn new(mut out: W, header: &DemoHeader) -> io::Result<DemoWriter<W>> {
        out.write_all(DEMO_MAGIC)?;
        write_record(&mut out, &encode(header))?;
        Ok(DemoWriter {
            out,
            frames: 0,
            last: last_snapshot(&header.start),
        })
    }

    /// A tick; every second's worth goes to the file (a game that's killed loses little).
    pub fn frame(&mut self, frame: &DemoFrame) -> io::Result<()> {
        self.frames += 1;
        let mut frame = frame.clone();
        deltas(&mut self.last, &mut frame.messages);
        write_record(&mut self.out, &encode(&frame))?;
        if self.frames.is_multiple_of(60) {
            self.out.flush()?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.out.flush()?;
        Ok(self.out)
    }
}

/// A whole demo.
#[derive(Debug, Clone)]
pub struct Demo {
    pub header: DemoHeader,
    pub frames: Vec<DemoFrame>,
}

/// Reads a demo's header only (for a list of demos).
pub fn read_demo_header(mut input: impl Read) -> io::Result<DemoHeader> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let mut magic = [0u8; 8];
    input.read_exact(&mut magic)?;
    if &magic != DEMO_MAGIC {
        return Err(bad("not a soldank demo"));
    }
    let header: DemoHeader = read_record(&mut input)?
        .and_then(|bytes| decode(&bytes))
        .ok_or_else(|| bad("bad demo header"))?;
    if header.protocol != PROTOCOL_VERSION {
        return Err(bad(&format!(
            "recorded with protocol {} (this game has {PROTOCOL_VERSION})",
            header.protocol
        )));
    }
    Ok(header)
}

/// Reads a demo; a last frame cut short (the game quit while recording) is left out.
pub fn read_demo(mut input: impl Read) -> io::Result<Demo> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let header = read_demo_header(&mut input)?;
    let mut last = last_snapshot(&header.start);
    let mut frames = Vec::new();
    while let Some(bytes) = read_record(&mut input)? {
        let Some(mut frame) = decode::<DemoFrame>(&bytes) else {
            break;
        };
        for message in &mut frame.messages {
            let snapshot = match message {
                ServerMessage::SnapshotDelta(delta) => match &last {
                    Some(base) if base.tick == delta.base => delta.apply(base),
                    _ => return Err(bad("a snapshot without the one before it")),
                },
                ServerMessage::Snapshot(snapshot) => snapshot.clone(),
                _ => continue,
            };
            *message = ServerMessage::Snapshot(snapshot.clone());
            last = Some(snapshot);
        }
        frames.push(frame);
    }
    Ok(Demo { header, frames })
}

/// Snapshots as deltas from the one before (the first one whole).
fn deltas(last: &mut Option<Snapshot>, messages: &mut [ServerMessage]) {
    for message in messages {
        let ServerMessage::Snapshot(snapshot) = message else {
            continue;
        };
        if let Some(base) = last.replace(snapshot.clone()) {
            let delta = SnapshotDelta::between(&base, snapshot);
            *message = ServerMessage::SnapshotDelta(delta);
        }
    }
}

fn last_snapshot(messages: &[ServerMessage]) -> Option<Snapshot> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Snapshot(snapshot) => Some(snapshot.clone()),
        _ => None,
    })
}

fn write_record(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    out.write_all(&(bytes.len() as u32).to_le_bytes())?;
    out.write_all(bytes)
}

/// The next record; `None` at the end (or a record cut short).
fn read_record(input: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match input.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let mut bytes = vec![0; u32::from_le_bytes(len) as usize];
    match input.read_exact(&mut bytes) {
        Ok(()) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demos_read_back() {
        let header = DemoHeader {
            protocol: PROTOCOL_VERSION,
            map: "ctf_Ash".into(),
            map_hash: 7,
            cvars: vec![("sv_gamemode".into(), "3".into())],
            weapons_mods: [Some("[Info]".into()), None],
            game_mod: Some(GameMod {
                name: "dm".into(),
                hash: 7,
            }),
            you: Some(2),
            start: vec![ServerMessage::MatchEnded],
            date: 0,
            local: false,
        };
        let mut writer = DemoWriter::new(Vec::new(), &header).unwrap();
        let mut frame = DemoFrame {
            messages: vec![ServerMessage::Paused(true)],
            ..DemoFrame::default()
        };
        let input = Input {
            buttons: Buttons::FIRE | Buttons::LEFT,
            aim: Vec2::new(10.0, -5.0),
        };
        frame.set_input(Some(&input));
        // single player's actions and checks go along
        frame.actions = vec![
            LocalAction::Map("ctf_Ash".into()),
            LocalAction::AddBot {
                file: "Dutch".into(),
                team: 2,
            },
            LocalAction::Cvars(vec![("sv_gravity".into(), "0.1".into())]),
        ];
        frame.check = Some(0x1234_5678_9abc);
        writer.frame(&frame).unwrap();
        writer.frame(&DemoFrame::default()).unwrap();
        let mut bytes = writer.finish().unwrap();

        let demo = read_demo(bytes.as_slice()).unwrap();
        assert_eq!(demo.header, header);
        assert_eq!(read_demo_header(bytes.as_slice()).unwrap(), header);
        assert_eq!(demo.frames, [frame.clone(), DemoFrame::default()]);
        assert_eq!(demo.frames[0].input(), Some(input));

        // cut short: the whole frames are there
        bytes.truncate(bytes.len() - 2);
        assert_eq!(read_demo(bytes.as_slice()).unwrap().frames, [frame]);
        assert!(read_demo(&b"SOLDATDM"[..]).is_err());
    }

    #[test]
    fn snapshots_are_kept_as_deltas() {
        let soldier = |num, x| SoldierState {
            num,
            buttons: 0,
            aim: [0.0, 0.0],
            pos: [x, 100.0],
            velocity: [1.0, 0.0],
            health: 150.0,
            vest: 0.0,
            dead: false,
            jets: 100,
            weapons: [(2, 30), (10, 12), (19, 2)],
            active_weapon: 0,
            kills: 0,
            deaths: 0,
            flags: 0,
            bonus: 0,
            bonus_time: 0,
            ping: 50,
            ping_ticks: 3,
            stat: None,
            weapon_sel: 0x3ff,
            position: 1,
            look: 0,
            respawn_counter: 0,
            quality: 100,
        };
        let snapshot = |tick: u64| Snapshot {
            tick,
            soldiers: (1..=8)
                .map(|n| soldier(n, tick as f32 + f32::from(n)))
                .collect(),
            things: Vec::new(),
            thing_snapshot: true,
            dead_snapshot: true,
            team_scores: [0; 6],
            time_left: 3600,
        };
        let header = DemoHeader {
            protocol: PROTOCOL_VERSION,
            map: "ctf_Ash".into(),
            map_hash: 7,
            cvars: Vec::new(),
            weapons_mods: [None, None],
            game_mod: None,
            you: Some(1),
            start: vec![ServerMessage::Snapshot(snapshot(10))],
            date: 0,
            local: false,
        };
        let frames: Vec<DemoFrame> = (11..200)
            .map(|tick| DemoFrame {
                messages: vec![
                    ServerMessage::Snapshot(snapshot(tick)),
                    ServerMessage::VoteOff,
                ],
                ..DemoFrame::default()
            })
            .collect();
        let mut writer = DemoWriter::new(Vec::new(), &header).unwrap();
        for frame in &frames {
            writer.frame(frame).unwrap();
        }
        let bytes = writer.finish().unwrap();
        let whole: usize = frames.iter().map(|f| encode(f).len() + 4).sum();
        assert!(bytes.len() * 3 < whole, "{} vs {whole}", bytes.len());
        let demo = read_demo(bytes.as_slice()).unwrap();
        assert_eq!(demo.header, header);
        assert_eq!(demo.frames, frames);
    }
}
