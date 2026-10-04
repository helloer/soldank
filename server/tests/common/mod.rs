//! Shared by the network tests: in-memory clients with their own worlds, links that can
//! delay and lose packets, and a match driving it all.
#![allow(dead_code)]

use renet2::{ClientId, ConnectionConfig, RenetClient, RenetServer};
use soldank_core::assets::Vfs;
use soldank_core::config::Cvars;
use soldank_core::demo::DemoFrame;
use soldank_core::net::*;
use soldank_core::*;
use soldank_server::ServerGame;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub fn vfs() -> Option<Vfs> {
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

#[derive(Debug, Default, Clone, Copy)]
pub struct SnapshotBytes {
    pub full: usize,
    pub full_bytes: usize,
    pub deltas: usize,
    pub delta_bytes: usize,
    pub expanded_bytes: usize,
}

/// A client with its own world, like the game's.
pub struct TestClient {
    pub renet: RenetClient,
    pub net: NetClient,
    pub world: Option<World>,
    data: Arc<GameData>,
    pub vfs: Vfs,
    pub notices: Vec<Notice>,
    /// The server's cvars and weapons mods from `Welcome`.
    cvars: Vec<(String, String)>,
    weapons_mods: [Option<String>; 2],
    /// Getting the server's map, until it's ready.
    download: Option<MapDownload>,
    /// Plays with the server's mod (`cl_servermods`).
    pub servermods: bool,
    /// The server's mod in use, and where its archive is kept.
    pub game_mod: Option<GameMod>,
    mods_dir: Option<tempfile::TempDir>,
    /// Getting the server's mod, and the map to get after it.
    mod_download: Option<(ModDownload, String, u64)>,
    /// Files that came from the server.
    pub downloaded: Vec<String>,
    /// Snapshots that came whole and as deltas: how many, their bytes, and the bytes of the
    /// deltas made whole.
    pub snapshots: SnapshotBytes,
    /// Recording, like the game's demos: the frames so far, and this tick's messages.
    pub demo: Option<Vec<DemoFrame>>,
    pending: DemoFrame,
}

impl TestClient {
    pub fn new(server: &mut RenetServer, id: u64, vfs: Vfs, data: Arc<GameData>) -> TestClient {
        TestClient {
            renet: server.new_local_client(id),
            net: NetClient::default(),
            world: None,
            data,
            vfs,
            notices: Vec::new(),
            cvars: Vec::new(),
            weapons_mods: [None, None],
            download: None,
            servermods: true,
            game_mod: None,
            mods_dir: None,
            mod_download: None,
            downloaded: Vec::new(),
            snapshots: SnapshotBytes::default(),
            demo: None,
            pending: DemoFrame::default(),
        }
    }

    pub fn send(&mut self, message: &ClientMessage) {
        let channel = match message {
            ClientMessage::Control(_) => channel::UNRELIABLE,
            _ => channel::RELIABLE,
        };
        self.renet.send_message(channel, encode(message));
    }

    /// Takes the server's messages into the world, then steps it with `input`.
    pub fn tick(&mut self, input: Option<Input>) {
        self.receive();
        let Some(world) = self.world.as_mut() else {
            return;
        };
        let inputs: Vec<_> = match (self.net.own(), input) {
            (Some(own), Some(input)) => vec![(own, input)],
            _ => Vec::new(),
        };
        if let Some(message) = input.and_then(|input| self.net.control(world, &input)) {
            self.renet
                .send_message(channel::UNRELIABLE, encode(&message));
        }
        world.step(&inputs);
        for bullet in world.net_bullets.iter_mut().flat_map(std::mem::take) {
            let message = ClientMessage::Bullet(bullet.state());
            self.renet.send_message(channel::RELIABLE, encode(&message));
        }
        if let Some(frames) = &mut self.demo {
            let mut frame = std::mem::take(&mut self.pending);
            frame.set_input(inputs.first().map(|(_, input)| input));
            frames.push(frame);
        }
    }

    /// The server's messages into the world.
    pub fn receive(&mut self) {
        for channel in [channel::RELIABLE, channel::UNRELIABLE] {
            while let Some(bytes) = self.renet.receive_message(channel) {
                let message: ServerMessage = decode(&bytes).expect("a server message");
                let delta = matches!(message, ServerMessage::SnapshotDelta(_));
                // deltas made whole (and recorded so)
                let Some(message) = self.net.expand(message) else {
                    continue;
                };
                match (&message, delta) {
                    (ServerMessage::Snapshot(_), false) => {
                        self.snapshots.full += 1;
                        self.snapshots.full_bytes += bytes.len();
                    }
                    (ServerMessage::Snapshot(_), true) => {
                        self.snapshots.deltas += 1;
                        self.snapshots.delta_bytes += bytes.len();
                        self.snapshots.expanded_bytes += encode(&message).len();
                    }
                    _ => {}
                }
                if self.demo.is_some() {
                    self.pending.messages.push(message.clone());
                }
                let message = match self.net.apply_session(message) {
                    Ok(notice) => {
                        self.session(&notice);
                        self.notices.push(notice);
                        continue;
                    }
                    Err(message) => message,
                };
                if let Some(world) = self.world.as_mut()
                    && let Some(notice) = self.net.apply(world, message)
                {
                    self.notices.push(notice);
                }
            }
        }
    }

    /// Like the game: the map is loaded (downloaded first if need be), then the client's ready.
    fn session(&mut self, notice: &Notice) {
        let requests = match notice {
            Notice::Welcome {
                map,
                map_hash,
                cvars,
                weapons_mods,
                game_mod,
            } => {
                self.cvars = cvars.clone();
                self.weapons_mods = weapons_mods.clone();
                match game_mod.clone().filter(|_| self.servermods) {
                    Some(game_mod) => {
                        let (download, request) = ModDownload::new(game_mod);
                        self.mod_download = Some((download, map.clone(), *map_hash));
                        vec![request]
                    }
                    None => self.start(map, *map_hash),
                }
            }
            Notice::MapChange { map, map_hash } => {
                self.world = None;
                self.net.reset();
                match self.mod_download.as_mut() {
                    Some((_, next, hash)) => {
                        (*next, *hash) = (map.clone(), *map_hash);
                        Vec::new()
                    }
                    None => self.start(map, *map_hash),
                }
            }
            Notice::FileChunk {
                request,
                path,
                size,
                offset,
                data,
            } if self
                .mod_download
                .as_ref()
                .is_some_and(|(download, ..)| download.wants(request)) =>
            {
                let (download, ..) = self.mod_download.as_mut().unwrap();
                let whole = download.chunk(path, *size, *offset, data);
                assert_eq!(download.failed, None);
                match whole {
                    Some(bytes) => {
                        let (download, map, map_hash) = self.mod_download.take().unwrap();
                        self.use_mod(download.game_mod, &bytes);
                        self.downloaded.push(path.clone());
                        self.start(&map, map_hash)
                    }
                    None => Vec::new(),
                }
            }
            Notice::FileChunk {
                request,
                path,
                size,
                offset,
                data,
            } => {
                let Some(download) = self.download.as_mut() else {
                    return;
                };
                let (whole, requests) =
                    download.chunk(&mut self.vfs, request, path, *size, *offset, data);
                self.downloaded.extend(whole.map(|(path, _)| path));
                requests
            }
            Notice::NoFile(request) => {
                if let Some(download) = self.download.as_mut() {
                    download.no_file(request);
                }
                Vec::new()
            }
            // the game plays by them at once
            Notice::Cvars(cvars) => {
                for (cvar, value) in cvars {
                    match self.cvars.iter_mut().find(|(c, _)| c == cvar) {
                        Some(known) => known.1 = value.clone(),
                        None => self.cvars.push((cvar.clone(), value.clone())),
                    }
                }
                if let Some(world) = self.world.as_mut() {
                    world.set_rules(rules(&self.cvars, &world.data));
                }
                Vec::new()
            }
            Notice::Weapons(mods) => {
                self.weapons_mods = mods.clone();
                if let Some(world) = self.world.as_mut() {
                    world.data = Arc::new(self.data.with_weapons_mods(mods));
                    world.set_rules(rules(&self.cvars, &world.data));
                    world.reapply_weapons();
                }
                Vec::new()
            }
            _ => Vec::new(),
        };
        for request in requests {
            self.send(&request);
        }
        if let Some(download) = &self.download {
            assert_eq!(download.failed, None);
        }
        if self.download.as_ref().is_some_and(MapDownload::done) {
            let map = self.download.take().unwrap().map;
            let data = Arc::new(self.data.with_weapons_mods(&self.weapons_mods));
            self.world = Some(client_world(&self.vfs, &data, &map, &self.cvars));
            let mod_hash = self.game_mod.as_ref().map_or(0, |game_mod| game_mod.hash);
            self.send(&ClientMessage::Ready { mod_hash });
        }
    }

    /// How far the server's mod has come, while it's on its way.
    pub fn mod_progress(&self) -> Option<(usize, usize)> {
        self.mod_download
            .as_ref()
            .map(|(download, ..)| download.progress())
    }

    /// The server's mod over the game's files, like the game: kept, mounted, loaded.
    fn use_mod(&mut self, game_mod: GameMod, bytes: &[u8]) {
        let dir = self
            .mods_dir
            .get_or_insert_with(|| tempfile::tempdir().unwrap());
        let path = dir.path().join(format!("{}.smod", game_mod.name));
        std::fs::write(&path, bytes).unwrap();
        self.vfs.mount(&path).unwrap();
        self.data = Arc::new(GameData::load(&self.vfs).unwrap());
        self.game_mod = Some(game_mod);
    }

    fn start(&mut self, map: &str, map_hash: u64) -> Vec<ClientMessage> {
        let (download, requests) = MapDownload::new(&self.vfs, map, map_hash);
        self.download = Some(download);
        requests
    }
}

/// The rules of a world with the server's `cvars`.
pub fn rules(cvars: &[(String, String)], data: &GameData) -> WorldConfig {
    let mut all = Cvars::new();
    register_cvars(&mut all);
    for (name, value) in cvars {
        all.set(name, value).unwrap();
    }
    WorldConfig::from_cvars(&all, data)
}

/// A client's world on the server's map, with its cvars.
pub fn client_world(
    vfs: &Vfs,
    data: &Arc<GameData>,
    map: &str,
    cvars: &[(String, String)],
) -> World {
    let mut all = Cvars::new();
    register_cvars(&mut all);
    for (name, value) in cvars {
        all.set(name, value).unwrap();
    }
    let map = MapFile::load(vfs, map).unwrap();
    let mut config = WorldConfig::from_cvars(&all, data);
    config.client = true;
    let mut world = World::new(data.clone(), map, config);
    world.net_bullets = Some(Vec::new());
    world
}

/// A recorded client's world again, from its frames alone (like a demo's playback).
pub fn replay(frames: &[DemoFrame], vfs: &Vfs, data: &Arc<GameData>) -> Option<World> {
    let mut net = NetClient::default();
    let mut world: Option<World> = None;
    for frame in frames {
        for message in &frame.messages {
            match net.apply_session(message.clone()) {
                Ok(Notice::Welcome {
                    map,
                    cvars,
                    weapons_mods,
                    ..
                }) => {
                    let data = Arc::new(data.with_weapons_mods(&weapons_mods));
                    world = Some(client_world(vfs, &data, &map, &cvars));
                }
                Ok(_) => {}
                Err(message) => {
                    if let Some(world) = world.as_mut() {
                        net.apply(world, message);
                    }
                }
            }
        }
        if let Some(world) = world.as_mut() {
            let inputs: Vec<_> = net.own().zip(frame.input()).into_iter().collect();
            world.step(&inputs);
        }
    }
    world
}

/// A `.smod` archive of these files.
pub fn smod(files: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (path, bytes) in files {
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(*path, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub fn hello(name: &str) -> ClientMessage {
    ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        password: String::new(),
        name: name.to_string(),
        looks: NetLooks {
            shirt: 0x8f8f8f,
            pants: 0x8f8f8f,
            skin: 0xe6b478,
            hair: 0,
            jet: 0,
            hair_style: 1,
            chain: 0,
            head_cap: 0,
        },
        team: None,
    }
}

/// The way between the server and a client: packets arrive `latency` ticks later, and
/// `loss` of them never.
pub struct Link {
    latency: u64,
    loss: u32,
    rng: u64,
    to_client: VecDeque<(u64, Vec<u8>)>,
    to_server: VecDeque<(u64, Vec<u8>)>,
}

impl Link {
    pub fn perfect() -> Link {
        Link::lossy(0, 0.0)
    }

    /// `latency` ticks each way, `loss` (0 to 1) of the packets lost.
    pub fn lossy(latency: u64, loss: f64) -> Link {
        Link {
            latency,
            loss: (loss * 10_000.0) as u32,
            rng: 0x9e37_79b9_7f4a_7c15,
            to_client: VecDeque::new(),
            to_server: VecDeque::new(),
        }
    }

    fn lost(&mut self) -> bool {
        // xorshift
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 10_000) < u64::from(self.loss)
    }

    /// What both sides sent goes in; what's due by `now` comes out.
    pub fn exchange(
        &mut self,
        now: u64,
        server: &mut RenetServer,
        id: ClientId,
        client: &mut RenetClient,
    ) {
        for packet in server.get_packets_to_send(id).unwrap_or_default() {
            if !self.lost() {
                self.to_client.push_back((now + self.latency, packet));
            }
        }
        for packet in client.get_packets_to_send() {
            if !self.lost() {
                self.to_server.push_back((now + self.latency, packet));
            }
        }
        while self.to_client.front().is_some_and(|(due, _)| *due <= now) {
            let (_, packet) = self.to_client.pop_front().unwrap();
            client.process_packet(&packet);
        }
        while self.to_server.front().is_some_and(|(due, _)| *due <= now) {
            let (_, packet) = self.to_server.pop_front().unwrap();
            let _ = server.process_packet_from(&packet, id);
        }
    }
}

/// Client `n`'s address: 10.0.0.n.
pub fn address(id: ClientId) -> Option<std::net::IpAddr> {
    Some(std::net::IpAddr::from([10, 0, 0, id as u8]))
}

/// A server with in-memory clients.
pub struct TestMatch {
    pub game: ServerGame,
    pub server: RenetServer,
    pub clients: Vec<(TestClient, Link)>,
    data: Arc<GameData>,
    now: u64,
    /// Clients connecting from elsewhere than [`address`].
    addresses: HashMap<ClientId, std::net::IpAddr>,
}

const DT: Duration = Duration::from_nanos(1_000_000_000 / 60);

impl TestMatch {
    /// A match on `map` with these server cvars, `None` without assets.
    pub fn new(map: &str, settings: &[(&str, &str)]) -> Option<TestMatch> {
        TestMatch::with_files(map, settings, Vec::new())
    }

    /// The same, with files only the server has.
    pub fn with_files(
        map: &str,
        settings: &[(&str, &str)],
        files: Vec<(&str, Vec<u8>)>,
    ) -> Option<TestMatch> {
        let mut vfs = vfs()?;
        for (path, bytes) in files {
            vfs.add(path, bytes);
        }
        let data = Arc::new(GameData::load(&vfs).unwrap());
        let mut cvars = Cvars::new();
        register_cvars(&mut cvars);
        // the tests play deathmatch unless they say otherwise
        cvars.set("sv_gamemode", "0").unwrap();
        for (name, value) in settings {
            cvars.set(name, value).unwrap();
        }
        let game = ServerGame::new(vfs, data, cvars, map).unwrap();
        // the clients' own data: without the server's files
        let data = Arc::new(GameData::load(&self::vfs()?).unwrap());
        Some(TestMatch {
            game,
            server: RenetServer::new(ConnectionConfig::test()),
            clients: Vec::new(),
            data,
            now: 0,
            addresses: HashMap::new(),
        })
    }

    /// A client says hello from `ip`; its index.
    pub fn join_from(&mut self, hello: ClientMessage, ip: [u8; 4]) -> usize {
        let id = self.clients.len() as u64 + 1;
        self.addresses.insert(id, ip.into());
        self.join(hello, Link::perfect())
    }

    /// A client says hello over `link`; its index.
    pub fn join(&mut self, hello: ClientMessage, link: Link) -> usize {
        let id = self.clients.len() as u64 + 1;
        let mut client = TestClient::new(&mut self.server, id, vfs().unwrap(), self.data.clone());
        client.send(&hello);
        self.clients.push((client, link));
        self.clients.len() - 1
    }

    fn exchange(&mut self) {
        for (i, (client, link)) in self.clients.iter_mut().enumerate() {
            link.exchange(self.now, &mut self.server, i as u64 + 1, &mut client.renet);
        }
    }

    /// One tick: the network, the server, then each client with its input.
    pub fn step(&mut self, inputs: &[Option<Input>]) {
        self.step_timed(inputs);
    }

    /// [`TestMatch::step`], saying how long the server took (its messages in, its tick).
    pub fn step_timed(&mut self, inputs: &[Option<Input>]) -> Duration {
        self.now += 1;
        self.server.update(DT);
        for (client, _) in &mut self.clients {
            client.renet.update(DT);
        }
        self.exchange();
        let start = std::time::Instant::now();
        let addresses = &self.addresses;
        self.game.receive(&mut self.server, |id| {
            addresses.get(&id).copied().or(address(id))
        });
        self.game.tick(&mut self.server);
        let server_time = start.elapsed();
        self.exchange();
        for (i, (client, _)) in self.clients.iter_mut().enumerate() {
            client.tick(inputs.get(i).copied().flatten());
        }
        server_time
    }

    /// Time passes for the network only (late and resent packets arrive); the clients take
    /// in what came.
    pub fn settle(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.now += 1;
            self.server.update(DT);
            for (client, _) in &mut self.clients {
                client.renet.update(DT);
            }
            self.exchange();
            let addresses = &self.addresses;
            self.game.receive(&mut self.server, |id| {
                addresses.get(&id).copied().or(address(id))
            });
            self.exchange();
        }
        for (client, _) in &mut self.clients {
            client.receive();
        }
    }

    /// Client `i` disconnects.
    pub fn leave(&mut self, i: usize) {
        let (client, _) = &mut self.clients[i];
        self.server
            .disconnect_local_client(i as u64 + 1, &mut client.renet);
    }

    /// `ticks` of standing still for everyone.
    pub fn settle_steps(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.step(&[]);
        }
    }

    /// The bot files the server has.
    pub fn game_bots(&self) -> Vec<String> {
        let vfs = vfs().unwrap();
        vfs.list("configs/bots")
            .into_iter()
            .filter_map(|f| f.strip_suffix(".bot").map(str::to_string))
            .filter(|name| !name.eq_ignore_ascii_case("dummy"))
            .collect()
    }

    pub fn client_mut(&mut self, i: usize) -> &mut TestClient {
        &mut self.clients[i].0
    }

    pub fn client(&self, i: usize) -> &TestClient {
        &self.clients[i].0
    }
}
