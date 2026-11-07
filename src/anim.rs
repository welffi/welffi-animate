use crate::curve::{self, Extend, Handle, Point, Side};
use crate::easing::{eased, Ease, Interp};
use crate::rigs::Stand;
use crate::markers::Marker;
use crate::props::Prop;
use crate::rig::{Pose, JOINT_COUNT, MESH_COUNT};
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CHANNELS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub frame: i32,
    pub pose: Pose,
    pub interp: Interp,
    pub ease: Ease,
    #[serde(default)]
    pub handle: Handle,
    #[serde(default)]
    pub left: [Vec2; CHANNELS],
    #[serde(default)]
    pub right: [Vec2; CHANNELS],
}

impl Key {
    pub fn point(&self, channel: usize) -> Point {
        Point { x: self.frame as f32, y: self.pose.channels()[channel], handle: self.handle, left: self.left[channel], right: self.right[channel] }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub keys: Vec<Key>,
    #[serde(default)]
    pub extend: Extend,
}

pub type KeyId = (usize, i32);
pub type PointId = (usize, usize, i32);

impl Track {
    pub fn key_at(&self, frame: i32) -> Option<&Key> {
        self.keys.iter().find(|k| k.frame == frame)
    }

    pub fn key_at_mut(&mut self, frame: i32) -> Option<&mut Key> {
        self.keys.iter_mut().find(|k| k.frame == frame)
    }

    fn span(&self, frame: f32) -> Option<(usize, f32)> {
        let keys = &self.keys;
        if keys.len() < 2 || frame <= keys[0].frame as f32 || frame >= keys[keys.len() - 1].frame as f32 {
            return None;
        }
        let i = keys.windows(2).position(|w| frame < w[1].frame as f32).unwrap_or(0);
        let (from, to) = (keys[i], keys[i + 1]);
        Some((i, (frame - from.frame as f32) / (to.frame - from.frame) as f32))
    }

    fn edge(&self, frame: f32) -> Option<&Key> {
        let first = self.keys.first()?;
        if frame <= first.frame as f32 {
            return Some(first);
        }
        self.keys.last().filter(|_| self.span(frame).is_none())
    }

    fn looped(&self, frame: f32) -> f32 {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else { return frame };
        let (start, end) = (first.frame as f32, last.frame as f32);
        if self.extend != Extend::Cyclic || start == end || (start..=end).contains(&frame) {
            return frame;
        }
        start + (frame - start).rem_euclid(end - start)
    }

    fn runs_on(&self, frame: f32) -> bool {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else { return false };
        self.extend == Extend::Linear && self.keys.len() > 1 && (frame < first.frame as f32 || frame > last.frame as f32)
    }

    fn end_slope(&self, channel: usize, front: bool) -> f32 {
        let n = self.keys.len();
        let (from, to) = if front { (&self.keys[0], &self.keys[1]) } else { (&self.keys[n - 2], &self.keys[n - 1]) };
        match from.interp {
            Interp::Constant => 0.0,
            Interp::Bezier if front => curve::slope(from.left[channel]) as f32,
            Interp::Bezier => curve::slope(to.right[channel]) as f32,
            _ => (to.pose.channels()[channel] - from.pose.channels()[channel]) / (to.frame - from.frame) as f32,
        }
    }

    pub fn sample(&self, frame: f32) -> Pose {
        let frame = self.looped(frame);
        if self.runs_on(frame) {
            return Pose::from_channels(std::array::from_fn(|c| self.channel(c, frame)));
        }
        let Some((i, t)) = self.span(frame) else {
            return self.edge(frame).map_or(Pose::IDENTITY, |k| k.pose.normalized());
        };
        let (from, to) = (self.keys[i], self.keys[i + 1]);
        match from.interp {
            Interp::Constant => from.pose.normalized(),
            Interp::Bezier => Pose::from_channels(std::array::from_fn(|c| curve::segment(&from.point(c), &to.point(c), frame))),
            style => from.pose.normalized().lerp(to.pose.normalized(), eased(style, from.ease, t)),
        }
    }

