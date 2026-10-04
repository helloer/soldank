//! Remote admin (`Rcon.pas`): a line protocol over TCP on the game's port, for admin tools.
//! The server greets, the client sends `sv_adminpassword`, then `/command` lines run as the
//! server's console, other lines are said to the admins, `REFRESHX` asks for the game's state
//! (a packed binary record) and `SHUTDOWN` stops the server. Admins see the server's console.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc as SharedArc, Mutex};
use std::time::{Duration, Instant};

/// `RCON_AUTH_TIMEOUT`
const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
/// `net_maxadminconnections`, `net_rcon_burst` and `net_rcon_limit`'s defaults: connections,
/// and new ones a second (in bursts of so many).
const MAX_CONNECTIONS: usize = 20;
const CONNECTS_PER_SECOND: u32 = 5;
const CONNECT_BURST: u32 = 10;

/// What an admin asked for.
#[derive(Debug, PartialEq)]
pub enum Request {
    /// A line: `/command`, or something to say.
    Line(String),
    /// `REFRESHX`: the state packet back to this connection.
    Refresh,
    Shutdown,
}

/// An admin connection, its writer.
struct Admin {
    id: u64,
    ip: IpAddr,
    out: Sender<Vec<u8>>,
}

/// The remote admin server: requests in, the console out.
pub struct Rcon {
    requests: Receiver<(u64, IpAddr, Request)>,
    admins: SharedArc<Mutex<Vec<Admin>>>,
    pub addr: SocketAddr,
}

impl Rcon {
    /// Listens on `bind` (TCP) for admins with `password`.
    pub fn start(bind: SocketAddr, password: String) -> std::io::Result<Rcon> {
        let listener = TcpListener::bind(bind)?;
        let addr = listener.local_addr()?;
        let (requests_tx, requests) = channel();
        let admins: SharedArc<Mutex<Vec<Admin>>> = SharedArc::default();
        let connections = SharedArc::new(AtomicU64::new(0));
        let shared = admins.clone();
        std::thread::spawn(move || {
            let mut bucket = (CONNECT_BURST, Instant::now());
            for stream in listener.incoming().map_while(Result::ok) {
                // a token bucket against floods of connections
                let refill = bucket.1.elapsed().as_secs() as u32;
                if refill > 0 {
                    bucket = (
                        (bucket.0 + refill * CONNECTS_PER_SECOND).min(CONNECT_BURST),
                        Instant::now(),
                    );
                }
                let open = connections.load(Ordering::Relaxed) as usize;
                if bucket.0 == 0 || open >= MAX_CONNECTIONS {
                    tracing::debug!("[RCON] connection refused");
                    continue;
                }
                bucket.0 -= 1;
                let (requests, admins, connections) =
                    (requests_tx.clone(), shared.clone(), connections.clone());
                let password = password.clone();
                connections.fetch_add(1, Ordering::Relaxed);
                std::thread::spawn(move || {
                    if let Err(error) = serve(stream, &password, &requests, &admins) {
                        tracing::debug!(%error, "[RCON]");
                    }
                    connections.fetch_sub(1, Ordering::Relaxed);
                });
            }
        });
        Ok(Rcon {
            requests,
            admins,
            addr,
        })
    }

    /// The requests since the last call: which connection, from where, what.
    pub fn poll(&self) -> Vec<(u64, IpAddr, Request)> {
        self.requests.try_iter().collect()
    }

    /// A console line to every admin (`BroadcastMsg`).
    pub fn broadcast(&self, line: &str) {
        let bytes = format!("{line}\r\n").into_bytes();
        let mut admins = self.admins.lock().unwrap_or_else(|e| e.into_inner());
        admins.retain(|admin| admin.out.send(bytes.clone()).is_ok());
    }

    /// Bytes to one connection.
    pub fn send(&self, id: u64, bytes: Vec<u8>) {
        let admins = self.admins.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(admin) = admins.iter().find(|a| a.id == id) {
            let _ = admin.out.send(bytes);
        }
    }

