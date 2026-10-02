pub use glam::{Vec2, Vec3, vec2, vec3};
pub use std::f32::consts::PI;

/// Angle in radians.
pub type Rad = f32;

pub fn rad(angle: f32) -> Rad {
    angle
}

/// Free Pascal evaluates arithmetic involving untyped real constants (`0.97`, `RUNSPEED / 6`)
/// and `Variant`s in extended precision, rounding only when the result is stored in a
/// `Single`. Doing the same in f64 reproduces opensoldat bit-for-bit; use `ext` to promote
/// and `fpc` to store.
pub fn ext(value: f32) -> f64 {
    f64::from(value)
}

/// Rounds an extended-precision Pascal expression into a `Single`. See [`ext`].
pub fn fpc(value: f64) -> f32 {
    value as f32
}
use std::ops::{Add, Mul, Sub};

pub fn distance(p1: Vec2, p2: Vec2) -> f32 {
    (p2 - p1).length()
}

pub fn vec2length(v: Vec2) -> f32 {
    v.length()
}

pub fn vec2normalize(v: Vec2) -> Vec2 {
    let magnitude = v.length();
    iif!(magnitude < 0.001, Vec2::ZERO, v / magnitude)
}

pub fn vec2angle(v: Vec2) -> Rad {
    f32::atan2(v.y, v.x)
}

pub fn point_line_distance(p1: Vec2, p2: Vec2, p3: Vec2) -> f32 {
    let u = ((p3.x - p1.x) * (p2.x - p1.x) + (p3.y - p1.y) * (p2.y - p1.y))
        / ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2));

    let x = p1.x + u * (p2.x - p1.x);
    let y = p1.y + u * (p2.y - p1.y);

    ((x - p3.x).powi(2) + (y - p3.y).powi(2)).sqrt()
}

pub fn lerp<T>(a: T, b: T, t: f32) -> T
where
    T: Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T> + Copy + Clone,
{
    a + (b - a) * t
}