    pub fn channel(&self, channel: usize, frame: f32) -> f32 {
        let frame = self.looped(frame);
        if self.runs_on(frame) {
            let front = frame < self.keys[0].frame as f32;
            let end = if front { &self.keys[0] } else { &self.keys[self.keys.len() - 1] };
            return end.pose.channels()[channel] + self.end_slope(channel, front) * (frame - end.frame as f32);
        }
        let Some((i, t)) = self.span(frame) else {
            return self.edge(frame).map_or(Pose::IDENTITY.channels()[channel], |k| k.pose.channels()[channel]);
        };
        let (from, to) = (self.keys[i].point(channel), self.keys[i + 1].point(channel));
        match self.keys[i].interp {
            Interp::Constant => from.y,
            Interp::Bezier => curve::segment(&from, &to, frame),
            style => from.y + (to.y - from.y) * eased(style, self.keys[i].ease, t),
        }
    }

    pub fn recalc(&mut self) {
        for channel in 0..CHANNELS {
            let mut points: Vec<Point> = self.keys.iter().map(|k| k.point(channel)).collect();
            curve::recalc_extended(&mut points, self.extend);
            for (key, point) in self.keys.iter_mut().zip(points) {
                key.left[channel] = point.left;
                key.right[channel] = point.right;
            }
        }
    }

    pub fn set(&mut self, frame: i32, pose: Pose, handle: Handle) {
        let pose = pose.aligned_to(self.sample(frame as f32).rot);
        match self.key_at_mut(frame) {
            Some(key) => key.pose = pose,
            None => {
                self.keys.push(Key { frame, pose, interp: Interp::Bezier, ease: Ease::Out, handle, left: Default::default(), right: Default::default() });
                self.keys.sort_by_key(|k| k.frame);
            }
        }
        self.recalc();
    }

    pub fn drag_handle(&mut self, channel: usize, frame: i32, side: Side, offset: Vec2) {
        let Some(key) = self.key_at_mut(frame) else { return };
        let mut point = key.point(channel);
        point.drag_handle(side, offset);
        key.handle = point.handle;
        key.left[channel] = point.left;
        key.right[channel] = point.right;
        self.recalc();
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub fps: u32,
    pub end: i32,
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub props: Vec<Prop>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub partner: Option<Box<Animation>>,
    #[serde(default)]
    pub at: Stand,
    #[serde(default)]
    pub second: bool,
}

impl Default for Animation {
    fn default() -> Self {
        Animation { fps: 60, end: 60, tracks: vec![Track::default(); JOINT_COUNT], props: Vec::new(), markers: Vec::new(), partner: None, at: Stand::default(), second: false }
    }
}

impl Animation {
    pub fn fresh() -> Animation {
        let mut anim = Animation::default();
        for track in &mut anim.tracks {
            track.set(0, Pose::IDENTITY, Handle::AutoClamped);
        }
        anim
    }

    pub fn poses(&self, frame: f32) -> Vec<Pose> {
        self.tracks.iter().map(|t| t.sample(frame)).collect()
    }

    pub fn add_prop(&mut self, prop: Prop) -> usize {
        self.props.push(prop);
        self.tracks.push(Track::default());
        self.tracks.len() - 1
    }

    pub fn remove_prop(&mut self, k: usize) {
        let joint = JOINT_COUNT + k;
        let removed = self.props.remove(k);
        self.tracks.remove(joint);
        for prop in &mut self.props {
            if prop.parent == joint {
                prop.parent = removed.parent;
                prop.rest = Pose::from_matrix(removed.rest.matrix() * prop.rest.matrix());
            }
            if prop.parent > joint {
                prop.parent -= 1;
            }
        }
    }

    pub fn bake_rest(&mut self, k: usize, pose: Pose) {
        let prop = &mut self.props[k];
        prop.rest = Pose::from_matrix(prop.rest.matrix() * pose.matrix());
        let inverse = pose.matrix().inverse();
        let track = &mut self.tracks[JOINT_COUNT + k];
        for key in &mut track.keys {
            key.pose = Pose::from_matrix(inverse * key.pose.matrix());
        }
        track.recalc();
    }

    pub fn key_frames(&self) -> BTreeSet<i32> {
        self.tracks.iter().flat_map(|t| t.keys.iter().map(|k| k.frame)).collect()
    }

