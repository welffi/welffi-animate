use crate::editor::Editor;
use crate::live::{Choice, Live};
use crate::rig::JOINT_COUNT;
use crate::steps::step_animation;
use eframe::egui::{self, Pos2, RichText};
use std::collections::BTreeSet;

pub struct StepPanel {
    hold: i32,
    open: Option<Open>,
}

impl Default for StepPanel {
    fn default() -> Self {
        StepPanel { hold: 4, open: None }
    }
}

struct Open {
    live: Live,
    tracks: BTreeSet<usize>,
    keys: usize,
}

impl StepPanel {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn open(&mut self, ed: &mut Editor, at: Pos2) {
        let tracks = if ed.parts.is_empty() { (0..JOINT_COUNT).collect() } else { ed.parts.clone() };
        self.open = Some(Open { live: Live::start(ed, at), tracks, keys: 0 });
        self.preview(ed);
    }

    fn preview(&mut self, ed: &mut Editor) {
        let Some(open) = &mut self.open else { return };
        ed.anim = open.live.base.clone();
        open.keys = step_animation(&mut ed.anim, &open.tracks, self.hold, ed.new_handle);
        let all = ed.anim.all_keys();
        ed.keys.retain(|k| all.contains(k));
    }

    pub fn ui(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        let Some(open) = &self.open else { return };
        if open.live.stale(ed) {
            self.open = None;
            return;
        }
        let mut changed = false;
        let hold = &mut self.hold;
        let fps = ed.anim.fps;
        let parts = if open.tracks.len() == JOINT_COUNT { "All parts".to_string() } else { open.tracks.iter().map(|&t| ed.rig.joints[t].part).collect::<Vec<_>>().join(", ") };
        let choice = open.live.window(ctx, "Low FPS look", |ui| {
            changed |= ui
                .add(egui::Slider::new(hold, 2..=12).text("Hold").custom_formatter(|v, _| format!("{v} frames")))
                .on_hover_text("How long each pose holds. The poses follow your original motion.")
                .changed();
            ui.label(RichText::new(format!("Looks like {:.0} fps", fps as f32 / *hold as f32)).weak());
            ui.separator();
            ui.label(format!("{parts}: {} held poses", open.keys));
        });
        if changed {
            self.preview(ed);
        }
        if choice != Choice::Keep {
            self.open = None;
            Live::end(ed, choice);
            ed.status = if choice == Choice::Apply { "Made the low FPS look" } else { "Left the animation as it was" }.to_string();
        }
    }
}
