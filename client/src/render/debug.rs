//! Debug drawing over the world: collision polygons, waypoints, spawn points, colliders and
//! skeletons. Switched on from the dev overlay (`--features dev`, F12).

use super::*;

/// What to draw.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct DebugDraw {
    pub polygons: bool,
    pub waypoints: bool,
    pub spawns: bool,
    pub colliders: bool,
    pub skeletons: bool,
}

impl DebugDraw {
    pub fn any(&self) -> bool {
        self.polygons || self.waypoints || self.spawns || self.colliders || self.skeletons
    }
}

/// A segment `width` world units thick.
fn line(batch: &mut DrawBatch, a: Vec2, b: Vec2, width: f32, color: Color) {
    let d = b - a;
    let len = d.length();
    if len <= 0.0 {
        return;
    }
    let n = vec2(-d.y, d.x) / len * (width / 2.0);
    batch.add_quad(
        None,
        &[
            vertex(a + n, Vec2::ZERO, color),
            vertex(b + n, Vec2::ZERO, color),
            vertex(b - n, Vec2::ZERO, color),
            vertex(a - n, Vec2::ZERO, color),
        ],
    );
}

/// A square `size` wide around `p`.
fn dot(batch: &mut DrawBatch, p: Vec2, size: f32, color: Color) {
    let h = size / 2.0;
    batch.add_quad(
        None,
        &[
            vertex(p + vec2(-h, -h), Vec2::ZERO, color),
            vertex(p + vec2(h, -h), Vec2::ZERO, color),
            vertex(p + vec2(h, h), Vec2::ZERO, color),
            vertex(p + vec2(-h, h), Vec2::ZERO, color),
        ],
    );
}

/// The polygon colour by type: walls white, background green, special ones yellow, harmful
/// ones red.
fn poly_color(polytype: PolyType) -> Color {
    use PolyType::*;
    match polytype {
        Normal => rgba(255, 255, 255, 140),
        Background | BackgroundTransition => rgba(80, 220, 80, 90),
        Deadly | BloodyDeadly | Hurts | Lava | Explosive | HurtsFlaggers => rgba(255, 60, 60, 160),
        _ => rgba(255, 220, 60, 140),
    }
}

/// Draws what `draw` asks for, `px` world units per screen pixel.
pub fn render_debug(
    batch: &mut DrawBatch,
    world: &World,
    draw: DebugDraw,
    px: f32,
    frame_percent: f32,
) {
    let map = &world.map;
    if draw.polygons {
        for poly in &map.polygons {
            let color = poly_color(poly.polytype);
            let v = poly.vertices.map(|v| vec2(v.x, v.y));
            for i in 0..3 {
                line(batch, v[i], v[(i + 1) % 3], px, color);
            }
        }
    }
    if draw.colliders {
        for c in map.colliders.iter().filter(|c| c.active) {
            let center = vec2(c.x, c.y);
            let at = |i: i32| {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                center + vec2(a.cos(), a.sin()) * c.radius
            };
            for i in 0..24 {
                line(batch, at(i), at(i + 1), px, rgba(80, 160, 255, 200));
            }
        }
    }
    if draw.waypoints {
        let at = |w: &Waypoint| vec2(w.x as f32, w.y as f32);
        for w in map.waypoints.iter().filter(|w| w.active) {
            let n = w.connections_num.clamp(0, w.connections.len() as i32) as usize;
            for &c in &w.connections[..n] {
                if let Some(to) = c.checked_sub(1).and_then(|i| map.waypoints.get(i as usize)) {
                    line(batch, at(w), at(to), px, rgba(255, 140, 0, 120));
                }
            }
        }
        for w in map.waypoints.iter().filter(|w| w.active) {
            let color = if w.jets {
                rgba(80, 200, 255, 230)
            } else {
                rgba(255, 140, 0, 230)
            };
            dot(batch, at(w), 4.0 * px, color);
        }
    }
    if draw.spawns {
        for s in map.spawnpoints.iter().filter(|s| s.active) {
            let color = match s.team {
                1 | 5 => rgb(255, 40, 40),
                2 | 6 => rgb(60, 80, 255),
                3 => rgb(255, 255, 40),
                4 => rgb(40, 255, 40),
                _ => rgb(255, 255, 255),
            };
            dot(batch, vec2(s.x as f32, s.y as f32), 6.0 * px, color);
        }
    }
    if draw.skeletons {
        for soldier in world.soldiers.values().filter(|s| !s.is_spectator()) {
            render_skeleton(soldier, batch, px, frame_percent);
        }
    }
}
