use crate::curve::{self, Handle, Point, Side};
use crate::easing::{eased, Ease, Interp};
use crate::rig::{Pose, JOINT_COUNT};
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

    pub fn sample(&self, frame: f32) -> Pose {
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
            curve::recalc(&mut points);
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
    pub tracks: [Track; JOINT_COUNT],
}

impl Default for Animation {
    fn default() -> Self {
        Animation { fps: 60, end: 60, tracks: Default::default() }
    }
}

impl Animation {
    pub fn poses(&self, frame: f32) -> [Pose; JOINT_COUNT] {
        std::array::from_fn(|i| self.tracks[i].sample(frame))
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

    pub fn recalc(&mut self) {
        self.tracks.iter_mut().for_each(Track::recalc);
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

    #[test]
    fn samples_stay_normalized_when_a_rotation_channel_is_dragged() {
        let mut track = Track::default();
        track.set(0, pose_x(0.0), Handle::AutoClamped);
        track.keys[0].pose.rot.w = 3.0;
        assert!((track.sample(0.0).rot.length() - 1.0).abs() < 1e-5);
    }
}
