//! Browsers' way in: the server's WebTransport socket and its certificate's hash over HTTP,
//! joined by a client that does what the page does (`web/webtransport.js`): netcode's connection
//! request in the URL, then datagrams. Needs the game assets, else the test is skipped.

mod common;

use common::*;
use renet2::{ConnectionConfig, RenetClient};
use renet2_netcode::{
    BoxedSocket, ClientAuthentication, ClientSocket, NetcodeClientTransport,
    NetcodeServerTransport, NetcodeTransportError, ServerAuthentication, ServerSetupConfig,
    WebServerDestination,
};
use soldank_core::net::*;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, SystemTime};

/// What the page does with a server's address: its certificate's hash first (plain HTTP on
/// the WebTransport port), then WebTransport with netcode's connection request in the URL,
/// opened in the background (the server accepts the session once its tick took the request).
#[derive(Debug)]
struct PageSocket {
    url: String,
    server: SocketAddr,
    hash: [u8; 32],
    runtime: tokio::runtime::Handle,
    /// The session on its way, then open; its datagrams.
    opening: Option<Mutex<mpsc::Receiver<wtransport::Connection>>>,
    connection: Option<wtransport::Connection>,
    incoming: Option<Mutex<mpsc::Receiver<Vec<u8>>>>,
}

impl PageSocket {
    /// The session, once open: its datagrams come through `incoming`.
    fn check_open(&mut self) {
        let Some(opening) = &self.opening else { return };
        let Ok(connection) = opening.lock().unwrap().try_recv() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let reader = connection.clone();
        self.runtime.spawn(async move {
            while let Ok(datagram) = reader.receive_datagram().await {
                if sender.send(datagram.payload().to_vec()).is_err() {
                    break;
                }
            }
        });
        self.connection = Some(connection);
        self.incoming = Some(Mutex::new(receiver));
        self.opening = None;
    }
}

impl ClientSocket for PageSocket {
    fn is_encrypted(&self) -> bool {
        true
    }

    fn is_reliable(&self) -> bool {
        false
    }

    fn addr(&self) -> std::io::Result<SocketAddr> {
        Err(ErrorKind::AddrNotAvailable.into())
    }

    fn is_closed(&mut self) -> bool {
        false
    }

    fn close(&mut self) {}

    fn preupdate(&mut self) {
        self.check_open();
    }

    fn try_recv(&mut self, buffer: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        let datagram = self
            .incoming
            .as_ref()
            .and_then(|incoming| incoming.lock().unwrap().try_recv().ok())
            .ok_or(ErrorKind::WouldBlock)?;
        buffer[..datagram.len()].copy_from_slice(&datagram);
        Ok((datagram.len(), self.server))
    }

    fn postupdate(&mut self) {}

    fn send(&mut self, _addr: SocketAddr, packet: &[u8]) -> Result<(), NetcodeTransportError> {
        if let Some(connection) = &self.connection {
            let _ = connection.send_datagram(packet);
            return Ok(());
        }
        // dropped while opening; the connection request opens it (renet2's `creq`)
        if self.opening.is_some() || packet[0] & 0x0f != 0 {
            return Ok(());
        }
        let encoded: String = packet.iter().map(|b| format!("%{b:02X}")).collect();
        let url = format!("{}?creq={encoded}", self.url);
        let config = wtransport::ClientConfig::builder()
            .with_bind_default()
            .with_server_certificate_hashes([wtransport::tls::Sha256Digest::new(self.hash)])
            .build();
        let (sender, receiver) = mpsc::channel();
        self.runtime.spawn(async move {
            let endpoint = wtransport::Endpoint::client(config).unwrap();
            match endpoint.connect(url).await {
                Ok(connection) => {
                    let _ = sender.send(connection);
                }
                Err(error) => eprintln!("WebTransport: {error}"),
            }
        });
        self.opening = Some(Mutex::new(receiver));
        Ok(())
    }
}

/// The certificate's hash the server tells over HTTP: `{"hash":"<hex>"}`.
fn fetch_hash(addr: SocketAddr) -> [u8; 32] {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("Access-Control-Allow-Origin: *"));
    let hex = response
        .split("\"hash\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}

#[test]
fn a_browser_joins_over_webtransport() {
    let Some(mut game) = TestMatch::new("ctf_Ash", &[]) else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    // a free port, for WebTransport (UDP) and the hash (TCP)
    let port = UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let socket = soldank_server::web::start(addr, 4, runtime.handle().clone()).unwrap();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let config = ServerSetupConfig {
        current_time: now,
        max_clients: 4,
        protocol_id: PROTOCOL_ID,
        socket_addresses: vec![vec![addr]],
        authentication: ServerAuthentication::Unsecure,
    };
    let mut transport =
        NetcodeServerTransport::new_with_sockets(config, vec![BoxedSocket::new(socket)]).unwrap();

    // the page's client
    let url = url::Url::parse(&format!("https://127.0.0.1:{port}/")).unwrap();
    let server = SocketAddr::from(WebServerDestination::Url(url.clone()));
    let page = PageSocket {
        url: url.to_string(),
        server,
        hash: fetch_hash(addr),
        runtime: runtime.handle().clone(),
        opening: None,
        connection: None,
        incoming: None,
    };
    let authentication = ClientAuthentication::Unsecure {
        protocol_id: PROTOCOL_ID,
        client_id: 7,
        socket_id: 0,
        server_addr: server,
        user_data: None,
    };
    let mut client_transport = NetcodeClientTransport::new(now, authentication, page).unwrap();
    let mut client = RenetClient::new(ConnectionConfig::test(), false);

    let dt = Duration::from_millis(16);
    let mut said_hello = false;
    let mut welcome = None;
    for _ in 0..600 {
        game.server.update(dt);
        let _ = transport.update(dt, &mut game.server);
        game.game.receive(&mut game.server, |_| None);
        game.game.tick(&mut game.server);
        transport.send_packets(&mut game.server);

        client.update(dt);
        let _ = client_transport.update(dt, &mut client);
        if client.is_connected() && !said_hello {
            client.send_message(channel::RELIABLE, encode(&hello("Browser")));
            said_hello = true;
        }
        while let Some(bytes) = client.receive_message(channel::RELIABLE) {
            if let Some(ServerMessage::Welcome { map, .. }) = decode(&bytes) {
                welcome = Some(map);
            }
        }
        let _ = client_transport.send_packets(&mut client);
        if welcome.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(welcome.as_deref(), Some("ctf_Ash"));
}
