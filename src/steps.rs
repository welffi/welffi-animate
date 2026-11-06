use crate::anim::{Animation, Track};
use crate::curve::Handle;
use crate::easing::Interp;
use std::collections::BTreeSet;

pub fn step_frames(first: i32, last: i32, hold: i32) -> BTreeSet<i32> {
    (first..=last).filter(|f| f.rem_euclid(hold) == 0).chain([first, last]).collect()
}

pub fn stepped(track: &Track, hold: i32, handle: Handle) -> Track {
    let (Some(first), Some(last)) = (track.keys.first(), track.keys.last()) else { return track.clone() };
    if first.frame == last.frame {
        return track.clone();
    }
    let mut out = Track { extend: track.extend, ..Track::default() };
    for frame in step_frames(first.frame, last.frame, hold) {
        out.set(frame, track.sample(frame as f32), handle);
    }
    for key in &mut out.keys {
        key.interp = Interp::Constant;
    }
    out
}

pub fn step_animation(anim: &mut Animation, tracks: &BTreeSet<usize>, hold: i32, handle: Handle) -> usize {
    for &t in tracks {
        anim.tracks[t] = stepped(&anim.tracks[t], hold, handle);
    }
    tracks.iter().map(|&t| anim.tracks[t].keys.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Pose;
    use glam::{Quat, Vec3};

    fn at(x: f32) -> Pose {
        Pose { pos: Vec3::new(x, 0.0, 0.0), rot: Quat::IDENTITY }
    }

    #[test]
    fn frames_land_on_a_shared_grid_and_keep_the_ends() {
        assert_eq!(step_frames(0, 10, 4), BTreeSet::from([0, 4, 8, 10]));
        assert_eq!(step_frames(3, 12, 4), BTreeSet::from([3, 4, 8, 12]));
    }

    #[test]
    fn holds_each_pose_along_the_original_motion() {
        let mut anim = Animation::default();
        anim.tracks[2].set(0, at(0.0), Handle::AutoClamped);
        anim.tracks[2].set(10, at(1.0), Handle::AutoClamped);
        anim.tracks[2].keys[0].interp = Interp::Linear;
        anim.tracks[3].set(0, at(5.0), Handle::AutoClamped);
        let before = anim.tracks[3].clone();
        step_animation(&mut anim, &BTreeSet::from([2, 3]), 4, Handle::AutoClamped);
        let xs: Vec<(i32, f32)> = anim.tracks[2].keys.iter().map(|k| (k.frame, (k.pose.pos.x * 1000.0).round() / 1000.0)).collect();
        assert_eq!(xs, vec![(0, 0.0), (4, 0.4), (8, 0.8), (10, 1.0)]);
        assert!(anim.tracks[2].keys.iter().all(|k| k.interp == Interp::Constant));
        assert_eq!(anim.tracks[2].sample(7.0).pos.x, 0.4);
        assert_eq!(anim.tracks[3], before);
    }
}
