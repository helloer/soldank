//! Playing on a server (`--connect`): the renet connection, and the protocol state that keeps
//! the client's world in line with the server's.

use renet::{ConnectionConfig, RenetClient};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};
use soldank_core::net::*;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, SystemTime};

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
}

impl Connection {
    /// Starts connecting to `server` (`host:port`, the port defaults to 23073).
    pub fn connect(server: &str) -> anyhow::Result<Connection> {
        let with_port = if server.contains(':') {
            server.to_string()
        } else {
            format!("{server}:{DEFAULT_PORT}")
        };
        let addr = with_port
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow::anyhow!("no address for {server}"))?;
        let socket = UdpSocket::bind(if addr.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        })?;
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
        let authentication = ClientAuthentication::Unsecure {
            protocol_id: PROTOCOL_ID,
            client_id: now.as_nanos() as u64,
            server_addr: addr,
            user_data: None,
        };
        let transport = NetcodeClientTransport::new(now, authentication, socket)?;
        Ok(Connection {
            renet: RenetClient::new(ConnectionConfig::default()),
            transport,
            net: NetClient::default(),
            server: addr,
            greeted: false,
            password: String::new(),
        })
    }

    pub fn with_password(mut self, password: &str) -> Connection {
        self.password = password.to_string();
        self
    }

    /// Time passed: packets in.
    pub fn update(&mut self, elapsed: Duration) -> anyhow::Result<()> {
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
            rtt: info.rtt,
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
