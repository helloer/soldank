//! Playing on a server from the browser: renet2's WebTransport client socket, over the page's own
//! WebTransport (`web/webtransport.js`: miniquad's loader hosts no wasm-bindgen). The server
//! listens on the port after the game's; the connection opens with netcode's connection request
//! in its URL (`creq`, like renet2's), then every packet is a datagram. (netcode's random
//! numbers come from egui-miniquad's `getrandom` source; WebTransport does the encryption.)

use renet2_netcode::{ClientSocket, NetcodeTransportError, WebServerDestination};
use soldank_core::net::DEFAULT_PORT;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn soldank_wt_open(url: *const u8, url_len: usize, info: *const u8, info_len: usize) -> i32;
    fn soldank_wt_state(id: i32) -> i32;
    fn soldank_wt_send(id: i32, ptr: *const u8, len: usize);
    fn soldank_wt_recv(id: i32, ptr: *mut u8, cap: usize) -> i32;
    fn soldank_wt_close(id: i32);
}

/// A connection's state as the page has it.
const CLOSED: i32 = 2;
/// The page's answer when it has no datagram.
const NO_DATAGRAM: i32 = -1;
/// The URL's key for netcode's connection request (renet2's `HTTP_CONNECT_REQ`).
const CONNECT_REQUEST: &str = "creq";

/// A WebTransport connection to a server, opened with netcode's first connection request.
#[derive(Debug)]
pub struct WebTransportSocket {
    /// `https://host:port/`, and where its certificate's hash is (`http://host:port/`).
    url: String,
    info: String,
    /// The server as netcode knows it (made from the URL).
    server: SocketAddr,
    /// The page's connection, once open.
    id: Option<i32>,
    closed: bool,
}

impl WebTransportSocket {
    /// To `server` (`host[:port]`, the game's port: WebTransport is on the next one).
    pub fn new(server: &str) -> anyhow::Result<WebTransportSocket> {
        let (host, port) = split_host_port(server)?;
        let web_port = port
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("no port after {port}"))?;
        let url = url::Url::parse(&format!("https://{host}:{web_port}/"))?;
        Ok(WebTransportSocket {
            server: SocketAddr::from(WebServerDestination::Url(url.clone())),
            info: format!("http://{host}:{web_port}/"),
            url: url.to_string(),
            id: None,
            closed: false,
        })
    }

    pub fn server(&self) -> SocketAddr {
        self.server
    }

    /// Opens the connection with netcode's connection request in its URL.
    fn open(&mut self, request: &[u8]) {
        let url = format!("{}?{CONNECT_REQUEST}={}", self.url, percent_encode(request));
        // SAFETY: the strings are read during the call
        let id = unsafe {
            soldank_wt_open(url.as_ptr(), url.len(), self.info.as_ptr(), self.info.len())
        };
        self.id = Some(id);
    }
}

impl Drop for WebTransportSocket {
    fn drop(&mut self) {
        self.close();
    }
}

impl ClientSocket for WebTransportSocket {
    /// WebTransport encrypts (netcode doesn't, like renet2's server socket).
    fn is_encrypted(&self) -> bool {
        true
    }

    fn is_reliable(&self) -> bool {
        false
    }

    fn addr(&self) -> std::io::Result<SocketAddr> {
        Err(Error::from(ErrorKind::AddrNotAvailable))
    }

    fn is_closed(&mut self) -> bool {
        // SAFETY: a plain call
        self.closed
            || self
                .id
                .is_some_and(|id| unsafe { soldank_wt_state(id) } == CLOSED)
    }

    fn close(&mut self) {
        if let Some(id) = self.id.take() {
            // SAFETY: a plain call
            unsafe { soldank_wt_close(id) };
        }
        self.closed = true;
    }

    fn preupdate(&mut self) {}

    fn try_recv(&mut self, buffer: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        if self.is_closed() {
            return Err(Error::from(ErrorKind::ConnectionAborted));
        }
        let Some(id) = self.id else {
            return Err(Error::from(ErrorKind::WouldBlock));
        };
        // SAFETY: the page writes at most `buffer.len()` bytes
        match unsafe { soldank_wt_recv(id, buffer.as_mut_ptr(), buffer.len()) } {
            NO_DATAGRAM => Err(Error::from(ErrorKind::WouldBlock)),
            n if n >= 0 => Ok((n as usize, self.server)),
            _ => Err(Error::from(ErrorKind::InvalidData)),
        }
    }

    fn postupdate(&mut self) {}

    fn send(&mut self, addr: SocketAddr, packet: &[u8]) -> Result<(), NetcodeTransportError> {
        if self.is_closed() {
            return Err(Error::from(ErrorKind::ConnectionAborted).into());
        }
        if addr != self.server {
            return Err(Error::from(ErrorKind::AddrNotAvailable).into());
        }
        match self.id {
            // the first packet that counts is netcode's connection request (type 0)
            None if packet.first().is_some_and(|prefix| prefix & 0x0f == 0) => self.open(packet),
            None => {}
            // SAFETY: the page copies the bytes during the call (dropped until it's open)
            Some(id) => unsafe { soldank_wt_send(id, packet.as_ptr(), packet.len()) },
        }
        Ok(())
    }
}

/// `host[:port]` (an IPv6 host in brackets), the port 23073 by default.
fn split_host_port(server: &str) -> anyhow::Result<(String, u16)> {
    let server = server.trim();
    let (host, port) = match server.rsplit_once(':') {
        // a bare IPv6 address has colons but no port
        Some((host, port)) if !host.contains(':') || host.ends_with(']') => (host, Some(port)),
        _ => (server, None),
    };
    if host.is_empty() {
        anyhow::bail!("no server address");
    }
    let port = match port {
        Some(port) => port
            .parse()
            .map_err(|_| anyhow::anyhow!("bad port {port}"))?,
        None => DEFAULT_PORT,
    };
    Ok((host.to_string(), port))
}

/// Bytes for a URL's query: unreserved characters as they are, the rest `%XX`.
fn percent_encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 3);
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                text.push(byte as char);
            }
            _ => text.push_str(&format!("%{byte:02X}")),
        }
    }
    text
}
