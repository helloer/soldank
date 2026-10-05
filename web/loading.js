// The game's files come before the game, with the page showing how far along they are (#loading):
// the program, then the files it asks for first (soldat.smod and the interface font) are handed
// to it through miniquad's file loading, in place of fetching them again. The game says when it
// has started (soldank_started), and the page goes.

// fetched files the game hasn't asked for yet, by name
var soldank_files = {};

miniquad_add_plugin({
    name: "soldank_files",
    version: 1,
    register_plugin: function (importObject) {
        var fetch_file = importObject.env.fs_load_file;
        importObject.env.fs_load_file = function (ptr, len) {
            var name = UTF8ToString(ptr, len);
            var bytes = soldank_files[name];
            if (bytes === undefined) {
                return fetch_file(ptr, len);
            }
            delete soldank_files[name];
            var file_id = FS.unique_id;
            FS.unique_id += 1;
            // later, like an answer from the network: the game notes the file's number first
            setTimeout(function () {
                FS.loaded_files[file_id] = bytes;
                wasm_exports.file_loaded(file_id);
            }, 0);
            return file_id;
        };

        // the game is up (or couldn't start: the page says so)
        importObject.env.soldank_started = function (ok) {
            if (ok) {
                document.getElementById("loading").remove();
            } else {
                soldank_loading_failed("The game couldn't start (the browser's console says why).");
            }
        };
    },
});

function soldank_loading_failed(message) {
    var loading = document.getElementById("loading");
    loading.classList.add("failed");
    document.getElementById("loading-text").textContent = message;
}

// Fetches `url`, telling `progress` of each piece: its bytes.
function soldank_fetch(url, progress) {
    return fetch(url).then(function (response) {
        if (!response.ok) {
            throw new Error(response.status + " " + response.statusText);
        }
        // with a compressed answer the length is the compressed one: the bytes may go past it
        var length = Number(response.headers.get("Content-Length")) || 0;
        progress.length(length);
        var reader = response.body.getReader();
        var bytes = new Uint8Array(length);
        var received = 0;
        var pieces = null;
        var read = function () {
            return reader.read().then(function (result) {
                if (result.done) {
                    if (pieces === null) {
                        return bytes.subarray(0, received);
                    }
                    var all = new Uint8Array(received);
                    var at = 0;
                    pieces.forEach(function (piece) {
                        all.set(piece, at);
                        at += piece.length;
                    });
                    return all;
                }
                var piece = result.value;
                if (pieces === null && received + piece.length <= bytes.length) {
                    bytes.set(piece, received);
                } else {
                    pieces = pieces || [bytes.subarray(0, received)];
                    pieces.push(piece);
                }
                received += piece.length;
                progress.received(received);
                return read();
            });
        };
        return read();
    }).catch(function (error) {
        throw new Error(url + " (" + error.message + ")");
    });
}

// Fetches the program (`wasm`), the game's `files` and the `optional` ones (the game goes on
// without them), then starts the game.
function soldank_load(wasm, files, optional) {
    var bar = document.getElementById("loading-done");
    var text = document.getElementById("loading-text");
    var all = [wasm].concat(files, optional);
    var lengths = all.map(function () { return 0; });
    var received = all.map(function () { return 0; });
    var megabytes = function (bytes) {
        return (bytes / 1e6).toFixed(1);
    };
    var show = function () {
        // the other files keep coming after one failed: the page keeps saying which
        if (document.getElementById("loading").classList.contains("failed")) {
            return;
        }
        var total = 0;
        var done = 0;
        for (var i = 0; i < all.length; i++) {
            total += lengths[i];
            done += lengths[i] ? Math.min(received[i], lengths[i]) : received[i];
        }
        text.textContent = total
            ? "Loading " + megabytes(done) + " of " + megabytes(total) + " MB"
            : "Loading " + megabytes(done) + " MB";
        bar.style.width = total ? (100 * done / total).toFixed(1) + "%" : "0";
    };
    var fetches = all.map(function (url, i) {
        var fetched = soldank_fetch(url, {
            length: function (length) {
                lengths[i] = length;
                show();
            },
            received: function (bytes) {
                received[i] = bytes;
                show();
            },
        });
        if (optional.indexOf(url) < 0) {
            return fetched;
        }
        return fetched.catch(function (error) {
            console.warn("cannot fetch " + error.message);
            return null;
        });
    });
    Promise.all(fetches).then(
        function (fetched) {
            all.forEach(function (url, i) {
                if (i > 0 && fetched[i] !== null) {
                    soldank_files[url] = fetched[i];
                }
            });
            text.textContent = "Starting\u2026";
            bar.style.width = "100%";
            load(URL.createObjectURL(new Blob([fetched[0]], { type: "application/wasm" })));
        },
        function (error) {
            soldank_loading_failed("Cannot fetch " + error.message);
        }
    );
}
