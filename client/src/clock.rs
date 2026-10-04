//! The game's time: fixed ticks (slower in bullet time), the frames drawn in between, and the
//! frame rate.

/// Seconds since the session started, the time not ticked away yet, and the frames drawn.
pub(crate) struct Clock {
    /// When the session started (`platform::now`).
    start: f64,
    /// The time at the last look, and the one before.
    pub(crate) cur: f64,
    prv: f64,
    /// Time not ticked away yet.
    pub(crate) acc: f64,
    /// Frames drawn this second, the last second's (`GetGameFps`), since when.
    frames: u32,
    pub(crate) fps: u32,
    fps_since: f64,
}

impl Clock {
    pub(crate) fn new() -> Clock {
        Clock {
            start: crate::platform::now(),
            cur: 0.0,
            prv: 0.0,
            acc: 0.0,
            frames: 0,
            fps: 0,
            fps_since: 0.0,
        }
    }

    /// Seconds since the session started.
    pub(crate) fn now(&self) -> f64 {
        crate::platform::now() - self.start
    }

    /// Time passed since the last look, `speed` times as fast (a demo's `demo_speed`).
    pub(crate) fn advance(&mut self, speed: f64) {
        self.cur = self.now();
        self.acc += (self.cur - self.prv) * speed;
        self.prv = self.cur;
    }

    /// A tick of `tick_time` seconds, if that much time is waiting.
    pub(crate) fn take_tick(&mut self, tick_time: f64) -> bool {
        if self.acc < tick_time {
            return false;
        }
        self.acc -= tick_time;
        true
    }

    /// A frame drawn: the frame rate, once a second.
    pub(crate) fn frame(&mut self) {
        self.frames += 1;
        if self.cur - self.fps_since >= 1.0 {
            self.fps = std::mem::take(&mut self.frames);
            self.fps_since = self.cur;
        }
    }
}
