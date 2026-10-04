//! In the browser there is no playing on a server (renet speaks UDP): `--connect` is refused.
#![allow(dead_code)]

use soldank_core::net::*;
use std::net::SocketAddr;
use std::time::Duration;

pub struct Connection {
    pub net: NetClient,
    pub server: SocketAddr,
    pub greeted: bool,
    pub password: String,
}

impl Connection {
    pub fn connect(_server: &str) -> anyhow::Result<Connection> {
        anyhow::bail!("no playing on a server in the browser")
    }

    pub fn with_password(mut self, password: &str) -> Connection {
        self.password = password.to_string();
        self
    }

    pub fn update(&mut self, _elapsed: Duration) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        false
    }

    pub fn stats(&self) -> crate::render::interface::NetStats {
        Default::default()
    }

    pub fn is_disconnected(&self) -> bool {
        true
    }

    pub fn send(&mut self, _message: &ClientMessage) {}

    pub fn receive(&mut self) -> Vec<ServerMessage> {
        Vec::new()
    }

    pub fn flush(&mut self) {}

    pub fn disconnect(&mut self) {}
}
