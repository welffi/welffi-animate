use crate::body_panel::BodyPanel;
use crate::editor::Editor;
use crate::move_panel::MovePanel;
use crate::step_panel::StepPanel;
use eframe::egui;

#[derive(Clone, Copy)]
pub enum Tool {
    Moves,
    Steps,
    Body,
}

#[derive(Default)]
pub struct Panels {
    moves: MovePanel,
    steps: StepPanel,
    body: BodyPanel,
}

impl Panels {
    pub fn open(&mut self, tool: Tool, ctx: &egui::Context, ed: &mut Editor) {
        if self.moves.is_open() || self.steps.is_open() || self.body.is_open() {
            return;
        }
        let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(ctx.content_rect().center());
        match tool {
            Tool::Moves => self.moves.open(ed, at),
            Tool::Steps => self.steps.open(ed, at),
            Tool::Body => self.body.open(ed, at),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        self.moves.ui(ctx, ed);
        self.steps.ui(ctx, ed);
        self.body.ui(ctx, ed);
    }
}
