use crate::editor::Editor;
use crate::props::part_color;
use crate::render::Draw;
use crate::rig::is_hip;
use crate::camera::{ray_box, Camera};
use eframe::egui::{Pos2, Rect};
use glam::{Mat4, Vec3};

const DIM: f32 = 0.6;
const HOVER_TINT: [f32; 4] = [1.0, 1.0, 1.0, 0.16];

fn worlds(ed: &Editor) -> Option<(Vec<Mat4>, &[crate::props::Prop])> {
    let (partner, rig) = ed.anim.partner.as_deref().zip(ed.partner_rig.as_ref())?;
    let stand = ed.anim.at.matrix();
    let worlds = rig.worlds(&partner.poses(ed.frame)).into_iter().map(|world| stand * world).collect();
    Some((worlds, &partner.props))
}

pub fn draws(ed: &Editor, hovered: bool) -> Vec<Draw> {
    let Some((worlds, props)) = worlds(ed) else { return Vec::new() };
    let tint = if hovered { HOVER_TINT } else { [0.0; 4] };
    worlds
        .into_iter()
        .enumerate()
        .map(|(j, world)| {
            let color = part_color(props, j);
            Draw { world, tint, color: [color[0] * DIM, color[1] * DIM, color[2] * DIM, color[3]] }
        })
        .collect()
}

pub fn nearest(camera: &Camera, rect: Rect, pos: Pos2, ed: &Editor, bounds: &[(Vec3, Vec3)]) -> Option<f32> {
    let (worlds, props) = worlds(ed)?;
    let (origin, dir) = camera.ray(rect, pos);
    let mut best: Option<f32> = None;
    for (j, (world, (min, max))) in worlds.iter().zip(bounds).enumerate() {
        if is_hip(j) || part_color(props, j)[3] == 0.0 {
            continue;
        }
        let inverse = world.inverse();
        if let Some(t) = ray_box(inverse.transform_point3(origin), inverse.transform_vector3(dir), *min, *max) {
            best = Some(best.map_or(t, |b| b.min(t)));
        }
    }
    best
}
