use crate::anim::KeyId;
use crate::body::{follow_body, Follow, Outcome};
use crate::editor::Editor;
use crate::live::{Choice, Live};
use eframe::egui::{self, Pos2, RichText};
use std::collections::BTreeSet;

const MOST_REASONS: usize = 5;

#[derive(Default)]
pub struct BodyPanel {
    follow: Follow,
    open: Option<Open>,
}

struct Open {
    live: Live,
    keys: BTreeSet<KeyId>,
    outcome: Outcome,
}

impl BodyPanel {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn open(&mut self, ed: &mut Editor, at: Pos2) {
        if ed.keys.is_empty() {
            ed.status = "Select the arm keys the body should follow, then press F".to_string();
            return;
        }
        self.open = Some(Open { live: Live::start(ed, at), keys: ed.keys.clone(), outcome: Outcome::default() });
        self.preview(ed);
    }

    fn preview(&mut self, ed: &mut Editor) {
        let Some(open) = &mut self.open else { return };
        ed.anim = open.live.base.clone();
        open.outcome = follow_body(&mut ed.anim, &ed.rig, &open.keys, self.follow, ed.new_handle);
    }

    pub fn ui(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        let Some(open) = &self.open else { return };
        if open.live.stale(ed) {
            self.open = None;
            return;
        }
        let mut changed = false;
        let follow = &mut self.follow;
        let choice = open.live.window(ctx, "Body follow", |ui| {
            let percent = |v: f64, _| format!("{:.0}%", v * 100.0);
            let frames = |v: f64, _| format!("{v} frames");
            changed |= ui.add(egui::Slider::new(&mut follow.torso, 0.0..=2.0).text("Torso").custom_formatter(percent)).on_hover_text("Twists and leans toward where the hand goes").changed();
            changed |= ui.add(egui::Slider::new(&mut follow.head, 0.0..=2.0).text("Head").custom_formatter(percent)).on_hover_text("Turns against the torso so the head stays steadier").changed();
            changed |= ui.add(egui::Slider::new(&mut follow.arm, 0.0..=2.0).text("Other arm").custom_formatter(percent)).on_hover_text("Swings the opposite way for balance").changed();
            changed |= ui.add(egui::Slider::new(&mut follow.lead, 0..=8).text("Torso leads").custom_formatter(frames)).on_hover_text("How early the torso starts before the arm").changed();
            changed |= ui.add(egui::Slider::new(&mut follow.drag, 0..=10).text("Head and arm drag").custom_formatter(frames)).on_hover_text("How late the head and the other arm settle").changed();
            ui.separator();
            let keyed: Vec<String> = open.outcome.keyed.iter().map(|&(track, count)| format!("{} {count}", ed.rig.joints[track].part)).collect();
            ui.label(if keyed.is_empty() { "Nothing follows yet".to_string() } else { format!("Keys: {}", keyed.join(", ")) });
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
            ed.status = if choice == Choice::Apply { "The body follows the arms now" } else { "Left the body as it was" }.to_string();
        }
    }
}
