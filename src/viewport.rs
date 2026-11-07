use crate::camera::{ray_box, Camera};
use crate::editor::Editor;
use crate::gizmo::{Gizmo, RING_RADIUS};
use crate::partner;
use crate::props::{mesh, part_color, Shape};
use crate::render::{Draw, PartMesh, Scene3d};
use crate::rig::{is_hip, JOINT_COUNT, LEFT_HIP, LEFT_LEG, RIGHT_HIP, RIGHT_LEG};
use crate::transform::{Axis, Mode, Transform};
use eframe::egui::{self, Color32, Event, Key, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};
use eframe::egui_wgpu::RenderState;
use glam::{Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const SELECTED_TINT: [f32; 4] = [1.0, 0.55, 0.12, 0.22];
const ACTIVE_TINT: [f32; 4] = [1.0, 0.62, 0.2, 0.32];
const HOVER_TINT: [f32; 4] = [1.0, 1.0, 1.0, 0.08];
const FOOT_TARGET_SIZE: f32 = 6.0;

pub struct Viewport {
    pub camera: Camera,
    pub local: bool,
    scene: Scene3d,
    body_bounds: Vec<(Vec3, Vec3)>,
    bounds: Vec<Vec<(Vec3, Vec3)>>,
    shapes: Vec<Vec<Shape>>,
    transform: Option<Transform>,
    swallow_click: bool,
    switch_next: bool,
    pub hovered: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Part(usize),
    Rig,
}

impl Viewport {
    pub fn new(rs: &RenderState, meshes: Vec<PartMesh>, skin_png: &[u8]) -> Viewport {
        let scene = Scene3d::new(rs, &meshes, skin_png);
        let body_bounds: Vec<(Vec3, Vec3)> = meshes.iter().map(|m| (m.min, m.max)).collect();
        let bounds = vec![body_bounds.clone()];
        Viewport { camera: Camera::default(), local: true, scene, body_bounds, bounds, shapes: Vec::new(), transform: None, swallow_click: false, switch_next: false, hovered: false }
    }

    fn sync_props(&mut self, rs: &RenderState, ed: &Editor) {
        let actors: Vec<&[crate::props::Prop]> = std::iter::once(ed.anim.props.as_slice()).chain(ed.anim.partner.as_ref().map(|p| p.props.as_slice())).collect();
        let shapes: Vec<Vec<Shape>> = actors.iter().map(|props| props.iter().map(|p| p.shape.clone()).collect()).collect();
        if self.shapes == shapes {
            return;
        }
        let meshes: Vec<Vec<PartMesh>> = actors.iter().map(|props| props.iter().map(|p| mesh(&p.shape)).collect()).collect();
        self.scene.set_actors(rs, &meshes);
        self.bounds = meshes.iter().map(|actor| self.body_bounds.iter().copied().chain(actor.iter().map(|m| (m.min, m.max))).collect()).collect();
        self.shapes = shapes;
    }

    pub fn frame_pair(&mut self) {
        self.camera.target = Vec3::new(0.0, 2.6, -2.0);
        self.camera.distance = 16.0;
        self.camera.look_from(FRAC_PI_2 + 0.3, 0.2);
    }

    pub fn switch_rig(&mut self, ed: &mut Editor) {
        if let Some(stand) = ed.switch_rig() {
            self.camera.carry(stand);
        }
    }

    fn draws(&self, ed: &Editor, worlds: &[Mat4], hit: Option<Hit>) -> Vec<Draw> {
        let hovered_part = match hit {
            Some(Hit::Part(j)) => Some(j),
            _ => None,
        };
        let tint = |j: usize| {
            if ed.active == Some(j) && ed.parts.contains(&j) {
                ACTIVE_TINT
            } else if ed.parts.contains(&j) {
                SELECTED_TINT
            } else if hovered_part == Some(j) {
                HOVER_TINT
            } else {
                [0.0; 4]
            }
        };
        let mut draws: Vec<Draw> = worlds.iter().enumerate().map(|(j, &world)| Draw { world, tint: tint(j), color: part_color(&ed.anim.props, j) }).collect();
        draws.extend(partner::draws(ed, hit == Some(Hit::Rig)));
        draws
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
        self.sync_props(rs, ed);
        let hit = pointer.filter(|p| rect.contains(*p) && self.transform.is_none()).and_then(|p| self.pick(rect, p, ed, &worlds));
        let hovered_part = match hit {
            Some(Hit::Part(j)) => Some(j),
            _ => None,
        };

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
                if hit == Some(Hit::Rig) {
                    self.switch_next = true;
                } else {
                    self.click_select(ui, ed, hovered_part);
                }
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

        let draws = self.draws(ed, &worlds, hit);
        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        let texture = self.scene.render(rs, size, self.camera.view_proj(rect), &draws);
        let painter = ui.painter_at(rect);
        painter.image(texture, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        if self.transform.is_none() {
            if let Some(g) = self.gizmo(ed, rect) {
                let ring = pointer.and_then(|p| g.ring_at(p));
                g.draw(painter.clone(), ring);
            }
        }
        self.overlay(&painter, rect, ed);
        if std::mem::take(&mut self.switch_next) {
            self.switch_rig(ed);
        }
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
                    ed.parts = (0..ed.anim.tracks.len()).collect();
                    ed.active = ed.active.or(Some(0));
                }
                Key::L => self.local = !self.local,
                Key::Tab => self.switch_next = ed.anim.partner.is_some(),
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

    fn handles(&self, ed: &Editor, rect: Rect) -> Vec<(usize, Pos2)> {
        let poses = ed.poses();
        let worlds = ed.rig.worlds(&poses);
        [LEFT_LEG, RIGHT_LEG, LEFT_HIP, RIGHT_HIP].into_iter().filter_map(|j| Some((j, self.camera.project(rect, ed.rig.pivot(j, &poses, &worlds))?))).collect()
    }

    fn pick(&self, rect: Rect, pos: Pos2, ed: &Editor, worlds: &[Mat4]) -> Option<Hit> {
        if let Some((j, _)) = self.handles(ed, rect).into_iter().find(|(_, p)| p.distance(pos) < FOOT_TARGET_SIZE + 3.0) {
            return Some(Hit::Part(j));
        }
        let own = self.nearest_part(rect, pos, ed, worlds);
        let other = self.bounds.get(1).and_then(|bounds| partner::nearest(&self.camera, rect, pos, ed, bounds));
        match (own, other) {
            (Some((near, _)), Some(far)) if far < near => Some(Hit::Rig),
            (Some((_, j)), _) => Some(Hit::Part(j)),
            (None, Some(_)) => Some(Hit::Rig),
            (None, None) => None,
        }
    }

    fn nearest_part(&self, rect: Rect, pos: Pos2, ed: &Editor, worlds: &[Mat4]) -> Option<(f32, usize)> {
        let (origin, dir) = self.camera.ray(rect, pos);
        let mut best: Option<(f32, usize)> = None;
        for (j, (world, (min, max))) in worlds.iter().zip(&self.bounds[0]).enumerate() {
            if is_hip(j) || j >= JOINT_COUNT && ed.anim.props[j - JOINT_COUNT].hidden {
                continue;
            }
            let inverse = world.inverse();
            let o = inverse.transform_point3(origin);
            let d = inverse.transform_vector3(dir);
            if let Some(t) = ray_box(o, d, *min, *max) {
                if best.is_none_or(|(b, _)| t < b) {
                    best = Some((t, j));
                }
            }
        }
        best
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
        for (j, p) in self.handles(ed, rect) {
            let selected = ed.parts.contains(&j);
            let outline = Stroke::new(1.5, Color32::from_gray(230));
            if is_hip(j) {
                painter.circle(p, FOOT_TARGET_SIZE, if selected { Color32::from_rgb(255, 160, 50) } else { Color32::TRANSPARENT }, outline);
                continue;
            }
            let square = Rect::from_center_size(p, Vec2::splat(FOOT_TARGET_SIZE * 2.0));
            if selected {
                painter.rect_filled(square, 1.0, Color32::from_rgb(255, 160, 50));
            }
            painter.rect_stroke(square, 1.0, outline, egui::StrokeKind::Middle);
        }
        let text = if self.local { "Local  (L)" } else { "Global  (L)" };
        painter.text(rect.left_top() + Vec2::new(10.0, 8.0), egui::Align2::LEFT_TOP, text, egui::FontId::proportional(13.0), Color32::from_gray(170));
        if ed.anim.partner.is_some() {
            let who = format!("Editing Rig {}  (Tab or click to switch)", if ed.anim.second { 2 } else { 1 });
            painter.text(rect.center_top() + Vec2::new(0.0, 8.0), egui::Align2::CENTER_TOP, who, egui::FontId::proportional(13.0), Color32::from_rgb(255, 160, 50));
        }
        let frame = format!("Frame {}", ed.current());
        painter.text(rect.right_top() + Vec2::new(-10.0, 8.0), egui::Align2::RIGHT_TOP, frame, egui::FontId::proportional(13.0), Color32::from_gray(170));
        crate::hints::paint(painter, rect);
    }
}