    /// The admins' addresses.
    pub fn admin_ips(&self) -> Vec<IpAddr> {
        let admins = self.admins.lock().unwrap_or_else(|e| e.into_inner());
        admins.iter().map(|a| a.ip).collect()
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// One connection: the password, then requests until it closes.
fn serve(
    stream: TcpStream,
    password: &str,
    requests: &Sender<(u64, IpAddr, Request)>,
    admins: &Mutex<Vec<Admin>>,
) -> std::io::Result<()> {
    let ip = stream.peer_addr()?.ip();
    tracing::debug!("[RCON] New connection from: {ip}");
    let mut writer = stream.try_clone()?;
    writer.write_all(b"OpenSoldat Admin Connection Established.\r\n")?;
    stream.set_read_timeout(Some(AUTH_TIMEOUT))?;
    let mut lines = BufReader::new(stream.try_clone()?).lines();
    let given = match lines.next() {
        Some(Ok(line)) => line,
        Some(Err(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            tracing::debug!("[RCON] Password request timed out ({ip}).");
            writer.write_all(b"Password request timed out.\r\n")?;
            return Ok(());
        }
        _ => return Ok(()),
    };
    if given.trim_end_matches('\r') != password {
        writer.write_all(b"Invalid password.\r\n")?;
        tracing::info!("[RCON] Invalid password from: {ip}");
        return Ok(());
    }
    stream.set_read_timeout(None)?;
    for line in [
        "Welcome, you are in command of the server now.",
        "List of commands available in the OpenSoldat game Manual.",
        &format!("Server Version: {}", env!("CARGO_PKG_VERSION")),
    ] {
        writer.write_all(format!("{line}\r\n").as_bytes())?;
    }
    tracing::debug!("[RCON] Admin connected ({ip}).");

    // writes come from the game's thread through this connection's queue
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (out, queue) = channel::<Vec<u8>>();
    admins
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Admin { id, ip, out });
    std::thread::spawn(move || {
        for bytes in queue {
            if writer.write_all(&bytes).is_err() {
                break;
            }
        }
        let _ = writer.shutdown(std::net::Shutdown::Both);
    });

    for line in lines.map_while(Result::ok) {
        let line = line.trim_end_matches('\r').to_string();
        let request = match line.as_str() {
            "" => continue,
            "REFRESHX" => Request::Refresh,
            "SHUTDOWN" => Request::Shutdown,
            _ => Request::Line(line),
        };
        if requests.send((id, ip, request)).is_err() {
            break;
        }
    }
    admins
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|admin| admin.id != id);
    tracing::debug!("[RCON] Admin disconnected ({ip}).");
    Ok(())
}

/// Pascal's `string[n]`: a length byte, then `n` bytes.
fn short_string(out: &mut Vec<u8>, text: &str, n: usize) {
    let bytes: Vec<u8> = text.bytes().take(n).collect();
    out.push(bytes.len() as u8);
    out.extend_from_slice(&bytes);
    out.resize(out.len() + n - bytes.len(), 0);
}

impl ServerGame {
    /// `TMsg_RefreshX`: the players (in the frags order), flags, scores and settings, packed
    /// like Soldat's record.
    pub fn refresh_x(&self) -> Vec<u8> {
        let mut ranked: Vec<_> = self
            .players
            .iter()
            .filter_map(|(&num, &id)| Some(ranking(num, self.world.soldiers.get(id)?)))
            .collect();
        ranked.sort_by_key(|&(num, ..)| num);
        sort_ranked(&mut ranked);
        let players: Vec<(PlayerNum, &Soldier)> = ranked
            .iter()
            .map(|&(num, ..)| (num, &self.world.soldiers[self.players[&num]]))
            .collect();
        let slot = |i: usize| players.get(i).copied();

        let mut out = b"REFRESHX\r\n".to_vec();
        for i in 0..MAX_PLAYERS {
            short_string(&mut out, slot(i).map_or("", |(_, s)| &s.name), 24);
        }
        for _ in 0..MAX_PLAYERS {
            short_string(&mut out, "", 11);
        }
        for i in 0..MAX_PLAYERS {
            out.push(slot(i).map_or(255, |(_, s)| s.team as u8));
        }
        for i in 0..MAX_PLAYERS {
            out.extend((slot(i).map_or(0, |(_, s)| s.kills) as u16).to_le_bytes());
        }
        for i in 0..MAX_PLAYERS {
            out.push(slot(i).map_or(0, |(_, s)| s.flags) as u8);
        }
        for i in 0..MAX_PLAYERS {
            out.extend((slot(i).map_or(0, |(_, s)| s.deaths) as u16).to_le_bytes());
        }
        for i in 0..MAX_PLAYERS {
            // `PingTime`: the ping in ticks, as milliseconds
            let ping = slot(i).map_or(0, |(_, s)| i32::from(s.ping_ticks) * 1000 / 60);
            out.extend(ping.to_le_bytes());
        }
        for i in 0..MAX_PLAYERS {
            out.push(slot(i).map_or(0, |(num, _)| num));
        }
        for i in 0..MAX_PLAYERS {
            let ip = slot(i)
                .filter(|(_, s)| s.brain.is_none())
                .and_then(|(num, _)| self.clients.values().find(|c| c.num == Some(num))?.ip);
            let v4 = match ip {
                Some(IpAddr::V4(v4)) => v4.octets(),
                Some(IpAddr::V6(v6)) => v6.to_ipv4_mapped().map_or([0; 4], |v4| v4.octets()),
                None => [0; 4],
            };
            out.extend(v4);
        }
        let at = |s: &Soldier| {
            if s.dead_meat {
                Vec2::ZERO
            } else {
                s.skeleton.pos(1)
            }
        };
        for i in 0..MAX_PLAYERS {
            out.extend(slot(i).map_or(0.0, |(_, s)| at(s).x).to_le_bytes());
        }
        for i in 0..MAX_PLAYERS {
            out.extend(slot(i).map_or(0.0, |(_, s)| at(s).y).to_le_bytes());
        }
        for kind in [ThingKind::AlphaFlag, ThingKind::BravoFlag] {
            let flag = self
                .world
                .things
                .iter()
                .find(|t| t.active && t.kind == kind);
            let pos = flag.map_or(Vec2::ZERO, |t| t.skeleton.pos(1));
            out.extend(pos.x.to_le_bytes());
            out.extend(pos.y.to_le_bytes());
        }
        for team in 1..=4 {
            out.extend((self.world.game.team_scores[team] as u16).to_le_bytes());
        }
        short_string(&mut out, &self.map_name(), 16);
        out.extend((self.cvars.int("sv_timelimit") as i32).to_le_bytes());
        out.extend(self.world.game.time_left.to_le_bytes());
        out.extend((self.world.config.kill_limit as u16).to_le_bytes());
        out.push(self.cvars.int("sv_gamemode") as u8);
        out.push(self.cvars.int("sv_maxplayers") as u8);
        out.push(self.cvars.int("sv_maxspectators") as u8);
        out.push(u8::from(!self.cvars.string("sv_password").is_empty()));
        short_string(&mut out, &self.listed_next_map(), 16);
        out
    }
}

/// `SizeOf(TMsg_RefreshX)`
pub const REFRESHX_SIZE: usize = 10
    + 32 * 25
    + 32 * 12
    + 32
    + 64
    + 32
    + 64
    + 128
    + 32
    + 128
    + 128
    + 128
    + 16
    + 8
    + 17
    + 8
    + 2
    + 4
    + 17;