    pub fn all_keys(&self) -> BTreeSet<KeyId> {
        self.tracks.iter().enumerate().flat_map(|(i, t)| t.keys.iter().map(move |k| (i, k.frame))).collect()
    }

    pub fn delete(&mut self, keys: &BTreeSet<KeyId>) {
        for (i, track) in self.tracks.iter_mut().enumerate() {
            track.keys.retain(|k| !keys.contains(&(i, k.frame)));
            track.recalc();
        }
    }

    pub fn move_keys(&mut self, keys: &BTreeSet<KeyId>, delta: i32) -> BTreeSet<KeyId> {
        let mut moved = BTreeSet::new();
        for (i, track) in self.tracks.iter_mut().enumerate() {
            let (mut picked, rest): (Vec<Key>, Vec<Key>) = track.keys.drain(..).partition(|k| keys.contains(&(i, k.frame)));
            for key in &mut picked {
                key.frame += delta;
                moved.insert((i, key.frame));
            }
            let taken: BTreeSet<i32> = picked.iter().map(|k| k.frame).collect();
            track.keys = rest.into_iter().filter(|k| !taken.contains(&k.frame)).chain(picked).collect();
            track.keys.sort_by_key(|k| k.frame);
            track.recalc();
        }
        moved
    }

    pub fn nudge(&mut self, points: &BTreeSet<PointId>, frames: i32, value: f32) -> BTreeSet<PointId> {
        for &(track, channel, frame) in points {
            if let Some(key) = self.tracks[track].key_at_mut(frame) {
                let mut channels = key.pose.channels();
                channels[channel] += value;
                key.pose = Pose::raw(channels);
            }
        }
        let keys: BTreeSet<KeyId> = points.iter().map(|&(track, _, frame)| (track, frame)).collect();
        let moved = self.move_keys(&keys, frames);
        points.iter().map(|&(track, channel, frame)| (track, channel, frame + frames)).filter(|&(track, _, frame)| moved.contains(&(track, frame))).collect()
    }

    pub fn set_interp(&mut self, keys: &BTreeSet<KeyId>, interp: Interp) {
        self.each_key(keys, |k| k.interp = interp);
    }

    pub fn set_ease(&mut self, keys: &BTreeSet<KeyId>, ease: Ease) {
        self.each_key(keys, |k| k.ease = ease);
    }

    pub fn set_handle(&mut self, keys: &BTreeSet<KeyId>, handle: Handle) {
        self.each_key(keys, |k| k.handle = handle);
        self.recalc();
    }

    pub fn set_extend(&mut self, keys: &BTreeSet<KeyId>, extend: Extend) -> usize {
        let mut changed = 0;
        for (i, track) in self.tracks.iter_mut().enumerate() {
            if keys.iter().any(|&(joint, _)| joint == i) {
                track.extend = extend;
                track.recalc();
                changed += 1;
            }
        }
        changed
    }

    fn add_hip_tracks(&mut self) {
        if self.tracks.len() != MESH_COUNT + self.props.len() {
            return;
        }
        let hips = JOINT_COUNT - MESH_COUNT;
        self.tracks.splice(MESH_COUNT..MESH_COUNT, vec![Track::default(); hips]);
        for prop in self.props.iter_mut().filter(|p| p.parent >= MESH_COUNT) {
            prop.parent += hips;
        }
    }

    pub fn recalc(&mut self) {
        self.add_hip_tracks();
        self.tracks.resize(JOINT_COUNT + self.props.len(), Track::default());
        self.tracks.iter_mut().for_each(Track::recalc);
        self.sync_partner();
    }

