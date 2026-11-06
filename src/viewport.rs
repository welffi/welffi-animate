use crate::camera::Camera;
use crate::editor::Editor;
use crate::render::{PartMesh, Scene3d};
use crate::rig::{JOINT_COUNT, LEFT_LEG, RIGHT_LEG};
use crate::transform::{Axis, Mode, Transform};
use eframe::egui::{self, Color32, Event, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};
use eframe::egui_wgpu::RenderState;
use glam::{Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const RING_RADIUS: f32 = 80.0;
const VIEW_RING_RADIUS: f32 = 96.0;
const AXIS_COLORS: [Color32; 3] = [Color32::from_rgb(232, 64, 80), Color32::from_rgb(120, 200, 40), Color32::from_rgb(60, 130, 240)];
const SELECTED_TINT: [f32; 4] = [1.0, 0.55, 0.12, 0.22];
const ACTIVE_TINT: [f32; 4] = [1.0, 0.62, 0.2, 0.32];
const HOVER_TINT: [f32; 4] = [1.0, 1.0, 1.0, 0.08];
const FOOT_TARGET_SIZE: f32 = 6.0;

pub struct Viewport {
    pub camera: Camera,
    pub local: bool,
    scene: Scene3d,
    bounds: [(Vec3, Vec3); JOINT_COUNT],
    transform: Option<Transform>,
    swallow_click: bool,
    pub hovered: bool,
}

impl Viewport {
    pub fn new(rs: &RenderState, meshes: [PartMesh; JOINT_COUNT], skin_png: &[u8]) -> Viewport {
        let scene = Scene3d::new(rs, &meshes, skin_png);
        let bounds = meshes.each_ref().map(|m| (m.min, m.max));
        Viewport { camera: Camera::default(), local: true, scene, bounds, transform: None, swallow_click: false, hovered: false }
    }

    pub fn busy(&self) -> bool {
        self.transform.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, rs: &RenderState, ed: &mut Editor) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        self.hovered = response.hovered() || response.dragged();
        let pointer = ui.input(|i| i.pointer.hover_pos());

        self.navigate(ui, &response, rect);

        let mut poses = ed.poses();
        let mut worlds = ed.rig.worlds(&poses);
        let hovered_part = pointer.filter(|p| rect.contains(*p) && self.transform.is_none()).and_then(|p| self.pick(rect, p, ed, &worlds));

        if self.transform.is_some() {
            self.run_transform(ui, rect, ed);
        } else {
            let gizmo = self.gizmo(ed, rect);
            let ring = pointer.and_then(|p| gizmo.as_ref().and_then(|g| g.ring_at(p)));
            if response.drag_started_by(PointerButton::Primary) && !ui.input(|i| i.modifiers.alt) {
                let origin = ui.input(|i| i.pointer.press_origin());
                if let (Some(g), Some(origin)) = (gizmo.as_ref(), origin) {
                    if let Some(axis) = g.ring_at(origin) {
                        self.transform = Transform::begin(ed, Mode::Rotate, axis, origin, g.center, &self.camera, rect, true);
                    }
                }
            }
            if response.clicked() && ring.is_none_or(|axis| axis == Axis::Trackball) && !self.swallow_click {
                self.click_select(ui, ed, hovered_part);
            }
            if !ui.input(|i| i.pointer.any_down()) {
                self.swallow_click = false;
            }
            if self.hovered && !ui.ctx().egui_wants_keyboard_input() {
                self.shortcuts(ui, rect, ed);
            }
        }
        if self.transform.is_some() {
            poses = ed.poses();
            worlds = ed.rig.worlds(&poses);
        }

        let tints: [[f32; 4]; JOINT_COUNT] = std::array::from_fn(|j| {
            if ed.active == Some(j) && ed.parts.contains(&j) {
                ACTIVE_TINT
            } else if ed.parts.contains(&j) {
                SELECTED_TINT
            } else if hovered_part == Some(j) {
                HOVER_TINT
            } else {
                [0.0; 4]
            }
        });
        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        let texture = self.scene.render(rs, size, self.camera.view_proj(rect), &worlds, &tints);
        let painter = ui.painter_at(rect);
        painter.image(texture, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        if self.transform.is_none() {
            if let Some(g) = self.gizmo(ed, rect) {
                let ring = pointer.and_then(|p| g.ring_at(p));
                g.draw(painter.clone(), ring);
            }
        }
        self.overlay(&painter, rect, ed);
    }

    fn navigate(&mut self, ui: &egui::Ui, response: &egui::Response, rect: Rect) {
        let (shift, ctrl, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.command, i.modifiers.alt));
        let delta = response.drag_delta();
        let middle = response.dragged_by(PointerButton::Middle);
        let alt_left = alt && response.dragged_by(PointerButton::Primary) && self.transform.is_none();
        if middle || alt_left {
            if shift {
                self.camera.pan(rect, delta);
            } else if ctrl {
                self.camera.zoom(-delta.y / 30.0);
            } else {
                self.camera.orbit(delta);
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta().y);
            if scroll != 0.0 {
                self.camera.zoom(scroll / 50.0);
            }
        }
    }

    fn click_select(&mut self, ui: &egui::Ui, ed: &mut Editor, hit: Option<usize>) {
        let shift = ui.input(|i| i.modifiers.shift);
        match hit {
            Some(j) if shift && ed.parts.contains(&j) && ed.active == Some(j) => {
                ed.parts.remove(&j);
                ed.active = ed.parts.iter().next_back().copied();
            }
            Some(j) if shift => {
                ed.parts.insert(j);
                ed.active = Some(j);
            }
            Some(j) => {
                ed.parts = [j].into();
                ed.active = Some(j);
            }
            None if !shift => {
                ed.parts.clear();
                ed.active = None;
            }
            None => {}
        }
    }

    fn shortcuts(&mut self, ui: &egui::Ui, rect: Rect, ed: &mut Editor) {
        let (pressed, alt) = ui.input(|i| {
            let keys: Vec<Key> = i.events.iter().filter_map(|e| match e {
                Event::Key { key, pressed: true, repeat: false, modifiers, .. } if !modifiers.command => Some(*key),
                _ => None,
            }).collect();
            (keys, i.modifiers.alt)
        });
        for key in pressed {
            match key {
                Key::R if alt => ed.clear_pose(true),
                Key::G if alt => ed.clear_pose(false),
                Key::R | Key::G => {
                    let mode = if key == Key::R { Mode::Rotate } else { Mode::Grab };
                    let mouse = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
                    let center = self.selection_center(ed);
                    self.transform = Transform::begin(ed, mode, Axis::View, mouse, center, &self.camera, rect, false);
                    if self.transform.is_none() {
                        ed.status = "Click a part first".to_string();
                    }
                }
                Key::A if alt => {
                    ed.parts.clear();
                    ed.active = None;
                }
                Key::A => {
                    ed.parts = (0..JOINT_COUNT).collect();
                    ed.active = ed.active.or(Some(0));
                }
                Key::L => self.local = !self.local,
                Key::Num1 | Key::Num3 | Key::Num7 => self.view_key(key, ui.input(|i| i.modifiers.ctrl)),
                Key::Home => {
                    self.camera.target = Vec3::new(0.0, 2.6, 0.0);
                    self.camera.distance = 12.0;
                }
                _ => {}
            }
        }
    }

    fn view_key(&mut self, key: Key, opposite: bool) {
        let flip = if opposite { PI } else { 0.0 };
        match key {
            Key::Num1 => self.camera.look_from(PI + flip, 0.0),
            Key::Num3 => self.camera.look_from(FRAC_PI_2 + flip, 0.0),
            _ => self.camera.look_from(PI, if opposite { -FRAC_PI_2 } else { FRAC_PI_2 }),
        }
    }

    fn run_transform(&mut self, ui: &egui::Ui, rect: Rect, ed: &mut Editor) {
        let Some(mut t) = self.transform.take() else { return };
        let (events, snap, mouse, primary_down) = ui.input(|i| (i.events.clone(), i.modifiers.command, i.pointer.hover_pos(), i.pointer.primary_down()));
        let mut finish: Option<bool> = None;
        for event in &events {
            match event {
                Event::Key { key, pressed: true, .. } => match key {
                    Key::R if t.mode == Mode::Rotate => t.axis = if t.axis == Axis::Trackball { Axis::View } else { Axis::Trackball },
                    Key::X => t.axis = t.axis.pressed(0),
                    Key::Y => t.axis = t.axis.pressed(1),
                    Key::Z => t.axis = t.axis.pressed(2),
                    Key::Backspace => {
                        t.typed.pop();
                    }
                    Key::Enter => finish = Some(true),
                    Key::Escape => finish = Some(false),
                    _ => {}
                },
                Event::Text(text) => {
                    for c in text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-') {
                        t.typed.push(c);
                    }
                }
                Event::PointerButton { button: PointerButton::Primary, pressed: true, .. } if !t.from_gizmo => finish = Some(true),
                Event::PointerButton { button: PointerButton::Secondary, pressed: true, .. } => finish = Some(false),
                _ => {}
            }
        }
        if t.from_gizmo && !primary_down && finish.is_none() {
            finish = Some(true);
        }
        if let Some(mouse) = mouse {
            t.update(ed, &self.camera, rect, mouse, snap);
        }
        ed.status = t.status();
        match finish {
            Some(true) => {
                self.swallow_click = primary_down;
                let joints = t.joints.clone();
                ed.finish_pose_edit(&joints);
                ed.status = if ed.auto_key { format!("Keyed at frame {}", ed.current()) } else { "Posed (not keyed, auto key is off)".to_string() };
            }
            Some(false) => {
                t.cancel(ed);
                ed.status = "Cancelled".to_string();
            }
            None => self.transform = Some(t),
        }
    }

    fn selection_center(&self, ed: &Editor) -> Vec3 {
        let poses = ed.poses();
        let worlds = ed.rig.worlds(&poses);
        match ed.active.filter(|a| ed.parts.contains(a)).or(ed.parts.iter().next().copied()) {
            Some(j) => ed.rig.pivot(j, &poses, &worlds),
            None => Vec3::ZERO,
        }
    }

    fn foot_targets(&self, ed: &Editor, rect: Rect) -> Vec<(usize, Pos2)> {
        let poses = ed.poses();
        let worlds = ed.rig.worlds(&poses);
        [LEFT_LEG, RIGHT_LEG].into_iter().filter_map(|j| Some((j, self.camera.project(rect, ed.rig.pivot(j, &poses, &worlds))?))).collect()
    }

    fn pick(&self, rect: Rect, pos: Pos2, ed: &Editor, worlds: &[Mat4; JOINT_COUNT]) -> Option<usize> {
        if let Some((j, _)) = self.foot_targets(ed, rect).into_iter().find(|(_, p)| p.distance(pos) < FOOT_TARGET_SIZE + 3.0) {
            return Some(j);
        }
        let (origin, dir) = self.camera.ray(rect, pos);
        let mut best: Option<(f32, usize)> = None;
        for (j, world) in worlds.iter().enumerate() {
            let inverse = world.inverse();
            let o = inverse.transform_point3(origin);
            let d = inverse.transform_vector3(dir);
            if let Some(t) = ray_box(o, d, self.bounds[j].0, self.bounds[j].1) {
                if best.is_none_or(|(b, _)| t < b) {
                    best = Some((t, j));
                }
            }
        }
        best.map(|(_, j)| j)
    }

    fn gizmo(&self, ed: &Editor, rect: Rect) -> Option<Gizmo> {
        let active = ed.active.filter(|a| ed.parts.contains(a))?;
        let poses = ed.poses();
        let worlds = ed.rig.worlds(&poses);
        let center = ed.rig.pivot(active, &poses, &worlds);
        let screen_center = self.camera.project(rect, center)?;
        let control = ed.rig.control(active, &poses, &worlds);
        let axes: [Vec3; 3] = std::array::from_fn(|i| if self.local { control.col(i).truncate().normalize() } else { Vec3::AXES[i] });
        let radius = RING_RADIUS * self.camera.units_per_pixel(rect, center);
        let toward = self.camera.toward_viewer();
        let rings = std::array::from_fn(|i| {
            let axis: Vec3 = axes[i];
            let u = axis.any_orthonormal_vector();
            let v = axis.cross(u);
            let points: Vec<(Pos2, bool)> = (0..=64)
                .map(|s| {
                    let a = s as f32 / 64.0 * TAU;
                    let offset = (u * a.cos() + v * a.sin()) * radius;
                    let front = offset.dot(toward) >= -0.05 * radius;
                    (self.camera.project(rect, center + offset).unwrap_or(screen_center), front)
                })
                .collect();
            points
        });
        Some(Gizmo { center, screen_center, rings, local: self.local })
    }

    fn overlay(&self, painter: &egui::Painter, rect: Rect, ed: &Editor) {
        for (j, p) in self.foot_targets(ed, rect) {
            let square = Rect::from_center_size(p, Vec2::splat(FOOT_TARGET_SIZE * 2.0));
            if ed.parts.contains(&j) {
                painter.rect_filled(square, 1.0, Color32::from_rgb(255, 160, 50));
            }
            painter.rect_stroke(square, 1.0, Stroke::new(1.5, Color32::from_gray(230)), egui::StrokeKind::Middle);
        }
        let text = if self.local { "Local  (L)" } else { "Global  (L)" };
        painter.text(rect.left_top() + Vec2::new(10.0, 8.0), egui::Align2::LEFT_TOP, text, egui::FontId::proportional(13.0), Color32::from_gray(170));
        let frame = format!("Frame {}", ed.current());
        painter.text(rect.right_top() + Vec2::new(-10.0, 8.0), egui::Align2::RIGHT_TOP, frame, egui::FontId::proportional(13.0), Color32::from_gray(170));
        crate::hints::paint(painter, rect);
    }
}

