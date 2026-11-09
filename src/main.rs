#![windows_subsystem = "windows"]

mod anim;
mod body;
mod body_panel;
mod app;
mod camera;
mod clip;
mod clip_load;
mod clip_video;
mod key_clipboard;
mod curve;
mod easing;
mod editor;
mod export;
mod rigs;
mod rigs_menu;
mod graph;
mod gizmo;
mod hints;
mod ik;
mod menus;
mod move_panel;
mod panels;
mod partner;
mod player;
mod moves;
mod library;
mod library_view;
mod marker_lane;
mod markers;
mod live;
mod step_panel;
mod steps;
mod nametags;
mod project;
mod props;
mod props_panel;
mod reference_panel;
mod render;
mod rig;
mod timeline;
mod timing_bar;
mod transform;
mod viewport;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("welffi animate")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("welffi animate", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
