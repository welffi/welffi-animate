use crate::render::{PartMesh, Vertex};
use crate::rig::{Pose, BODY_PARTS, JOINT_COUNT, LEFT_ARM, RIGHT_ARM};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

const BALL_RINGS: usize = 12;
const BALL_SEGMENTS: usize = 24;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Block(Vec3),
    Ball(Vec3),
    Obj(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    pub name: String,
    pub parent: usize,
    pub rest: Pose,
    pub color: [u8; 3],
    pub shape: Shape,
    #[serde(default)]
    pub hidden: bool,
}

impl Prop {
    pub fn new(name: String, shape: Shape) -> Prop {
        Prop { name, parent: RIGHT_ARM, rest: grip(RIGHT_ARM), color: [200, 200, 205], shape, hidden: false }
    }
}

pub fn grip(parent: usize) -> Pose {
    match parent {
        LEFT_ARM | RIGHT_ARM => Pose { pos: Vec3::new(0.0, -1.0, 0.0), rot: Quat::IDENTITY },
        _ => Pose::IDENTITY,
    }
}

pub fn is_inside(props: &[Prop], joint: usize, ancestor: usize) -> bool {
    let mut at = joint;
    for _ in 0..=props.len() {
        if at == ancestor {
            return true;
        }
        match at.checked_sub(JOINT_COUNT).and_then(|k| props.get(k)) {
            Some(prop) => at = prop.parent,
            None => return false,
        }
    }
    false
}

pub fn mesh(shape: &Shape) -> PartMesh {
    let vertices = match shape {
        Shape::Block(size) => block(*size),
        Shape::Ball(size) => ball(*size),
        Shape::Obj(text) => parse_obj(text),
    };
    let (min, max) = bounds(&vertices);
    PartMesh { vertices, min, max }
}

fn bounds(vertices: &[Vertex]) -> (Vec3, Vec3) {
    if vertices.is_empty() {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    vertices.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), v| (lo.min(Vec3::from(v.pos)), hi.max(Vec3::from(v.pos))))
}

fn vertex(pos: Vec3, normal: Vec3) -> Vertex {
    Vertex { pos: pos.into(), normal: normal.into(), uv: [0.0, 0.0] }
}

fn block(size: Vec3) -> Vec<Vertex> {
    let half = size / 2.0;
    let mut out = Vec::new();
    for axis in 0..3 {
        for sign in [-1.0, 1.0] {
            let normal = Vec3::AXES[axis] * sign;
            let u = Vec3::AXES[(axis + 1) % 3];
            let v = normal.cross(u);
            let corner = |a: f32, b: f32| (normal + u * a + v * b) * half;
            let quad = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
            for i in [0, 1, 2, 0, 2, 3] {
                out.push(vertex(quad[i], normal));
            }
        }
    }
    out
}

fn ball(size: Vec3) -> Vec<Vertex> {
    let half = size / 2.0;
    let point = |ring: usize, segment: usize| {
        let (lat, lon) = (ring as f32 / BALL_RINGS as f32 * PI, segment as f32 / BALL_SEGMENTS as f32 * TAU);
        let unit = Vec3::new(lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin());
        vertex(unit * half, (unit / half.max(Vec3::splat(1e-4))).normalize_or_zero())
    };
    let mut out = Vec::new();
    for ring in 0..BALL_RINGS {
        for segment in 0..BALL_SEGMENTS {
            let (a, b) = (point(ring, segment), point(ring, segment + 1));
            let (c, d) = (point(ring + 1, segment), point(ring + 1, segment + 1));
            out.extend([a, c, b, b, c, d]);
        }
    }
    out
}

pub fn parse_obj(text: &str) -> Vec<Vertex> {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut out = Vec::new();
    let resolve = |index: &str, count: usize| -> Option<usize> {
        let i: i64 = index.parse().ok()?;
        let at = if i < 0 { count as i64 + i } else { i - 1 };
        (0..count as i64).contains(&at).then_some(at as usize)
    };
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let numbers = |words: std::str::SplitWhitespace| -> Vec3 {
            let v: Vec<f32> = words.take(3).filter_map(|w| w.parse().ok()).collect();
            Vec3::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0))
        };
        match words.next() {
            Some("v") => positions.push(numbers(words)),
            Some("vn") => normals.push(numbers(words)),
            Some("f") => {
                let corners: Vec<(Vec3, Option<Vec3>)> = words
                    .filter_map(|word| {
                        let mut parts = word.split('/');
                        let pos = positions[resolve(parts.next()?, positions.len())?];
                        let normal = parts.nth(1).and_then(|n| resolve(n, normals.len())).map(|n| normals[n]);
                        Some((pos, normal))
                    })
                    .collect();
                for i in 1..corners.len().saturating_sub(1) {
                    let tri = [corners[0], corners[i], corners[i + 1]];
                    let flat = (tri[1].0 - tri[0].0).cross(tri[2].0 - tri[0].0).normalize_or_zero();
                    out.extend(tri.map(|(pos, normal)| vertex(pos, normal.unwrap_or(flat))));
                }
            }
            _ => {}
        }
    }
    centered(out)
}

fn centered(mut vertices: Vec<Vertex>) -> Vec<Vertex> {
    let (min, max) = bounds(&vertices);
    let center = (min + max) / 2.0;
    for v in &mut vertices {
        v.pos = (Vec3::from(v.pos) - center).into();
    }
    vertices
}

pub fn linear_color(color: [u8; 3]) -> [f32; 4] {
    let c = |v: u8| (v as f32 / 255.0).powf(2.2);
    [c(color[0]), c(color[1]), c(color[2]), 1.0]
}

pub fn unique_name(props: &[Prop], wanted: &str, skip: Option<usize>) -> String {
    let base = wanted.trim();
    let base = if base.is_empty() { "Prop" } else { base };
    let taken = |name: &str| BODY_PARTS.contains(&name) || props.iter().enumerate().any(|(i, p)| Some(i) != skip && p.name == name);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}{n}")).find(|name| !taken(name)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_faces_become_centered_triangles() {
        let text = "v 0 0 0\nv 2 0 0\nv 2 2 0\nv 0 2 0\nvn 0 0 1\nf 1//1 2//1 3//1 4//1\n";
        let vertices = parse_obj(text);
        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0].pos, [-1.0, -1.0, 0.0]);
        assert_eq!(vertices[0].normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn names_stay_unique_and_never_take_a_body_part() {
        let props = vec![Prop::new("Sword".into(), Shape::Block(Vec3::ONE))];
        assert_eq!(unique_name(&props, "Sword", None), "Sword2");
        assert_eq!(unique_name(&props, "Sword", Some(0)), "Sword");
        assert_eq!(unique_name(&props, "Right Arm", None), "Right Arm2");
        assert_eq!(unique_name(&props, "  ", None), "Prop");
    }
}
