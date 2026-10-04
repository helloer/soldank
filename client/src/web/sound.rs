//! Sound in the browser: kira, with a backend of its own. The page's `sound.js` (a miniquad
//! plugin) makes a Web Audio context whose script processor asks the game for each buffer
//! (`soldank_audio_render`); kira's renderer mixes it. (kira's cpal backend needs
//! wasm-bindgen, which miniquad's loader doesn't host.)

use kira::backend::{Backend, Renderer};
use std::cell::RefCell;

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// Starts the page's audio (`sound.js`): its sample rate, 0 without Web Audio.
    fn soldank_audio_start() -> u32;
}

thread_local! {
    static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
    /// The last buffer, interleaved stereo.
    static BUFFER: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
}

pub struct WebBackend;

impl Backend for WebBackend {
    type Settings = ();
    type Error = String;

    fn setup(_: (), _internal_buffer_size: usize) -> Result<(WebBackend, u32), String> {
        match unsafe { soldank_audio_start() } {
            0 => Err("no Web Audio in this browser".to_string()),
            sample_rate => Ok((WebBackend, sample_rate)),
        }
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), String> {
        RENDERER.with(|r| *r.borrow_mut() = Some(renderer));
        Ok(())
    }
}

/// The page's audio wants `frames` more (stereo): mixed into a buffer, whose address goes back.
#[unsafe(no_mangle)]
pub extern "C" fn soldank_audio_render(frames: u32) -> *const f32 {
    BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        buffer.clear();
        buffer.resize(frames as usize * 2, 0.0);
        RENDERER.with(|renderer| {
            if let Some(renderer) = renderer.borrow_mut().as_mut() {
                renderer.on_start_processing();
                renderer.process(&mut buffer, 2);
            }
        });
        buffer.as_ptr()
    })
}

/// The version of `sound.js` this game goes with (miniquad's loader compares them).
#[unsafe(no_mangle)]
pub extern "C" fn soldank_audio_crate_version() -> u32 {
    1
}
