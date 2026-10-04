//! Playing on a server (`--connect`): the renet2 connection (UDP, or WebTransport in the
//! browser), and the protocol state that keeps the client's world in line with the server's.

use renet2::{ConnectionConfig, RenetClient};
use renet2_netcode::{ClientAuthentication, NetcodeClientTransport};
use soldank_core::net::*;
use std::net::SocketAddr;
use std::time::Duration;

pub struct Connection {
    renet: RenetClient,
    transport: NetcodeClientTransport,
    /// Which soldier each player is, and how messages change the world.
    pub net: NetClient,
    pub server: SocketAddr,
    /// `Hello` went out.
    pub greeted: bool,
    /// The server's password.
    pub password: String,
    /// The last update's step: renet's round trip counts one of these (see `stats`).
    tick: Duration,
}

impl Connection {
    /// Starts connecting to `server` (`host:port`, the port defaults to 23073).
    pub fn connect(server: &str) -> anyhow::Result<Connection> {
        let now = crate::platform::since_epoch();
        let (addr, transport) = transport(server, now)?;
        let renet = RenetClient::new(ConnectionConfig::test(), transport.is_reliable());
        Ok(Connection {
            renet,
            transport,
            net: NetClient::default(),
            server: addr,
            greeted: false,
            password: String::new(),
            tick: Duration::ZERO,
        })
    }

    pub fn with_password(mut self, password: &str) -> Connection {
        self.password = password.to_string();
        self
    }

    /// Time passed: packets in.
    pub fn update(&mut self, elapsed: Duration) -> anyhow::Result<()> {
        self.tick = elapsed;
        self.renet.update(elapsed);
        self.transport.update(elapsed, &mut self.renet)?;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.renet.is_connected()
    }

    /// The connection's numbers, for the GameStats texts.
    pub fn stats(&self) -> crate::render::interface::NetStats {
        let info = self.renet.network_info();
        crate::render::interface::NetStats {
            // the round trip less the tick it waits to be read, about the network's (like
            // the server's ping for the scoreboard)
            rtt: (info.rtt - self.tick.as_secs_f64()).max(0.0),
            packet_loss: info.packet_loss,
            sent: info.bytes_sent_per_second,
            received: info.bytes_received_per_second,
        }
    }

    /// The connection is over (refused, kicked, timed out, the server gone).
    pub fn is_disconnected(&self) -> bool {
        self.renet.is_disconnected() || self.transport.disconnect_reason().is_some()
    }

    pub fn send(&mut self, message: &ClientMessage) {
        let channel = match message {
            ClientMessage::Control(_) => channel::UNRELIABLE,
            _ => channel::RELIABLE,
        };
        self.renet.send_message(channel, encode(message));
    }

    /// The server's messages since the last call, in order per channel.
    pub fn receive(&mut self) -> Vec<ServerMessage> {
        let mut messages = Vec::new();
        for channel in [channel::RELIABLE, channel::UNRELIABLE] {
            while let Some(bytes) = self.renet.receive_message(channel) {
                match decode::<ServerMessage>(&bytes) {
                    Some(message) => messages.push(message),
                    None => tracing::warn!("bad message from the server"),
                }
            }
        }
        messages
    }

    /// Packets out.
    pub fn flush(&mut self) {
        if let Err(error) = self.transport.send_packets(&mut self.renet) {
            tracing::warn!(%error, "network");
        }
    }

    pub fn disconnect(&mut self) {
        self.transport.disconnect();
    }
}

/// Unsecure netcode (no connect tokens): to the server's socket `socket_id` (0 its UDP, 1 the
/// browsers' WebTransport) at `addr`.
fn authentication(addr: SocketAddr, socket_id: u8, now: Duration) -> ClientAuthentication {
    ClientAuthentication::Unsecure {
        protocol_id: PROTOCOL_ID,
        client_id: now.as_nanos() as u64,
        socket_id,
        server_addr: addr,
        user_data: None,
    }
}

/// The server's address, and netcode to it over UDP.
#[cfg(not(target_arch = "wasm32"))]
fn transport(server: &str, now: Duration) -> anyhow::Result<(SocketAddr, NetcodeClientTransport)> {
    use std::net::ToSocketAddrs;
    let with_port = if server.contains(':') {
        server.to_string()
    } else {
        format!("{server}:{DEFAULT_PORT}")
    };
    let addr = with_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("no address for {server}"))?;
    let socket = std::net::UdpSocket::bind(if addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    })?;
    let socket = renet2_netcode::NativeSocket::new(socket)?;
    let transport = NetcodeClientTransport::new(now, authentication(addr, 0, now), socket)?;
    Ok((addr, transport))
}

/// In the browser: over WebTransport, to the port after the game's.
#[cfg(target_arch = "wasm32")]
fn transport(server: &str, now: Duration) -> anyhow::Result<(SocketAddr, NetcodeClientTransport)> {
    let socket = crate::web::webtransport::WebTransportSocket::new(server)?;
    let addr = socket.server();
    let transport = NetcodeClientTransport::new(now, authentication(addr, 1, now), socket)?;
    Ok((addr, transport))
}
