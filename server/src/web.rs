//! Browsers join over WebTransport (renet2's), on the port after the game's. Its certificate is
//! either a real one for the server's domain name (files, e.g. Let's Encrypt's), which browsers
//! check as usual, or a self-signed one made at each start: browsers take that for two weeks by
//! its hash, which the page asks for first, over plain HTTP on the same port number (TCP). Pages
//! served over HTTPS can't ask over plain HTTP but on the page's own computer (localhost).

use anyhow::Context;
use renet2_netcode::{ServerCertHash, WebTransportServer, WebTransportServerConfig};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A real certificate's files (PEM): the certificate (or its chain, like Let's Encrypt's
/// `fullchain.pem`) and its private key (`privkey.pem`).
#[derive(Debug, Clone)]
pub struct CertificateFiles {
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// A WebTransport socket for at most `max_clients` browsers at `addr`, with the certificate of
/// `files`, or a self-signed one and the HTTP answer with its hash beside it.
pub fn start(
    addr: SocketAddr,
    max_clients: usize,
    files: Option<&CertificateFiles>,
    runtime: tokio::runtime::Handle,
) -> anyhow::Result<WebTransportServer> {
    let Some(files) = files else {
        let (config, hash) = WebTransportServerConfig::new_selfsigned(addr, max_clients)?;
        let socket = WebTransportServer::new(config, runtime)?;
        serve_cert_hash(addr, &hash)?;
        return Ok(socket);
    };
    // no hash told: browsers given one take only a certificate of two weeks at most, matching it
    let config = WebTransportServerConfig {
        cert: read_certificate(&files.cert)?,
        key: PrivateKeyDer::from_pem_file(&files.key)
            .with_context(|| format!("no private key in {}", files.key.display()))?,
        listen: addr,
        max_clients,
    };
    WebTransportServer::new(config, runtime)
}

/// The first certificate of a PEM file: the server's own, of a chain (renet2 sends no more).
fn read_certificate(path: &Path) -> anyhow::Result<CertificateDer<'static>> {
    let cannot = || format!("cannot read a certificate from {}", path.display());
    CertificateDer::pem_file_iter(path)
        .with_context(cannot)?
        .next()
        .with_context(cannot)?
        .with_context(cannot)
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
