use crate::anim::Animation;
use crate::editor::Editor;
use eframe::egui::{self, Key, Pos2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Keep,
    Apply,
    Cancel,
}

pub struct Live {
    pub base: Animation,
    at: Pos2,
    changes: u64,
}

impl Live {
    pub fn start(ed: &mut Editor, at: Pos2) -> Live {
        ed.checkpoint();
        Live { base: ed.anim.clone(), at, changes: ed.changes }
    }

    pub fn stale(&self, ed: &Editor) -> bool {
        ed.changes != self.changes
    }

    pub fn window(&self, ctx: &egui::Context, title: &str, contents: impl FnOnce(&mut egui::Ui)) -> Choice {
        let mut choice = Choice::Keep;
        egui::Window::new(title).default_pos(self.at).collapsible(false).resizable(false).show(ctx, |ui| {
            contents(ui);
            ui.horizontal(|ui| {
                if ui.button("Apply  (Enter)").clicked() {
                    choice = Choice::Apply;
                }
                if ui.button("Cancel  (Esc)").clicked() {
                    choice = Choice::Cancel;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            choice = Choice::Cancel;
        } else if ctx.input(|i| i.key_pressed(Key::Enter)) {
            choice = Choice::Apply;
        }
        choice
    }

    pub fn end(ed: &mut Editor, choice: Choice) {
        match choice {
            Choice::Apply => ed.changes += 1,
            Choice::Cancel => ed.undo(),
            Choice::Keep => {}
        }
    }
}
