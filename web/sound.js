// The game's sound in the browser: a Web Audio script processor asks the game for each buffer
// of kira's mix (soldank_audio_render, interleaved stereo in the wasm memory). Browsers start
// sound once the page is used: the first click or key.
miniquad_add_plugin({
    name: "soldank_audio",
    version: 1,
    register_plugin: function (importObject) {
        importObject.env.soldank_audio_start = function () {
            var AudioContext = window.AudioContext || window.webkitAudioContext;
            if (!AudioContext) {
                return 0;
            }
            var context = new AudioContext();
            var node = context.createScriptProcessor(2048, 0, 2);
            node.onaudioprocess = function (event) {
                var left = event.outputBuffer.getChannelData(0);
                var right = event.outputBuffer.getChannelData(1);
                var frames = left.length;
                var ptr = wasm_exports.soldank_audio_render(frames);
                var mix = new Float32Array(wasm_memory.buffer, ptr, frames * 2);
                for (var i = 0; i < frames; i++) {
                    left[i] = mix[2 * i];
                    right[i] = mix[2 * i + 1];
                }
            };
            node.connect(context.destination);
            var resume = function () {
                if (context.state !== "running") {
                    context.resume();
                }
            };
            ["pointerdown", "keydown", "touchstart"].forEach(function (name) {
                document.addEventListener(name, resume);
            });
            // (kept, or the processor may be collected)
            window.soldank_audio = { context: context, node: node };
            return context.sampleRate;
        };
    },
});