    fn each_key(&mut self, keys: &BTreeSet<KeyId>, mut f: impl FnMut(&mut Key)) {
        for (i, track) in self.tracks.iter_mut().enumerate() {
            track.keys.iter_mut().filter(|k| keys.contains(&(i, k.frame))).for_each(&mut f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn pose_x(x: f32) -> Pose {
        Pose { pos: Vec3::new(x, 0.0, 0.0), rot: Quat::IDENTITY }
    }

    #[test]
    fn two_bezier_keys_ease_in_and_out() {
        let mut track = Track::default();
        track.set(0, pose_x(0.0), Handle::AutoClamped);
        track.set(10, pose_x(1.0), Handle::AutoClamped);
        assert!((track.sample(5.0).pos.x - 0.5).abs() < 1e-5);
        assert!(track.sample(1.0).pos.x < 0.1);
    }

    #[test]
    fn constant_holds_until_next_key() {
        let mut track = Track::default();
        track.set(0, pose_x(0.0), Handle::AutoClamped);
        track.set(10, pose_x(1.0), Handle::AutoClamped);
        track.keys[0].interp = Interp::Constant;
        assert_eq!(track.sample(9.9).pos.x, 0.0);
        assert_eq!(track.sample(10.0).pos.x, 1.0);
    }

    #[test]
    fn new_key_keeps_quaternion_on_same_side() {
        let mut track = Track::default();
        track.set(0, Pose { pos: Vec3::ZERO, rot: Quat::from_rotation_y(0.2) }, Handle::AutoClamped);
        track.set(10, Pose { pos: Vec3::ZERO, rot: -Quat::from_rotation_y(0.4) }, Handle::AutoClamped);
        assert!(track.keys[1].pose.rot.w > 0.0);
    }

    #[test]
    fn moving_keys_onto_others_replaces_them() {
        let mut anim = Animation::default();
        anim.tracks[0].set(0, pose_x(0.0), Handle::AutoClamped);
        anim.tracks[0].set(5, pose_x(5.0), Handle::AutoClamped);
        let moved = anim.move_keys(&BTreeSet::from([(0, 0)]), 5);
        assert_eq!(moved, BTreeSet::from([(0, 5)]));
        assert_eq!(anim.tracks[0].keys.len(), 1);
        assert_eq!(anim.tracks[0].keys[0].pose.pos.x, 0.0);
    }

    #[test]
    fn nudge_changes_one_channel_and_moves_the_whole_key() {
        let mut anim = Animation::default();
        anim.tracks[2].set(0, pose_x(0.0), Handle::AutoClamped);
        anim.tracks[2].set(10, pose_x(1.0), Handle::AutoClamped);
        let moved = anim.nudge(&BTreeSet::from([(2, 1, 10)]), 3, 0.5);
        assert_eq!(moved, BTreeSet::from([(2, 1, 13)]));
        let key = anim.tracks[2].key_at(13).unwrap();
        assert_eq!((key.pose.pos.x, key.pose.pos.y), (1.0, 0.5));
    }

    fn looping_track(extend: Extend) -> Track {
        let mut track = Track::default();
        track.set(0, pose_x(0.0), Handle::AutoClamped);
        track.set(10, pose_x(4.0), Handle::AutoClamped);
        track.set(20, pose_x(0.0), Handle::AutoClamped);
        track.extend = extend;
        track.recalc();
        track
    }

    #[test]
    fn cyclic_repeats_the_motion_both_ways() {
        let track = looping_track(Extend::Cyclic);
        for frame in [3.0, 12.5, 17.0] {
            let inside = track.sample(frame).pos.x;
            assert!((track.sample(frame + 20.0).pos.x - inside).abs() < 1e-4);
            assert!((track.sample(frame - 40.0).pos.x - inside).abs() < 1e-4);
            assert!((track.channel(0, frame + 60.0) - inside).abs() < 1e-4);
        }
    }

    #[test]
    fn constant_holds_past_the_ends_and_linear_runs_on() {
        let constant = looping_track(Extend::Constant);
        assert_eq!(constant.sample(35.0).pos.x, 0.0);
        let mut line = Track::default();
        line.set(0, pose_x(0.0), Handle::Auto);
        line.set(10, pose_x(5.0), Handle::Auto);
        line.extend = Extend::Linear;
        line.recalc();
        assert!((line.sample(20.0).pos.x - 10.0).abs() < 0.05);
        assert!((line.sample(-10.0).pos.x + 5.0).abs() < 0.05);
    }

    #[test]
    fn extend_is_set_on_the_tracks_of_the_chosen_keys() {
        let mut anim = Animation::default();
        anim.tracks[1].set(0, pose_x(0.0), Handle::AutoClamped);
        anim.tracks[1].set(10, pose_x(1.0), Handle::AutoClamped);
        anim.tracks[2].set(0, pose_x(0.0), Handle::AutoClamped);
        assert_eq!(anim.set_extend(&BTreeSet::from([(1, 10)]), Extend::Cyclic), 1);
        assert_eq!(anim.tracks[1].extend, Extend::Cyclic);
        assert_eq!(anim.tracks[2].extend, Extend::Constant);
    }

    #[test]
    fn a_fresh_animation_keys_every_part_at_zero() {
        let anim = Animation::fresh();
        assert!(anim.tracks.iter().all(|t| t.keys.len() == 1 && t.key_at(0).is_some()));
    }

    fn block(name: &str) -> Prop {
        Prop::new(name.into(), crate::props::Shape::Block(Vec3::ONE))
    }

    fn near(a: glam::Mat4, b: glam::Mat4) -> bool {
        a.abs_diff_eq(b, 1e-4)
    }

    #[test]
    fn placing_a_prop_keeps_how_its_keys_look() {
        let mut anim = Animation::default();
        let sword = anim.add_prop(block("Sword"));
        let turned = Pose { pos: Vec3::new(0.0, 0.5, 0.0), rot: Quat::from_rotation_x(0.7) };
        anim.tracks[sword].set(0, turned, Handle::AutoClamped);
        anim.tracks[sword].set(10, pose_x(1.0), Handle::AutoClamped);
        let rest = anim.props[0].rest.matrix();
        let before = [0.0, 10.0].map(|f| rest * anim.poses(f)[sword].matrix());
        anim.bake_rest(0, turned);
        let rest = anim.props[0].rest.matrix();
        let after = [0.0, 10.0].map(|f| rest * anim.poses(f)[sword].matrix());
        assert!(near(before[0], after[0]) && near(before[1], after[1]));
        assert!(near(anim.poses(0.0)[sword].matrix(), glam::Mat4::IDENTITY));
    }

    #[test]
    fn removing_a_prop_hands_its_children_to_its_parent() {
        let mut anim = Animation::default();
        anim.add_prop(block("Hilt"));
        anim.add_prop(block("Sword"));
        let blade = anim.add_prop(block("Blade"));
        anim.props[2].parent = JOINT_COUNT + 1;
        anim.props[2].rest = pose_x(2.0);
        anim.tracks[blade].set(3, Pose::IDENTITY, Handle::AutoClamped);
        let world = anim.props[1].rest.matrix() * anim.props[2].rest.matrix();
        anim.remove_prop(1);
        assert_eq!(anim.tracks.len(), JOINT_COUNT + 2);
        assert_eq!(anim.props[1].parent, crate::rig::RIGHT_ARM);
        assert!(near(anim.props[1].rest.matrix(), world));
        assert!(anim.tracks[JOINT_COUNT + 1].key_at(3).is_some());
    }

    #[test]
    fn old_files_get_hip_tracks_and_props_keep_their_parents() {
        let mut anim = Animation::default();
        anim.add_prop(block("Hilt"));
        anim.add_prop(block("Blade"));
        anim.tracks[JOINT_COUNT].set(5, Pose::IDENTITY, Handle::AutoClamped);
        anim.tracks.drain(MESH_COUNT..JOINT_COUNT);
        anim.props[1].parent = MESH_COUNT;
        anim.recalc();
        assert_eq!(anim.tracks.len(), JOINT_COUNT + 2);
        assert!(anim.tracks[JOINT_COUNT].key_at(5).is_some());
        assert_eq!(anim.props[1].parent, JOINT_COUNT);
        assert_eq!(anim.props[0].parent, crate::rig::RIGHT_ARM);
    }

    #[test]
    fn samples_stay_normalized_when_a_rotation_channel_is_dragged() {
        let mut track = Track::default();
        track.set(0, pose_x(0.0), Handle::AutoClamped);
        track.keys[0].pose.rot.w = 3.0;
        assert!((track.sample(0.0).rot.length() - 1.0).abs() < 1e-5);
    }
}
