use glam::{DMat2, DVec2, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handle {
    #[default]
    AutoClamped,
    Auto,
    Vector,
    Aligned,
    Free,
}

impl Handle {
    pub const ALL: [Handle; 5] = [Handle::AutoClamped, Handle::Auto, Handle::Vector, Handle::Aligned, Handle::Free];

    pub fn name(self) -> &'static str {
        match self {
            Handle::AutoClamped => "Auto Clamped",
            Handle::Auto => "Automatic",
            Handle::Vector => "Vector",
            Handle::Aligned => "Aligned",
            Handle::Free => "Free",
        }
    }

    pub fn computed(self) -> bool {
        matches!(self, Handle::AutoClamped | Handle::Auto | Handle::Vector)
    }

    pub fn after_drag(self) -> Handle {
        match self {
            Handle::AutoClamped | Handle::Auto | Handle::Aligned => Handle::Aligned,
            Handle::Vector | Handle::Free => Handle::Free,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub handle: Handle,
    pub left: Vec2,
    pub right: Vec2,
}

const MIN_REACH: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Point {
    pub fn drag_handle(&mut self, side: Side, offset: Vec2) {
        self.handle = self.handle.after_drag();
        let (moved, other, sign) = match side {
            Side::Left => (&mut self.left, &mut self.right, -1.0),
            Side::Right => (&mut self.right, &mut self.left, 1.0),
        };
        *moved = Vec2::new((offset.x * sign).max(MIN_REACH) * sign, offset.y);
        if self.handle == Handle::Aligned {
            *other = -moved.normalize_or_zero() * other.length();
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Extend {
    #[default]
    Constant,
    Linear,
    Cyclic,
}

impl Extend {
    pub const ALL: [Extend; 3] = [Extend::Constant, Extend::Linear, Extend::Cyclic];

    pub fn name(self) -> &'static str {
        match self {
            Extend::Constant => "Constant",
            Extend::Linear => "Linear",
            Extend::Cyclic => "Make Cyclic",
        }
    }
}

const PADDING: usize = 8;

pub fn recalc_extended(points: &mut [Point], extend: Extend) {
    let n = points.len();
    if extend == Extend::Constant || n < 2 {
        return recalc(points);
    }
    let (before, after) = match extend {
        Extend::Cyclic => cyclic_padding(points),
        _ => linear_padding(points),
    };
    let start = before.len();
    let mut all = [before, points.to_vec(), after].concat();
    recalc(&mut all);
    points.copy_from_slice(&all[start..start + n]);
}

fn linear_padding(points: &[Point]) -> (Vec<Point>, Vec<Point>) {
    let n = points.len();
    let tail = |end: Point, neighbor: Point, step: usize| Point {
        x: end.x + (end.x - neighbor.x) * step as f32,
        y: end.y + (end.y - neighbor.y) * step as f32,
        handle: Handle::Auto,
        left: Vec2::ZERO,
        right: Vec2::ZERO,
    };
    let before = (1..=PADDING).rev().map(|step| tail(points[0], points[1], step)).collect();
    let after = (1..=PADDING).map(|step| tail(points[n - 1], points[n - 2], step)).collect();
    (before, after)
}

fn cyclic_padding(points: &[Point]) -> (Vec<Point>, Vec<Point>) {
    let ring = &points[..points.len() - 1];
    let period = points[points.len() - 1].x - points[0].x;
    let at = |i: isize| {
        let count = ring.len() as isize;
        let mut point = ring[i.rem_euclid(count) as usize];
        point.x += i.div_euclid(count) as f32 * period;
        point
    };
    let before = (1..=PADDING as isize).rev().map(|step| at(-step)).collect();
    let after = (1..=PADDING as isize).map(|step| at(ring.len() as isize + step)).collect();
    (before, after)
}

pub fn recalc(points: &mut [Point]) {
    let n = points.len();
    if n < 2 {
        for p in points.iter_mut().filter(|p| p.handle.computed()) {
            p.left = Vec2::new(-1.0, 0.0);
            p.right = Vec2::new(1.0, 0.0);
        }
        return;
    }
    let mut locked: Vec<Option<f64>> = vec![None; n];
    let mut limits = vec![(f64::MIN, f64::MAX); n];
    for i in 0..n {
        if !matches!(points[i].handle, Handle::Auto | Handle::AutoClamped) {
            continue;
        }
        if i == 0 || i == n - 1 {
            locked[i] = Some(0.0);
            continue;
        }
        if points[i].handle == Handle::AutoClamped {
            let before = (points[i].y - points[i - 1].y) as f64;
            let after = (points[i + 1].y - points[i].y) as f64;
            if before * after <= 0.0 {
                locked[i] = Some(0.0);
                continue;
            }
            let w0 = (points[i].x - points[i - 1].x) as f64;
            let w1 = (points[i + 1].x - points[i].x) as f64;
            let most = (3.0 * before / w0).abs().min((3.0 * after / w1).abs());
            limits[i] = if after > 0.0 { (0.0, most) } else { (-most, 0.0) };
        }
    }
    let slopes = loop {
        let slopes = solve(points, &locked);
        let mut clamped = false;
        for i in 0..n {
            if locked[i].is_none() && points[i].handle == Handle::AutoClamped {
                let (low, high) = limits[i];
                if slopes[i].x < low || slopes[i].x > high {
                    locked[i] = Some(slopes[i].x.clamp(low, high));
                    clamped = true;
                }
            }
        }
        if !clamped {
            break slopes;
        }
    };
    for i in 0..n {
        if !points[i].handle.computed() {
            continue;
        }
        let w0 = if i > 0 { points[i].x - points[i - 1].x } else { points[1].x - points[0].x } / 3.0;
        let w1 = if i < n - 1 { points[i + 1].x - points[i].x } else { points[i].x - points[i - 1].x } / 3.0;
        points[i].left = Vec2::new(-w0, -(slopes[i].x as f32) * w0);
        points[i].right = Vec2::new(w1, slopes[i].y as f32 * w1);
    }
}

pub fn slope(handle: Vec2) -> f64 {
    let x = if handle.x.abs() < 1e-6 { 1e-6f32.copysign(handle.x) } else { handle.x };
    (handle.y / x) as f64
}

fn rows(top: [f64; 2], bottom: [f64; 2]) -> DMat2 {
    DMat2::from_cols(DVec2::new(top[0], bottom[0]), DVec2::new(top[1], bottom[1]))
}

fn solve(points: &[Point], locked: &[Option<f64>]) -> Vec<DVec2> {
    let n = points.len();
    let mut carry = vec![DMat2::ZERO; n];
    let mut partial = vec![DVec2::ZERO; n];
    for i in 0..n {
        let p = points[i];
        let w0 = if i > 0 { (p.x - points[i - 1].x) as f64 } else { 1.0 };
        let w1 = if i < n - 1 { (points[i + 1].x - p.x) as f64 } else { 1.0 };
        let dy0 = if i > 0 { (p.y - points[i - 1].y) as f64 } else { 0.0 };
        let dy1 = if i < n - 1 { (points[i + 1].y - p.y) as f64 } else { 0.0 };
        let (mut before, mut after) = (DVec2::ZERO, DVec2::ZERO);
        let (own, value) = match (locked[i], p.handle) {
            (Some(s), _) => (DMat2::IDENTITY, DVec2::splat(s)),
            (None, Handle::Aligned | Handle::Free) => (DMat2::IDENTITY, DVec2::new(slope(p.left), slope(p.right))),
            (None, Handle::Auto | Handle::AutoClamped) => {
                before.y = 2.0 / w0;
                after.y = 2.0 / w1;
                (rows([1.0, -1.0], [4.0 / w0, 4.0 / w1]), DVec2::new(0.0, 6.0 * (dy0 / (w0 * w0) + dy1 / (w1 * w1))))
            }
            (None, Handle::Vector) => {
                let (top, left_value) = if i > 0 {
                    before.x = 1.0;
                    ([2.0, 0.0], 3.0 * dy0 / w0)
                } else {
                    ([1.0, -1.0], 0.0)
                };
                let (bottom, right_value) = if i < n - 1 {
                    after.y = 1.0;
                    ([0.0, 2.0], 3.0 * dy1 / w1)
                } else {
                    ([1.0, -1.0], 0.0)
                };
                (rows(top, bottom), DVec2::new(left_value, right_value))
            }
        };
        let from_previous = DMat2::from_cols(DVec2::ZERO, before);
        let to_next = DMat2::from_cols(after, DVec2::ZERO);
        let (matrix, rhs) = if i == 0 { (own, value) } else { (own - from_previous * carry[i - 1], value - from_previous * partial[i - 1]) };
        if matrix.determinant().abs() < 1e-12 {
            return vec![DVec2::ZERO; n];
        }
        let inverse = matrix.inverse();
        carry[i] = inverse * to_next;
        partial[i] = inverse * rhs;
    }
    let mut slopes = vec![DVec2::ZERO; n];
    slopes[n - 1] = partial[n - 1];
    for i in (0..n - 1).rev() {
        slopes[i] = partial[i] - carry[i] * slopes[i + 1];
    }
    slopes
}

pub fn segment(a: &Point, b: &Point, x: f32) -> f32 {
    let width = (b.x - a.x) as f64;
    let mut out = DVec2::new(a.right.x.max(0.0) as f64, a.right.y as f64);
    let mut into = DVec2::new(b.left.x.min(0.0) as f64, b.left.y as f64);
    let reach = out.x - into.x;
    if reach > width {
        out *= width / reach;
        into *= width / reach;
    }
    let start = DVec2::new(a.x as f64, a.y as f64);
    let end = DVec2::new(b.x as f64, b.y as f64);
    let controls = [start, start + out, end + into, end];
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..48 {
        let mid = (low + high) / 2.0;
        if cubic(controls.map(|c| c.x), mid) < x as f64 {
            low = mid;
        } else {
            high = mid;
        }
    }
    cubic(controls.map(|c| c.y), (low + high) / 2.0) as f32
}

fn cubic(v: [f64; 4], t: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * v[0] + 3.0 * u * u * t * v[1] + 3.0 * u * t * t * v[2] + t * t * t * v[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    const XS: [f32; 5] = [0.0, 10.0, 15.0, 30.0, 40.0];
    const YS: [f32; 5] = [0.0, 5.0, -2.0, 3.0, 3.5];
    const AT: [f32; 9] = [1.0, 3.0, 7.5, 12.0, 14.0, 20.0, 25.0, 33.0, 37.0];

    fn points(xs: &[f32], ys: &[f32], handles: &[Handle]) -> Vec<Point> {
        xs.iter().zip(ys).zip(handles).map(|((&x, &y), &handle)| Point { x, y, handle, left: Vec2::ZERO, right: Vec2::ZERO }).collect()
    }

    fn evaluate(points: &[Point], x: f32) -> f32 {
        let i = points.windows(2).position(|w| x >= w[0].x && x < w[1].x).unwrap();
        segment(&points[i], &points[i + 1], x)
    }

    fn check(mut points: Vec<Point>, expected: &[f32]) {
        recalc(&mut points);
        for (&x, &y) in AT.iter().zip(expected) {
            let got = evaluate(&points, x);
            assert!((got - y).abs() < 1e-3, "at {x}: got {got}, blender {y}");
        }
    }

    fn mixed(rest: Handle, middle: Handle) -> Vec<Point> {
        points(&XS, &YS, &[rest, rest, middle, rest, rest])
    }

    #[test]
    fn auto_clamped_matches_blender() {
        check(mixed(Handle::AutoClamped, Handle::AutoClamped), &[0.14, 1.08, 4.21875, 2.536, -1.272, -0.87037, 1.37037, 3.3285, 3.4865]);
    }

    #[test]
    fn automatic_matches_blender() {
        check(mixed(Handle::Auto, Handle::Auto), &[0.20646, 1.54524, 5.25724, 2.59688, -0.60004, -3.99374, -0.76116, 3.83111, 3.7019]);
    }

    #[test]
    fn vector_beside_auto_clamped_matches_blender() {
        check(mixed(Handle::AutoClamped, Handle::Vector), &[0.14, 1.08, 4.21875, 3.544, 0.072, 0.07407, 1.84259, 3.3285, 3.4865]);
    }

    #[test]
    fn vector_beside_automatic_matches_blender() {
        check(mixed(Handle::Auto, Handle::Vector), &[0.1886, 1.4202, 4.97813, 3.0256, -0.1872, 0.04938, 1.81173, 3.34483, 3.4935]);
    }

    #[test]
    fn free_handle_matches_blender() {
        let mut p = mixed(Handle::AutoClamped, Handle::Free);
        p[2].left = Vec2::new(-3.0, -2.0);
        p[2].right = Vec2::new(5.0, 3.0);
        check(p, &[0.14, 1.08, 4.21875, 0.75, -2.27483, 0.49074, 2.09259, 3.29175, 3.47075]);
    }

    #[test]
    fn matches_blender_with_uneven_spacing() {
        let mut p = points(&[0.0, 2.0, 30.0, 31.0, 50.0], &[0.0, 1.0, 4.0, 4.2, -1.0], &[Handle::AutoClamped; 5]);
        recalc(&mut p);
        for (x, y) in [(1.0, 0.419643), (5.0, 1.781531), (14.0, 2.589002), (30.5, 4.136207), (37.0, 2.971832), (45.0, -0.1092)] {
            assert!((evaluate(&p, x) - y).abs() < 1e-3, "at {x}");
        }
    }

    #[test]
    fn end_keys_stay_flat() {
        let mut p = points(&[0.0, 10.0], &[0.0, 5.0], &[Handle::Auto; 2]);
        recalc(&mut p);
        assert_eq!(p[0].right, Vec2::new(10.0 / 3.0, 0.0));
        assert_eq!(p[1].left, Vec2::new(-10.0 / 3.0, 0.0));
    }

    #[test]
    fn cyclic_ends_share_one_slope() {
        let mut p = points(&[0.0, 10.0, 20.0, 30.0], &[0.0, 5.0, -2.0, 0.0], &[Handle::AutoClamped; 4]);
        recalc_extended(&mut p, Extend::Cyclic);
        let (start, end) = (slope(p[0].left), slope(p[3].right));
        assert!(start.abs() > 0.05, "seam is flat: {start}");
        assert!((start - end).abs() < 1e-3, "seam kinks: {start} vs {end}");
        assert!((slope(p[0].right) - start).abs() < 1e-3);
    }

    #[test]
    fn linear_ends_follow_the_line() {
        let mut p = points(&[0.0, 10.0], &[0.0, 5.0], &[Handle::Auto; 2]);
        recalc_extended(&mut p, Extend::Linear);
        assert!((slope(p[0].left) - 0.5).abs() < 1e-3);
        assert!((slope(p[1].right) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn dragging_an_auto_handle_keeps_the_other_side_aligned() {
        let mut p = Point { x: 0.0, y: 0.0, handle: Handle::Auto, left: Vec2::new(-2.0, -1.0), right: Vec2::new(3.0, 1.5) };
        p.drag_handle(Side::Right, Vec2::new(4.0, -4.0));
        assert_eq!(p.handle, Handle::Aligned);
        assert!((p.left.length() - Vec2::new(-2.0, -1.0).length()).abs() < 1e-5);
        assert!(p.left.perp_dot(p.right).abs() < 1e-5 && p.left.dot(p.right) < 0.0);
    }

    #[test]
    fn handles_cannot_cross_their_key() {
        let mut p = Point { x: 0.0, y: 0.0, handle: Handle::Vector, left: Vec2::new(-2.0, 0.0), right: Vec2::new(2.0, 0.0) };
        p.drag_handle(Side::Left, Vec2::new(5.0, 1.0));
        assert_eq!(p.handle, Handle::Free);
        assert!(p.left.x < 0.0);
        assert_eq!(p.right, Vec2::new(2.0, 0.0));
    }
}
