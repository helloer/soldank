# Soldank

[![CI](https://github.com/helloer/soldank/actions/workflows/ci.yml/badge.svg)](https://github.com/helloer/soldank/actions/workflows/ci.yml)

Open source clone of Soldat engine written in rust

## Play

Download a [release](https://github.com/helloer/soldank/releases), unpack it and run `soldank`.
Run `soldank-server` to host a game (UDP port 23073, settings in `configs/server.cfg`).
Browsers join it on the next port, 23074 (UDP and TCP).

## Build

1. Install [Rust](https://rustup.rs) 1.88 or newer.
2. Download `soldat.smod` and `play-regular.ttf` from the
   [opensoldat/base v0.4 release](https://github.com/opensoldat/base/releases/tag/v0.4) into the
   repository's root.
3. Run the game or the server:

   ```
   cargo run --release
   cargo run --release -p soldank-server
   ```

For the browser version:

```
rustup target add wasm32-unknown-unknown
web/build.sh
python3 -m http.server -d target/web
```

Then open http://localhost:8000/.
