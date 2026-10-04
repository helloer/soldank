//! Remote admin over a real TCP socket on localhost.

use soldank_server::rcon::{Rcon, Request};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn read_line(reader: &mut impl BufRead) -> String {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line
}

/// The requests that come within a second.
fn wait_for(rcon: &Rcon, count: usize) -> Vec<Request> {
    let start = Instant::now();
    let mut got = Vec::new();
    while got.len() < count && start.elapsed() < Duration::from_secs(1) {
        got.extend(rcon.poll().into_iter().map(|(_, _, request)| request));
        std::thread::sleep(Duration::from_millis(5));
    }
    got
}

#[test]
fn admins_log_in_and_talk_to_the_server() {
    let rcon = Rcon::start("127.0.0.1:0".parse().unwrap(), "secret".into()).unwrap();

    // a wrong password: told, and let go
    let mut stranger = TcpStream::connect(rcon.addr).unwrap();
    let mut reader = BufReader::new(stranger.try_clone().unwrap());
    assert_eq!(
        read_line(&mut reader),
        "OpenSoldat Admin Connection Established.\r\n"
    );
    stranger.write_all(b"guess\r\n").unwrap();
    assert_eq!(read_line(&mut reader), "Invalid password.\r\n");
    assert_eq!(read_line(&mut reader), "", "closed");

    let mut admin = TcpStream::connect(rcon.addr).unwrap();
    let mut reader = BufReader::new(admin.try_clone().unwrap());
    read_line(&mut reader);
    admin.write_all(b"secret\n").unwrap();
    assert_eq!(
        read_line(&mut reader),
        "Welcome, you are in command of the server now.\r\n"
    );
    read_line(&mut reader);
    assert!(read_line(&mut reader).starts_with("Server Version: "));

    admin
        .write_all(b"/kick Major\r\nhello admins\r\nREFRESHX\r\nSHUTDOWN\r\n")
        .unwrap();
    assert_eq!(
        wait_for(&rcon, 4),
        [
            Request::Line("/kick Major".into()),
            Request::Line("hello admins".into()),
            Request::Refresh,
            Request::Shutdown,
        ]
    );

    // the console reaches the admins
    rcon.broadcast("Major has been kicked.");
    assert_eq!(read_line(&mut reader), "Major has been kicked.\r\n");
    assert_eq!(
        rcon.admin_ips(),
        ["127.0.0.1".parse::<std::net::IpAddr>().unwrap()]
    );
}
