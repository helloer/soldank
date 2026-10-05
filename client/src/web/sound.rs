//! Sound in the browser: kira, with a backend of its own. The page's `sound.js` (a miniquad
//! plugin) plays it through Web Audio; once a frame, after the game's update, kira's renderer
//! mixes as much as the page wants ([`mix`]), so a sound starts in the frame that asked for it.
//! (Mixed when Web Audio asks instead, every sound asked for since its last buffer would start
//! at once, at the buffer's start.) (kira's cpal backend needs wasm-bindgen, which miniquad's
//! loader doesn't host.)

use kira::backend::{Backend, Renderer};
use std::cell::RefCell;

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// Starts the page's audio (`sound.js`): its sample rate, 0 without Web Audio.
    fn soldank_audio_start() -> u32;
    /// How many frames the page wants mixed now.
    fn soldank_audio_wanted() -> u32;
    /// Plays `frames` mixed frames (interleaved stereo at `ptr`) after the last ones.
    fn soldank_audio_queue(ptr: *const f32, frames: u32);
}

thread_local! {
    static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
    /// The last frames mixed, interleaved stereo.
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

/// Mixes what the page wants of the sound now: once a frame, after the game's update.
pub fn mix() {
    // SAFETY: a call for a number
    let frames = unsafe { soldank_audio_wanted() };
    if frames == 0 {
        return;
    }
    RENDERER.with(|renderer| {
        let mut renderer = renderer.borrow_mut();
        let Some(renderer) = renderer.as_mut() else {
            return;
        };
        BUFFER.with(|buffer| {
            let mut buffer = buffer.borrow_mut();
            buffer.clear();
            buffer.resize(frames as usize * 2, 0.0);
            renderer.on_start_processing();
            renderer.process(&mut buffer, 2);
            // SAFETY: the page copies the frames during the call
            unsafe { soldank_audio_queue(buffer.as_ptr(), frames) };
        });
    });
}

/// The version of `sound.js` this game goes with (miniquad's loader compares them).
#[unsafe(no_mangle)]
pub extern "C" fn soldank_audio_crate_version() -> u32 {
    2
}
