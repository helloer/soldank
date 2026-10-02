# Soldank

[WIP] open source clone of Soldat engine written in rust

# WIP Screenshot (click to play video)

[![WIP screenshot](https://www.dropbox.com/s/7kijx1lv2dle6km/soldank.png?raw=1)](https://www.dropbox.com/s/56xba14jicat59l/soldank.mkv?dl=0)
# Goals:

* Fully authentic look and feel
* ~~bugs~~ feature-complete port of soldat

# How to build:
1. Install Rust (stable, 1.85+) - https://rustup.rs
2. Copy ```anims objects maps textures scenery-gfx gostek-gfx objects-gfx sparks-gfx weapons-gfx interface-gfx mod.ini``` from the `shared` directory of [opensoldat/base](https://github.com/opensoldat/base) (or from a Soldat install) to `soldank/assets`
3. ```cargo run --release``` to run the game (```-- --map <name>``` to pick a map, default `ctf_Ash`)

# ROADMAP:

* ~~Refactor rendering code and add support for sceneries and gostek rendering~~
* ~~Implement proper game loop~~

