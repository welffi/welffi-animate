use crate::anim::{Animation, KeyId};
use crate::curve::Handle;
use crate::rig::Pose;
use glam::{Quat, Vec3};
use std::collections::BTreeSet;

const BUILD_UP_TIME: f32 = 0.14;
const SNAP_TIME: f32 = 0.44;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub build_up: f32,
    pub snap: f32,
    pub overshoot: f32,
    pub settle: f32,
}

impl Default for Shape {
    fn default() -> Self {
        Shape { build_up: 0.18, snap: 0.7, overshoot: 0.1, settle: 0.65 }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub build_ups: usize,
    pub snaps: usize,
    pub overshoots: usize,
    pub skipped: Vec<(KeyId, &'static str)>,
}

struct Move {
    from: Pose,
    to: Pose,
    axis: Vec3,
    angle: f32,
}

impl Move {
    fn new(from: Pose, to: Pose) -> Option<Move> {
        let turn = from.rot.inverse() * to.rot;
        let turn = if turn.w < 0.0 { -turn } else { turn };
        let (axis, angle) = turn.to_axis_angle();
        let still = (to.pos - from.pos).length() < 1e-4 && angle.abs() < 1e-4;
        (!still).then_some(Move { from, to, axis, angle })
    }

    fn along(&self, amount: f32) -> Pose {
        let pos = self.from.pos + (self.to.pos - self.from.pos) * amount;
        Pose { pos, rot: self.from.rot * Quat::from_axis_angle(self.axis, self.angle * amount) }
    }
}

fn share(frames: i32, part: f32) -> i32 {
    ((frames as f32 * part).round() as i32).max(1)
}

pub fn shape_moves(anim: &mut Animation, keys: &BTreeSet<KeyId>, shape: Shape, handle: Handle) -> Outcome {
    let mut outcome = Outcome::default();
    let mut added: Vec<(usize, i32, Pose)> = Vec::new();
    let mut moved: Vec<(usize, i32, Pose)> = Vec::new();
    for &(track, frame) in keys {
        let list = &anim.tracks[track].keys;
        let Some(i) = list.iter().position(|k| k.frame == frame) else { continue };
        let Some(before) = i.checked_sub(1).map(|b| list[b]) else {
            outcome.skipped.push(((track, frame), "first key, so no move lands on it"));
            continue;
        };
        let Some(motion) = Move::new(before.pose, list[i].pose) else {
            outcome.skipped.push(((track, frame), "same pose as the key before, so nothing moves"));
            continue;
        };
        let length = frame - before.frame;
        let (mut start_frame, mut start) = (before.frame, 0.0);
        if shape.build_up > 0.0 {
            if length < 3 {
                outcome.skipped.push(((track, frame), "move too short for a build-up"));
            } else {
                start_frame = before.frame + share(length, BUILD_UP_TIME);
                start = -shape.build_up;
                added.push((track, start_frame, motion.along(start)));
                outcome.build_ups += 1;
            }
        }
        let mut peak = 1.0;
        if shape.overshoot > 0.0 {
            let settle = share(length, shape.settle);
            let room = list.get(i + 1).map_or(settle, |next| settle.min((next.frame - frame) / 2));
            if room < 1 {
                outcome.skipped.push(((track, frame), "no room to settle before the next key"));
            } else {
                peak = 1.0 + shape.overshoot;
                moved.push((track, frame, motion.along(peak)));
                added.push((track, frame + room, motion.to));
                outcome.overshoots += 1;
            }
        }
        if shape.snap > 0.0 {
            let snap_frame = start_frame + ((frame - start_frame) as f32 * SNAP_TIME).round() as i32;
            if snap_frame <= start_frame || snap_frame >= frame {
                outcome.skipped.push(((track, frame), "move too short for a snap"));
            } else {
                let progress = SNAP_TIME + shape.snap * (1.0 - SNAP_TIME);
                added.push((track, snap_frame, motion.along(start + (peak - start) * progress)));
                outcome.snaps += 1;
            }
        }
    }
    for (track, frame, pose) in moved {
        if let Some(key) = anim.tracks[track].key_at_mut(frame) {
            key.pose = pose;
        }
    }
    for (track, frame, pose) in added {
        if anim.tracks[track].key_at(frame).is_none() {
            anim.tracks[track].set(frame, pose, handle);
        }
    }
    anim.recalc();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Pose {
        Pose { pos: Vec3::new(x, 0.0, 0.0), rot: Quat::IDENTITY }
    }

    fn arm_move(frames: &[(i32, f32)]) -> Animation {
        let mut anim = Animation::default();
        for &(frame, x) in frames {
            anim.tracks[2].set(frame, at(x), Handle::AutoClamped);
        }
        anim
    }

    fn xs(anim: &Animation) -> Vec<(i32, f32)> {
        anim.tracks[2].keys.iter().map(|k| (k.frame, (k.pose.pos.x * 1000.0).round() / 1000.0)).collect()
    }

    #[test]
    fn default_shape_matches_welffis_arm_raise() {
        let mut anim = arm_move(&[(0, 0.0), (29, 1.0)]);
        let outcome = shape_moves(&mut anim, &BTreeSet::from([(2, 29)]), Shape::default(), Handle::AutoClamped);
        assert_eq!((outcome.build_ups, outcome.snaps, outcome.overshoots), (1, 1, 1));
        assert_eq!(xs(&anim), vec![(0, 0.0), (4, -0.18), (15, 0.885), (29, 1.1), (48, 1.0)]);
    }

    #[test]
    fn settle_stays_before_the_next_key() {
        let mut anim = arm_move(&[(0, 0.0), (10, 1.0), (14, 0.5)]);
        let shape = Shape { build_up: 0.0, snap: 0.0, overshoot: 0.2, settle: 1.0 };
        shape_moves(&mut anim, &BTreeSet::from([(2, 10)]), shape, Handle::AutoClamped);
        assert_eq!(xs(&anim), vec![(0, 0.0), (10, 1.2), (12, 1.0), (14, 0.5)]);
    }

    #[test]
    fn says_why_a_key_was_left_alone() {
        let mut anim = arm_move(&[(0, 0.0), (10, 0.0)]);
        let outcome = shape_moves(&mut anim, &BTreeSet::from([(2, 0), (2, 10)]), Shape::default(), Handle::AutoClamped);
        assert_eq!(outcome.skipped.len(), 2);
        assert_eq!(anim.tracks[2].keys.len(), 2);
    }

    #[test]
    fn rotation_overshoots_past_the_turn() {
        let mut anim = Animation::default();
        anim.tracks[2].set(0, Pose::IDENTITY, Handle::AutoClamped);
        anim.tracks[2].set(10, Pose { pos: Vec3::ZERO, rot: Quat::from_rotation_z(1.0) }, Handle::AutoClamped);
        let shape = Shape { build_up: 0.1, snap: 0.0, overshoot: 0.2, settle: 0.3 };
        shape_moves(&mut anim, &BTreeSet::from([(2, 10)]), shape, Handle::AutoClamped);
        let angle = |frame| anim.tracks[2].key_at(frame).unwrap().pose.rot.to_axis_angle();
        assert!((angle(10).1 - 1.2).abs() < 1e-4);
        assert!((angle(1).1 - 0.1).abs() < 1e-4 && angle(1).0.z < 0.0);
        assert!((angle(13).1 - 1.0).abs() < 1e-4);
    }
}