struct Gizmo {
    center: Vec3,
    screen_center: Pos2,
    rings: [Vec<(Pos2, bool)>; 3],
    local: bool,
}

impl Gizmo {
    fn ring_at(&self, p: Pos2) -> Option<Axis> {
        let mut best: Option<(f32, usize)> = None;
        for (i, ring) in self.rings.iter().enumerate() {
            for w in ring.windows(2) {
                if !(w[0].1 && w[1].1) {
                    continue;
                }
                let d = segment_distance(p, w[0].0, w[1].0);
                if d < 7.0 && best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, i));
                }
            }
        }
        if let Some((_, i)) = best {
            return Some(if self.local { Axis::Local(i) } else { Axis::Global(i) });
        }
        let distance = p.distance(self.screen_center);
        if (distance - VIEW_RING_RADIUS).abs() < 6.0 {
            Some(Axis::View)
        } else {
            (distance < RING_RADIUS).then_some(Axis::Trackball)
        }
    }

    fn draw(&self, painter: egui::Painter, hovered: Option<Axis>) {
        if hovered == Some(Axis::Trackball) {
            painter.circle_filled(self.screen_center, RING_RADIUS, Color32::from_white_alpha(18));
        }
        let view_hot = hovered == Some(Axis::View);
        painter.circle_stroke(self.screen_center, VIEW_RING_RADIUS, Stroke::new(if view_hot { 3.0 } else { 1.5 }, Color32::from_gray(if view_hot { 255 } else { 200 })));
        for (i, ring) in self.rings.iter().enumerate() {
            let hot = matches!(hovered, Some(Axis::Local(j)) | Some(Axis::Global(j)) if j == i);
            let color = if hot { AXIS_COLORS[i].gamma_multiply(1.3) } else { AXIS_COLORS[i] };
            let stroke = Stroke::new(if hot { 3.5 } else { 2.0 }, color);
            for w in ring.windows(2) {
                if w[0].1 && w[1].1 {
                    painter.line_segment([w[0].0, w[1].0], stroke);
                }
            }
        }
        painter.circle_filled(self.screen_center, 2.5, Color32::WHITE);
    }
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = dir.recip();
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let near = t1.min(t2).max_element();
    let far = t1.max(t2).min_element();
    (far >= near.max(0.0)).then_some(near.max(0.0))
}
