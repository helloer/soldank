use super::*;

/// The player's view: follows the soldier, offset towards the mouse cursor.
pub struct Camera {
    pub pos: Vec2,
    pub pos_prev: Vec2,
    /// Cursor position in game-screen coordinates (`game_width` x `game_height`).
    pub mouse: Vec2,
    pub mouse_prev: Vec2,
    /// Log2-ish zoom level, `exp(zoom)` scales the view.
    pub zoom: f32,
    pub game_width: f32,
    pub game_height: f32,
}

impl Camera {
    pub fn new(pos: Vec2, game_width: f32, game_height: f32) -> Camera {
        Camera {
            pos,
            pos_prev: pos,
            mouse: Vec2::ZERO,
            mouse_prev: Vec2::ZERO,
            zoom: 0.0,
            game_width,
            game_height,
        }
    }

    /// World point under the cursor, used as the soldier's aim.
    pub fn aim(&self) -> Vec2 {
        vec2(
            self.mouse.x - self.game_width / 2.0 + self.pos.x,
            self.mouse.y - self.game_height / 2.0 + self.pos.y,
        )
    }

    /// One fixed tick: zoom by `zoom_dir` (-1, 0 or 1) and ease towards `target`.
    pub fn update(&mut self, target: Vec2, zoom_dir: f32, dt: f32) {
        self.pos_prev = self.pos;
        self.mouse_prev = self.mouse;
        self.zoom += zoom_dir * dt;

        let z = f32::exp(self.zoom);
        let mut m = Vec2::ZERO;

        m.x = z * (self.mouse.x - self.game_width / 2.0) / 7.0
            * ((2.0 * 640.0 / self.game_width - 1.0)
                + (self.game_width - 640.0) / self.game_width * 0.0 / 6.8);
        m.y = z * (self.mouse.y - self.game_height / 2.0) / 7.0;

        let norm = target - self.pos;
        self.pos += norm * 0.14;
        self.pos += m;
    }
}
