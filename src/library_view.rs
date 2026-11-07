use crate::camera::Camera;
use crate::editor::Editor;
use crate::library::{self, Saved};
use crate::render::{load_parts, Draw, Scene3d, Surface};
use crate::rig::{Rig, JOINT_COUNT};
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Vec2};
use eframe::egui_wgpu::RenderState;
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI};
use std::path::PathBuf;

const CARD: Vec2 = Vec2::new(210.0, 252.0);
const PREVIEW: Vec2 = Vec2::new(198.0, 190.0);
const CARD_FILL: Color32 = Color32::from_rgb(34, 34, 38);
const BORDER: Color32 = Color32::from_rgb(62, 62, 70);
const ACCENT: Color32 = Color32::from_rgb(255, 160, 50);

struct Card {
    saved: Saved,
    rig: Rig,
    partner_rig: Option<Rig>,
    surface: Surface,
}

impl Card {
    fn new(saved: Saved) -> Card {
        let partner_rig = saved.anim.partner.as_ref().map(|p| Rig::new(&p.props));
        Card { rig: Rig::new(&saved.anim.props), partner_rig, saved, surface: Surface::default() }
    }

    fn details(&self) -> String {
        let anim = &self.saved.anim;
        let keys = anim.all_keys().len() + anim.partner.as_ref().map_or(0, |p| p.all_keys().len());
        let rigs = if anim.partner.is_some() { "  ·  2 rigs" } else { "" };
        format!("{:.1}s  ·  {keys} keys{rigs}", anim.end as f32 / anim.fps.max(1) as f32)
    }
}

pub struct LibraryView {
    scene: Scene3d,
    camera: Camera,
    pair_camera: Camera,
    cards: Vec<Card>,
    name: String,
    renaming: Option<(usize, String)>,
    deleting: Option<usize>,
}

fn loop_frame(seconds: f64, fps: u32, end: i32) -> f32 {
    (seconds * fps as f64).rem_euclid(end.max(1) as f64) as f32
}

impl LibraryView {
    pub fn new(rs: &RenderState) -> LibraryView {
        let meshes = load_parts(include_bytes!("../assets/r6.bin"));
        let mut scene = Scene3d::new(rs, &meshes, include_bytes!("../assets/r6.png"));
        scene.set_actors(rs, &[Vec::new(), Vec::new()]);
        LibraryView {
            scene,
            camera: Camera { target: Vec3::new(0.0, 2.5, 0.0), yaw: PI + 0.5, pitch: 0.2, distance: 12.5 },
            pair_camera: Camera { target: Vec3::new(0.0, 2.5, -2.0), yaw: FRAC_PI_2 + 0.3, pitch: 0.2, distance: 15.0 },
            cards: Vec::new(),
            name: String::new(),
            renaming: None,
            deleting: None,
        }
    }

