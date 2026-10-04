//! Server demos (`record`, `demo_autorecord`): a watcher on the server itself that hears what
//! a spectator would and keeps it like the game's demos (Soldat's "Demo Recorder" player,
//! whom the players don't see). The game plays them like its own.

use renet2::{ClientId, RenetClient, RenetServer};
use soldank_core::demo::*;
use soldank_core::net::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The recorder's name (`CreateDemoPlayer`).
const NAME: &str = "Demo Recorder";

pub struct Recorder {
    /// Its connection to the server (in memory).
    pub id: ClientId,
    client: RenetClient,
    net: NetClient,
    path: PathBuf,
    /// The demo's file name, for the console.
    pub name: String,
    /// Started by `demo_autorecord`: it ends with the map.
    pub auto: bool,
    /// The server's mod, for `Ready`.
    mod_hash: u64,
    /// Open once the welcome came (the demo's header is the match then).
    writer: Option<DemoWriter<BufWriter<File>>>,
    frame: DemoFrame,
}

impl Recorder {
    /// Joins as `id` (`password`: the server's) to record `demos/<name>.sdemo` in `config_dir`.
    pub fn start(
        renet: &mut RenetServer,
        id: ClientId,
        config_dir: &Path,
        name: &str,
        auto: bool,
        password: &str,
        mod_hash: u64,
    ) -> Recorder {
        let mut client = renet.new_local_client(id);
        let hello = ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            password: password.to_string(),
            name: NAME.to_string(),
            looks: NetLooks {
                shirt: 0,
                pants: 0,
                skin: 0,
                hair: 0,
                jet: 0,
                hair_style: 0,
                chain: 0,
                head_cap: 0,
            },
            team: None,
        };
        client.send_message(channel::RELIABLE, encode(&hello));
        let file = format!("{name}.{DEMO_EXTENSION}");
        Recorder {
            id,
            client,
            net: NetClient::default(),
            path: config_dir.join("demos").join(&file),
            name: file,
            auto,
            mod_hash,
            writer: None,
            frame: DemoFrame::default(),
        }
    }

    /// A tick of the server's: what it sent goes into the demo, the answers back.
    pub fn tick(&mut self, renet: &mut RenetServer, elapsed: Duration) -> std::io::Result<()> {
        for packet in renet.get_packets_to_send(self.id).unwrap_or_default() {
            self.client.process_packet(&packet);
        }
        self.client.update(elapsed);
        for channel in [channel::RELIABLE, channel::UNRELIABLE] {
            while let Some(bytes) = self.client.receive_message(channel) {
                if let Some(message) = decode::<ServerMessage>(&bytes) {
                    self.message(message)?;
                }
            }
        }
        if let Some(writer) = &mut self.writer {
            writer.frame(&std::mem::take(&mut self.frame))?;
        }
        for packet in self.client.get_packets_to_send() {
            let _ = renet.process_packet_from(&packet, self.id);
        }
        Ok(())
    }

    fn message(&mut self, message: ServerMessage) -> std::io::Result<()> {
        match message {
            // the demo starts: the match as the header, then everything that comes
            ServerMessage::Welcome {
                you,
                map,
                map_hash,
                cvars,
                weapons_mods,
                game_mod,
            } => {
                self.net.you = Some(you);
                let header = DemoHeader {
                    protocol: PROTOCOL_VERSION,
                    map,
                    map_hash,
                    cvars,
                    weapons_mods,
                    game_mod,
                    you: Some(you),
                    start: Vec::new(),
                    date: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs()),
                    local: false,
                };
                if let Some(dir) = self.path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let file = BufWriter::new(File::create(&self.path)?);
                self.writer = Some(DemoWriter::new(file, &header)?);
                self.ready();
            }
            // (the server has the map)
            ServerMessage::MapChange { .. } => {
                self.frame.messages.push(message);
                self.ready();
            }
            ServerMessage::FileChunk { .. } | ServerMessage::NoFile(_) => {}
            // whole snapshots, as the game keeps them
            message => {
                if let Some(message) = self.net.expand(message) {
                    self.frame.messages.push(message);
                }
            }
        }
        Ok(())
    }

    fn ready(&mut self) {
        let ready = ClientMessage::Ready {
            mod_hash: self.mod_hash,
        };
        self.client.send_message(channel::RELIABLE, encode(&ready));
    }

    /// The demo's end: the file's last frames out, the recorder gone.
    pub fn stop(mut self, renet: &mut RenetServer) -> std::io::Result<()> {
        renet.disconnect_local_client(self.id, &mut self.client);
        if let Some(writer) = self.writer.take() {
            use std::io::Write;
            writer.finish()?.flush()?;
        }
        Ok(())
    }
}
