//! Browsers join over WebTransport (renet2's), on the port after the game's. Its certificate is a
//! self-signed one made at each start (browsers take those for two weeks, by their hash), so the
//! page asks for the hash first, over plain HTTP on the same port number (TCP).

use renet2_netcode::{ServerCertHash, WebTransportServer, WebTransportServerConfig};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

/// A WebTransport socket for at most `max_clients` browsers at `addr`, and the HTTP answer with
/// its certificate's hash beside it.
pub fn start(
    addr: SocketAddr,
    max_clients: usize,
    runtime: tokio::runtime::Handle,
) -> anyhow::Result<WebTransportServer> {
    let (config, hash) = WebTransportServerConfig::new_selfsigned(addr, max_clients)?;
    let socket = WebTransportServer::new(config, runtime)?;
    serve_cert_hash(addr, &hash)?;
    Ok(socket)
}

/// The certificate's hash as the page reads it: `{"hash":"<64 hex digits>"}`.
fn hash_json(hash: &ServerCertHash) -> String {
    let hex: String = hash.hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("{{\"hash\":\"{hex}\"}}")
}

/// Answers every HTTP request on `addr` (TCP) with the hash, for any page (CORS).
fn serve_cert_hash(addr: SocketAddr, hash: &ServerCertHash) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let body = hash_json(hash);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            answer(stream, &body);
        }
    });
    Ok(())
}

fn answer(mut stream: TcpStream, body: &str) {
    // the request itself doesn't matter: its start, and the answer
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut request = [0u8; 1024];
    let _ = stream.read(&mut request);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\
         Cache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_hex_json() {
        let hash = ServerCertHash {
            hash: std::array::from_fn(|i| i as u8),
        };
        let json = hash_json(&hash);
        assert_eq!(json.len(), "{\"hash\":\"\"}".len() + 64);
        assert!(json.starts_with("{\"hash\":\"000102"));
    }
}