    pub fn open(&mut self, rs: &RenderState) {
        self.cards.iter_mut().for_each(|card| card.surface.free(rs));
        self.cards = library::list(&library::folder()).into_iter().map(Card::new).collect();
        self.renaming = None;
        self.deleting = None;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, rs: &RenderState, ed: &mut Editor) -> Option<PathBuf> {
        self.header(ui, ed);
        ui.separator();
        let mut load = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if self.cards.is_empty() {
                ui.label("Nothing here yet. Name the current animation above and press Save to library.");
            }
            ui.horizontal_wrapped(|ui| {
                for i in 0..self.cards.len() {
                    load = self.card(ui, rs, ed, i).or(load.take());
                }
            });
        });
        self.confirm_delete(ui.ctx(), rs, ed);
        if !self.cards.is_empty() {
            ui.ctx().request_repaint();
        }
        load
    }

    fn header(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        ui.horizontal(|ui| {
            ui.heading("Library");
            ui.add_space(12.0);
            let field = ui.add(TextEdit::singleline(&mut self.name).hint_text("Name for the current animation").desired_width(240.0));
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.button("Save to library").clicked() || enter {
                self.save(ed);
            }
        });
    }

    fn save(&mut self, ed: &mut Editor) {
        match library::save(&library::folder(), &self.name, &ed.anim) {
            Ok(saved) => {
                ed.status = format!("Saved \"{}\" to the library", saved.name);
                self.cards.insert(0, Card::new(saved));
                self.renaming = None;
                self.deleting = None;
                self.name.clear();
            }
            Err(error) => ed.status = format!("Couldn't save to the library: {error}"),
        }
    }

    fn card(&mut self, ui: &mut egui::Ui, rs: &RenderState, ed: &mut Editor, i: usize) -> Option<PathBuf> {
        let (rect, response) = ui.allocate_exact_size(CARD, Sense::click());
        let painter = ui.painter_at(rect);
        let border = if response.hovered() { ACCENT } else { BORDER };
        painter.rect(rect, 8.0, CARD_FILL, Stroke::new(1.0, border), StrokeKind::Inside);
        let preview = Rect::from_min_size(rect.min + Vec2::splat(6.0), PREVIEW);
        self.preview(ui, rs, i, preview);
        let title = Rect::from_min_size(rect.min + Vec2::new(10.0, PREVIEW.y + 14.0), Vec2::new(CARD.x - 20.0, 22.0));
        let renaming = self.renaming.as_ref().is_some_and(|(at, _)| *at == i);
        if renaming {
            self.rename_field(ui, ed, i, title);
        } else {
            painter.text(title.left_center(), Align2::LEFT_CENTER, &self.cards[i].saved.name, FontId::proportional(15.0), Color32::WHITE);
        }
        let details = self.cards[i].details();
        painter.text(title.left_bottom() + Vec2::new(0.0, 14.0), Align2::LEFT_CENTER, details, FontId::proportional(12.0), Color32::from_gray(150));
        let mut load = (response.clicked() && !renaming).then(|| self.cards[i].saved.path.clone());
        response.on_hover_text("Click to load. Right-click to rename or delete.").context_menu(|ui| {
            if ui.button("Load").clicked() {
                load = Some(self.cards[i].saved.path.clone());
                ui.close();
            }
            if ui.button("Rename").clicked() {
                self.renaming = Some((i, self.cards[i].saved.name.clone()));
                ui.close();
            }
            if ui.button("Delete").clicked() {
                self.deleting = Some(i);
                ui.close();
            }
        });
        load
    }

    fn preview(&mut self, ui: &egui::Ui, rs: &RenderState, i: usize, rect: Rect) {
        let card = &mut self.cards[i];
        if !ui.is_rect_visible(rect) {
            card.surface.free(rs);
            return;
        }
        let anim = &card.saved.anim;
        let frame = loop_frame(ui.input(|input| input.time), anim.fps, anim.end);
        let draw = |world| Draw { world, tint: [0.0; 4], color: [1.0; 4] };
        let mut draws: Vec<Draw> = card.rig.worlds(&anim.poses(frame)).into_iter().take(JOINT_COUNT).map(draw).collect();
        if let Some((partner, rig)) = anim.partner.as_deref().zip(card.partner_rig.as_ref()) {
            let stand = anim.at.matrix();
            draws.extend(rig.worlds(&partner.poses(frame)).into_iter().take(JOINT_COUNT).map(|world| draw(stand * world)));
        }
        let camera = if anim.partner.is_some() { &self.pair_camera } else { &self.camera };
        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        let texture = self.scene.render_to(rs, &mut card.surface, size, camera.view_proj(rect), &draws);
        ui.painter().image(texture, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }

    fn rename_field(&mut self, ui: &mut egui::Ui, ed: &mut Editor, i: usize, rect: Rect) {
        let Some((_, text)) = self.renaming.as_mut() else { return };
        let field = ui.put(rect, TextEdit::singleline(text));
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
            return;
        }
        if !field.lost_focus() {
            return;
        }
        let name = std::mem::take(text);
        self.renaming = None;
        if !ui.input(|input| input.key_pressed(Key::Escape)) {
            self.finish_rename(ed, i, &name);
        }
    }

    fn finish_rename(&mut self, ed: &mut Editor, i: usize, name: &str) {
        let card = &mut self.cards[i];
        match library::rename(&card.saved.path, name) {
            Ok(path) => {
                card.saved.name = library::stem(&path);
                card.saved.path = path;
                ed.status = format!("Renamed to \"{}\"", card.saved.name);
            }
            Err(error) => ed.status = format!("Couldn't rename: {error}"),
        }
    }

    fn confirm_delete(&mut self, ctx: &egui::Context, rs: &RenderState, ed: &mut Editor) {
        let Some(i) = self.deleting.filter(|&i| i < self.cards.len()) else { return };
        let name = self.cards[i].saved.name.clone();
        let mut choice = None;
        egui::Window::new("Delete from library").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            ui.label(format!("Delete \"{name}\"? This can't be undone."));
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        let Some(confirmed) = choice else { return };
        self.deleting = None;
        if !confirmed {
            return;
        }
        match std::fs::remove_file(&self.cards[i].saved.path) {
            Ok(()) => {
                self.cards[i].surface.free(rs);
                self.cards.remove(i);
                ed.status = format!("Deleted \"{name}\" from the library");
            }
            Err(error) => ed.status = format!("Couldn't delete \"{name}\": {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_loop_over_the_animation_length() {
        assert_eq!(loop_frame(0.0, 60, 60), 0.0);
        assert!((loop_frame(0.5, 60, 60) - 30.0).abs() < 1e-4);
        assert!((loop_frame(1.25, 60, 60) - 15.0).abs() < 1e-4);
        assert_eq!(loop_frame(3.0, 60, 0), 0.0);
    }
}
