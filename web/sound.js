// The game's sound in the browser. The game mixes kira's sound once a frame, after its update,
// as much as the time since the last frame took (soldank_audio_wanted, then soldank_audio_queue),
// so a sound starts in the frame that asked for it; the page plays each piece at its own time, a
// little later (latency), through an audio worklet, or a script processor where there's none
// (pages not served securely). Browsers start sound once the page is used: the first click or key.

// Writes the frames [start, start + left.length) of the queued pieces ({start, samples}:
// interleaved stereo, in order) to left and right, silence where there are none; pieces played
// or come too late go. (Also the worklet's, so it stays on its own.)
function soldank_audio_fill(queue, start, left, right) {
    var frames = left.length;
    left.fill(0);
    right.fill(0);
    while (queue.length > 0) {
        var piece = queue[0];
        var end = piece.start + piece.samples.length / 2;
        if (piece.start >= start + frames) {
            return;
        }
        for (var frame = Math.max(start, piece.start); frame < Math.min(end, start + frames); frame++) {
            var at = 2 * (frame - piece.start);
            left[frame - start] = piece.samples[at];
            right[frame - start] = piece.samples[at + 1];
        }
        if (end > start + frames) {
            return;
        }
        queue.shift();
    }
}

miniquad_add_plugin({
    name: "soldank_audio",
    version: 2,
    register_plugin: function (importObject) {
        // the context, how the pieces go to what plays them, how far ahead they're mixed, where
        // the last one ends (frames of the context's time)
        var audio = null;

        importObject.env.soldank_audio_start = function () {
            var AudioContext = window.AudioContext || window.webkitAudioContext;
            if (!AudioContext) {
                return 0;
            }
            var context = new AudioContext();
            var rate = context.sampleRate;
            audio = { context: context, send: null, latency: 0, end: 0 };
            if (context.audioWorklet) {
                var code = soldank_audio_fill.toString() + `
                    class SoldankOutput extends AudioWorkletProcessor {
                        constructor() {
                            super();
                            this.queue = [];
                            this.port.onmessage = (event) => this.queue.push(event.data);
                        }
                        process(inputs, outputs) {
                            soldank_audio_fill(this.queue, currentFrame, outputs[0][0], outputs[0][1]);
                            return true;
                        }
                    }
                    registerProcessor("soldank-output", SoldankOutput);`;
                var url = URL.createObjectURL(new Blob([code], { type: "text/javascript" }));
                context.audioWorklet.addModule(url).then(
                    function () {
                        var node = new AudioWorkletNode(context, "soldank-output", {
                            numberOfInputs: 0,
                            outputChannelCount: [2],
                        });
                        node.connect(context.destination);
                        audio.node = node;
                        audio.latency = Math.round(0.05 * rate);
                        audio.send = function (piece) {
                            node.port.postMessage(piece, [piece.samples.buffer]);
                        };
                    },
                    function (error) {
                        console.warn("no audio worklet:", error);
                    }
                );
            } else {
                // on the page's thread, which the frames hold up: more ahead
                var queue = [];
                var node = context.createScriptProcessor(1024, 0, 2);
                node.onaudioprocess = function (event) {
                    var start = Math.round(event.playbackTime * rate);
                    var output = event.outputBuffer;
                    soldank_audio_fill(queue, start, output.getChannelData(0), output.getChannelData(1));
                };
                node.connect(context.destination);
                audio.node = node;
                audio.latency = Math.round(0.1 * rate);
                audio.send = function (piece) {
                    queue.push(piece);
                };
            }
            var resume = function () {
                if (context.state !== "running") {
                    context.resume();
                }
            };
            ["pointerdown", "keydown", "touchstart"].forEach(function (name) {
                document.addEventListener(name, resume);
            });
            // (kept, or the node may be collected)
            window.soldank_audio = audio;
            return rate;
        };

        // the frames to mix now: up to the latency ahead of what plays, after the last piece (or
        // from just ahead of now, after none came for a while: the tab hidden, a long frame)
        importObject.env.soldank_audio_wanted = function () {
            if (audio === null || audio.send === null || audio.context.state !== "running") {
                return 0;
            }
            var now = Math.round(audio.context.currentTime * audio.context.sampleRate);
            audio.end = Math.max(audio.end, now + 256);
            return Math.max(0, now + audio.latency - audio.end);
        };

        // the frames mixed (interleaved stereo in the wasm memory), to play after the last
        importObject.env.soldank_audio_queue = function (ptr, frames) {
            var samples = new Float32Array(wasm_memory.buffer, ptr, frames * 2).slice();
            audio.send({ start: audio.end, samples: samples });
            audio.end += frames;
        };
    },
});
