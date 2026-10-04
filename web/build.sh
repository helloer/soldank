#!/usr/bin/env bash
# Builds the browser version into target/web: the page, miniquad's loader (js/gl.js of the
# miniquad crate in use) and its plugins, the game, and links to the game's files (soldat.smod and the
# interface font play-regular.ttf, in the repository or in assets/). Serve the directory over HTTP, e.g.
#   python3 -m http.server -d target/web
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown -p soldank "$@"
out=target/web
mkdir -p "$out"
cp web/index.html web/sound.js web/webtransport.js "$out/"
cp target/wasm32-unknown-unknown/release/soldank.wasm "$out/"
# miniquad's loader, and the plugins egui-miniquad's crates need (their js/ of the version in use)
metadata=$(cargo metadata --format-version 1)
for crate in miniquad:gl.js sapp-jsutils:sapp_jsutils.js quad-url:quad-url.js; do
    dir=$(echo "$metadata" | python3 -c '
import json, sys, os
name = sys.argv[1]
print(os.path.dirname(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"] == name)))' "${crate%%:*}")
    cp "$dir/js/${crate#*:}" "$out/"
done
# the font next to soldat.smod, like the desktop game takes it, or in assets/
for file in soldat.smod play-regular.ttf; do
    found=""
    for place in "$file" "assets/$file"; do
        if [ -e "$place" ]; then
            found=$place
            break
        fi
    done
    if [ -n "$found" ]; then
        ln -sf "$(realpath "$found")" "$out/$file"
    else
        echo "missing $file: the page needs it next to it" >&2
    fi
done
echo "built $out ($(du -h "$out/soldank.wasm" | cut -f1) of wasm)"
