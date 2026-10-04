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
        // the cursor starts in the middle of the screen
        let middle = vec2(game_width, game_height) / 2.0;
        Camera {
            pos,
            pos_prev: pos,
            mouse: middle,
            mouse_prev: middle,
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

    /// `CalculateRecoil`: turns the cursor around the soldier at `player` (world
    /// position) by `angle` radians, half way, keeping it on the same side.
    pub fn apply_recoil(&mut self, player: Vec2, angle: f32) {
        let p = player - self.pos + vec2(self.game_width / 2.0, self.game_height / 2.0);
        let d = self.mouse - p;
        let radius = d.length();
        let alpha = if d.x > 0.0 {
            (f32::atan(d.y / d.x) + angle).min(PI / 2.0)
        } else {
            (f32::atan(d.y / d.x) + PI - angle).max(PI / 2.0)
        };

        let mut cursor = p + (d + vec2(alpha.cos(), alpha.sin()) * radius) / 2.0;
        if d.x > 0.0 && cursor.x <= p.x {
            cursor.x = p.x + 0.0001;
        } else if d.x <= 0.0 && cursor.x >= p.x {
            cursor.x = p.x - 0.0001;
        }
        self.mouse = cursor;
    }

    /// One fixed tick: zoom towards `zoom_goal` (`EaseZoom`), then follow a soldier (its
    /// position and `AimDistCoef`), reaching out towards the cursor, or without one move
    /// with the cursor (the free camera).
    pub fn update(&mut self, follow: Option<(Vec2, f32)>, zoom_goal: f32) {
        self.pos_prev = self.pos;
        self.mouse_prev = self.mouse;
        self.zoom = ease_zoom(self.zoom, zoom_goal);

        let z = f32::exp(self.zoom);
        let (w, h) = (self.game_width, self.game_height);
        let center = vec2(w / 2.0, h / 2.0);

        match follow {
            Some((target, coef)) => {
                let m = vec2(
                    z * (self.mouse.x - center.x) / coef
                        * ((2.0 * 640.0 / w - 1.0)
                            + (w - 640.0) / w * (DEFAULT_AIM_DIST - coef) / 6.8),
                    z * (self.mouse.y - center.y) / coef,
                );
                self.pos += (target - self.pos) * CAMSPEED;
                self.pos += m;
            }
            None => {
                // the free camera rests while the cursor is in the middle
                let ratio = w / 640.0;
                let middle = self.mouse.x > 310.0 * ratio
                    && self.mouse.x < 330.0 * ratio
                    && self.mouse.y > 230.0
                    && self.mouse.y < 250.0;
                if !middle {
                    self.pos += (self.mouse - center) * z / SPECTATOR_AIM_DIST;
                }
            }
        }
    }

    /// Puts the cursor in the middle of the screen.
    pub fn center_mouse(&mut self) {
        self.mouse = vec2(self.game_width / 2.0, self.game_height / 2.0);
        self.mouse_prev = self.mouse;
    }
}

/// `CAMSPEED`, `SPECTATORAIMDIST`
const CAMSPEED: f32 = 0.14;
const SPECTATOR_AIM_DIST: f32 = 30.0;

/// `EaseZoom`: a quarter of the way each tick, snapping when close.
fn ease_zoom(current: f32, goal: f32) -> f32 {
    let zoom = current + (goal - current) / 4.0;
    if (goal - zoom).abs() < 0.01 {
        goal
    } else {
        zoom
    }
}
