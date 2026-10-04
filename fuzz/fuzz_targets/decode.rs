//! Whatever bytes a client or a server sends: a message or `None`, never a crash. A decoded
//! message encodes again.

#![no_main]

use libfuzzer_sys::fuzz_target;
use soldank_core::net::{ClientMessage, ServerMessage, decode, encode};

fuzz_target!(|data: &[u8]| {
    if let Some(message) = decode::<ClientMessage>(data) {
        let _ = encode(&message);
    }
    if let Some(message) = decode::<ServerMessage>(data) {
        let _ = encode(&message);
    }
});
