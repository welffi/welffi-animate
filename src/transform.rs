use crate::camera::Camera;
use crate::editor::Editor;
use crate::rig::{Pose, JOINT_COUNT};
use eframe::egui::{Pos2, Rect};
use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Rotate,
    Grab,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Axis {
    View,
    Trackball,
    Global(usize),
    Local(usize),
}

impl Axis {
    pub fn pressed(self, i: usize) -> Axis {
        match self {
            Axis::Global(j) if j == i => Axis::Local(i),
            Axis::Local(j) if j == i => Axis::View,
            _ => Axis::Global(i),
        }
    }

    pub fn label(self) -> String {
        const NAMES: [&str; 3] = ["X", "Y", "Z"];
        match self {
            Axis::View => "free".to_string(),
            Axis::Trackball => "trackball".to_string(),
            Axis::Global(i) => format!("global {}", NAMES[i]),
            Axis::Local(i) => format!("local {}", NAMES[i]),
        }
    }
}

pub struct Transform {
    pub mode: Mode,
    pub axis: Axis,
    pub joints: Vec<usize>,
    pub from_gizmo: bool,
    pub typed: String,
    start_mouse: Pos2,
    center: Vec3,
    start_worlds: [Mat4; JOINT_COUNT],
    start_controls: [Mat4; JOINT_COUNT],
    pivots: [Vec3; JOINT_COUNT],
    start_overrides: [Option<Pose>; JOINT_COUNT],
    angle: f32,
    last_screen_angle: f32,
}

impl Transform {
    pub fn begin(ed: &Editor, mode: Mode, axis: Axis, mouse: Pos2, center: Vec3, camera: &Camera, rect: Rect, from_gizmo: bool) -> Option<Transform> {
        let joints = top_level(ed);
        if joints.is_empty() {
            return None;
        }
        let poses = ed.poses();
        let worlds = ed.rig.worlds(&poses);
        let pivots = std::array::from_fn(|j| ed.rig.pivot(j, &poses, &worlds));
        let controls = std::array::from_fn(|j| ed.rig.control(j, &poses, &worlds));
        let screen_angle = camera.project(rect, center).map_or(0.0, |c| screen_angle(c, mouse));
        Some(Transform {
            mode,
            axis,
            joints,
            from_gizmo,
            typed: String::new(),
            start_mouse: mouse,
            center,
            start_worlds: worlds,
            start_controls: controls,
            pivots,
            start_overrides: ed.overrides,
            angle: 0.0,
            last_screen_angle: screen_angle,
        })
    }

    pub fn update(&mut self, ed: &mut Editor, camera: &Camera, rect: Rect, mouse: Pos2, snap: bool) {
        let typed: Option<f32> = self.typed.parse().ok();
        match self.mode {
            Mode::Rotate => {
                if let Some(c) = camera.project(rect, self.center) {
                    let now = screen_angle(c, mouse);
                    let mut step = now - self.last_screen_angle;
                    step -= (step / std::f32::consts::TAU).round() * std::f32::consts::TAU;
                    self.angle += step;
                    self.last_screen_angle = now;
                }
                let angle = snapped(self.angle, snap);
                for &j in &self.joints {
                    let rotation = if self.axis == Axis::Trackball {
                        trackball(camera, mouse - self.start_mouse, snap)
                    } else {
                        let axis = self.axis_vector(j, camera);
                        let signed = match typed {
                            Some(degrees) => degrees.to_radians(),
                            None if axis.dot(camera.toward_viewer()) >= 0.0 => angle,
                            None => -angle,
                        };
                        Quat::from_axis_angle(axis, signed)
                    };
                    let p = self.pivots[j];
                    let spin = Mat4::from_translation(p) * Mat4::from_quat(rotation) * Mat4::from_translation(-p);
                    self.place(ed, j, spin * self.start_controls[j]);
                }
            }
            Mode::Grab => {
                let delta = mouse - self.start_mouse;
                for &j in &self.joints {
                    let mut offset = match (self.axis, typed) {
                        (Axis::View, _) => {
                            let scale = camera.units_per_pixel(rect, self.center);
                            (camera.right() * delta.x - camera.up() * delta.y) * scale
                        }
                        (_, Some(amount)) => self.axis_vector(j, camera) * amount,
                        _ => {
                            let axis = self.axis_vector(j, camera);
                            axis * along_screen(camera, rect, self.center, axis, delta)
                        }
                    };
                    if snap {
                        offset = (offset * 10.0).round() / 10.0;
                    }
                    self.place(ed, j, Mat4::from_translation(offset) * self.start_controls[j]);
                }
            }
        }
    }

    fn place(&self, ed: &mut Editor, joint: usize, world: Mat4) {
        ed.overrides[joint] = Some(ed.rig.pose_for_control(joint, &self.start_worlds, world));
    }

    pub fn axis_vector(&self, joint: usize, camera: &Camera) -> Vec3 {
        match self.axis {
            Axis::View | Axis::Trackball => camera.toward_viewer(),
            Axis::Global(i) => Vec3::AXES[i],
            Axis::Local(i) => self.start_controls[joint].col(i).truncate().normalize(),
        }
    }

    pub fn cancel(self, ed: &mut Editor) {
        ed.overrides = self.start_overrides;
    }

    pub fn status(&self) -> String {
        let what = match self.mode {
            Mode::Rotate => "Rotate",
            Mode::Grab => "Move",
        };
        let amount = if self.typed.is_empty() { String::new() } else { format!("  {}", self.typed) };
        format!("{what} ({}){amount}   X/Y/Z axis, type a number, click or Enter to confirm, right-click or Esc to cancel", self.axis.label())
    }
}

fn top_level(ed: &Editor) -> Vec<usize> {
    ed.parts.iter().copied().filter(|&j| ed.rig.joints[j].parent.is_none_or(|p| !ed.parts.contains(&p))).collect()
}

fn snapped(angle: f32, snap: bool) -> f32 {
    if snap { (angle.to_degrees() / 5.0).round().to_radians() * 5.0 } else { angle }
}

fn trackball(camera: &Camera, delta: eframe::egui::Vec2, snap: bool) -> Quat {
    const RADIANS_PER_PIXEL: f32 = 0.01;
    let turn = snapped(delta.x * RADIANS_PER_PIXEL, snap);
    let tip = snapped(delta.y * RADIANS_PER_PIXEL, snap);
    Quat::from_axis_angle(camera.up(), turn) * Quat::from_axis_angle(camera.right(), tip)
}

fn screen_angle(center: Pos2, mouse: Pos2) -> f32 {
    (-(mouse.y - center.y)).atan2(mouse.x - center.x)
}

fn along_screen(camera: &Camera, rect: Rect, center: Vec3, axis: Vec3, delta: eframe::egui::Vec2) -> f32 {
    let (Some(a), Some(b)) = (camera.project(rect, center), camera.project(rect, center + axis)) else { return 0.0 };
    let dir = b - a;
    let length = dir.length();
    if length < 1.0 {
        return 0.0;
    }
    delta.dot(dir / length) / length
}
