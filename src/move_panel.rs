use crate::anim::KeyId;
use crate::editor::Editor;
use crate::live::{Choice, Live};
use crate::moves::{shape_moves, Outcome, Shape};
use eframe::egui::{self, Pos2, RichText};
use std::collections::BTreeSet;

const MOST_REASONS: usize = 5;

#[derive(Default)]
pub struct MovePanel {
    shape: Shape,
    open: Option<Open>,
}

struct Open {
    live: Live,
    keys: BTreeSet<KeyId>,
    outcome: Outcome,
}

impl MovePanel {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn open(&mut self, ed: &mut Editor, at: Pos2) {
        if ed.keys.is_empty() {
            ed.status = "Select the keys your moves land on, then press B".to_string();
            return;
        }
        self.open = Some(Open { live: Live::start(ed, at), keys: ed.keys.clone(), outcome: Outcome::default() });
        self.preview(ed);
    }

    fn preview(&mut self, ed: &mut Editor) {
        let Some(open) = &mut self.open else { return };
        ed.anim = open.live.base.clone();
        open.outcome = shape_moves(&mut ed.anim, &open.keys, self.shape, ed.new_handle);
    }

    pub fn ui(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        let Some(open) = &self.open else { return };
        if open.live.stale(ed) {
            self.open = None;
            return;
        }
        let mut changed = false;
        let shape = &mut self.shape;
        let choice = open.live.window(ctx, "Build-up and overshoot", |ui| {
            let percent = |v: f64, _| format!("{:.0}%", v * 100.0);
            changed |= ui.add(egui::Slider::new(&mut shape.build_up, 0.0..=0.6).text("Build-up").custom_formatter(percent)).on_hover_text("Pulls back the other way before the move. 0 turns it off.").changed();
            changed |= ui.add(egui::Slider::new(&mut shape.snap, 0.0..=1.0).text("Snap").custom_formatter(percent)).on_hover_text("Whips out of the build-up fast, then eases into the pose. 0 turns it off.").changed();
            changed |= ui.add(egui::Slider::new(&mut shape.overshoot, 0.0..=0.6).text("Overshoot").custom_formatter(percent)).on_hover_text("Carries past the pose, then settles back onto it. 0 turns it off.").changed();
            changed |= ui.add(egui::Slider::new(&mut shape.settle, 0.2..=1.5).text("Settle").custom_formatter(percent)).on_hover_text("How long the settle takes, compared to the move").changed();
            ui.separator();
            ui.label(format!("Adds {} build-up, {} snap and {} overshoot", open.outcome.build_ups, open.outcome.snaps, open.outcome.overshoots));
            for ((track, frame), reason) in open.outcome.skipped.iter().take(MOST_REASONS) {
                ui.label(RichText::new(format!("{} at {frame}: {reason}", ed.rig.joints[*track].part)).weak());
            }
            if open.outcome.skipped.len() > MOST_REASONS {
                ui.label(RichText::new(format!("and {} more left alone", open.outcome.skipped.len() - MOST_REASONS)).weak());
            }
        });
        if changed {
            self.preview(ed);
        }
        if choice != Choice::Keep {
            self.open = None;
            Live::end(ed, choice);
            ed.status = if choice == Choice::Apply { "Added build-up and overshoot" } else { "Left the moves as they were" }.to_string();
        }
    }
}
