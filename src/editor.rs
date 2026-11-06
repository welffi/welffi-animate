use crate::anim::{Animation, KeyId};
use crate::curve::Handle;
use crate::rig::{Pose, Rig};
use std::collections::BTreeSet;

const UNDO_LIMIT: usize = 200;

pub struct Editor {
    pub rig: Rig,
    pub anim: Animation,
    pub frame: f32,
    pub playing: bool,
    pub auto_key: bool,
    pub new_handle: Handle,
    pub key_names: bool,
    pub parts: BTreeSet<usize>,
    pub active: Option<usize>,
    pub keys: BTreeSet<KeyId>,
    pub overrides: Vec<Option<Pose>>,
    pub status: String,
    pub changes: u64,
    copied: Vec<Option<Pose>>,
    undo: Vec<Animation>,
    redo: Vec<Animation>,
}

impl Editor {
    pub fn new(anim: Animation) -> Editor {
        Editor {
            rig: Rig::new(&anim.props),
            overrides: vec![None; anim.tracks.len()],
            anim,
            frame: 0.0,
            playing: false,
            auto_key: true,
            new_handle: Handle::AutoClamped,
            key_names: false,
            parts: BTreeSet::new(),
            active: None,
            keys: BTreeSet::new(),
            status: String::new(),
            changes: 0,
            copied: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn current(&self) -> i32 {
        self.frame.round() as i32
    }

    pub fn poses(&self) -> Vec<Pose> {
        let mut poses = self.anim.poses(self.frame);
        for (pose, over) in poses.iter_mut().zip(&self.overrides) {
            if let Some(over) = *over {
                *pose = over;
            }
        }
        poses
    }

    pub fn set_frame(&mut self, frame: f32) {
        if frame != self.frame {
            self.frame = frame;
            self.clear_overrides();
        }
    }

    fn clear_overrides(&mut self) {
        self.overrides.iter_mut().for_each(|o| *o = None);
    }

    pub fn props_changed(&mut self) {
        self.rig = Rig::new(&self.anim.props);
        let count = self.anim.tracks.len();
        self.overrides = vec![None; count];
        self.copied.resize(count, None);
        self.parts.retain(|&j| j < count);
        self.active = self.active.filter(|&j| j < count);
        self.keys.retain(|&(j, _)| j < count);
    }

    pub fn checkpoint(&mut self) {
        self.undo.push(self.anim.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.changes += 1;
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.anim, previous));
            self.after_history("Undo");
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.anim, next));
            self.after_history("Redo");
        }
    }

    fn after_history(&mut self, what: &str) {
        self.props_changed();
        let all = self.anim.all_keys();
        self.keys.retain(|k| all.contains(k));
        self.changes += 1;
        self.status = what.to_string();
    }

    pub fn replace(&mut self, anim: Animation) {
        self.anim = anim;
        self.undo.clear();
        self.redo.clear();
        self.keys.clear();
        self.props_changed();
        self.frame = 0.0;
        self.playing = false;
        self.changes += 1;
    }

    pub fn finish_pose_edit(&mut self, joints: &[usize]) {
        if self.auto_key && !joints.is_empty() {
            self.key_joints(joints);
        }
    }

    pub fn insert_keys(&mut self) {
        let joints: Vec<usize> = if self.parts.is_empty() { (0..self.anim.tracks.len()).collect() } else { self.parts.iter().copied().collect() };
        self.key_joints(&joints);
        self.status = format!("Keyed {} part{} at frame {}", joints.len(), if joints.len() == 1 { "" } else { "s" }, self.current());
    }

    fn key_joints(&mut self, joints: &[usize]) {
        let poses = self.poses();
        let frame = self.current();
        self.checkpoint();
        for &j in joints {
            self.anim.tracks[j].set(frame, poses[j], self.new_handle);
            self.overrides[j] = None;
        }
        self.frame = frame as f32;
    }

    pub fn clear_pose(&mut self, rotation: bool) {
        let poses = self.poses();
        let joints: Vec<usize> = self.parts.iter().copied().collect();
        for &j in &joints {
            let mut pose = poses[j];
            if rotation {
                pose.rot = glam::Quat::IDENTITY;
            } else {
                pose.pos = glam::Vec3::ZERO;
            }
            self.overrides[j] = Some(pose);
        }
        self.finish_pose_edit(&joints);
    }

    pub fn copy_pose(&mut self) {
        let poses = self.poses();
        self.copied = vec![None; poses.len()];
        for &j in &self.parts {
            self.copied[j] = Some(poses[j]);
        }
        let count = self.parts.len();
        self.status = format!("Copied pose of {count} part{}", if count == 1 { "" } else { "s" });
    }

    pub fn paste_pose(&mut self, mirrored: bool) {
        let mut joints = Vec::new();
        for (j, pose) in self.copied.iter().enumerate() {
            let Some(pose) = *pose else { continue };
            let (target, pose) = if mirrored { (self.rig.joints[j].mirror, self.rig.mirrored(j, pose)) } else { (j, pose) };
            self.overrides[target] = Some(pose);
            joints.push(target);
        }
        if joints.is_empty() {
            self.status = "Nothing copied yet. Select parts and press Ctrl+C".to_string();
            return;
        }
        self.finish_pose_edit(&joints);
        self.status = if mirrored { "Pasted mirrored pose".to_string() } else { "Pasted pose".to_string() };
    }

    pub fn delete_keys(&mut self) {
        if self.keys.is_empty() {
            return;
        }
        self.checkpoint();
        self.anim.delete(&self.keys);
        self.status = format!("Deleted {} key{}", self.keys.len(), if self.keys.len() == 1 { "" } else { "s" });
        self.keys.clear();
    }

    pub fn jump_key(&mut self, forward: bool) {
        let frames = self.anim.key_frames();
        let now = self.current();
        let next = if forward { frames.range(now + 1..).next() } else { frames.range(..now).next_back() };
        if let Some(&frame) = next {
            self.set_frame(frame as f32);
        }
    }
}
