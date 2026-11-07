use crate::anim::Animation;
use serde::{Deserialize, Serialize};

pub const DEFAULT_NAME: &str = "Marker";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub frame: i32,
    pub name: String,
}

pub fn marker_name(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() { DEFAULT_NAME.to_string() } else { trimmed.to_string() }
}

impl Animation {
    pub fn add_marker(&mut self, frame: i32, name: &str) -> usize {
        self.markers.push(Marker { frame: frame.max(0), name: marker_name(name) });
        self.markers.len() - 1
    }

    pub fn marker_names_at(&self, frame: i32) -> Vec<&str> {
        self.markers.iter().filter(|m| m.frame == frame).map(|m| m.name.as_str()).collect()
    }

    pub fn last_marker(&self) -> Option<i32> {
        self.markers.iter().map(|m| m.frame).max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_never_empty() {
        assert_eq!(marker_name("  Hit "), "Hit");
        assert_eq!(marker_name("   "), DEFAULT_NAME);
    }

    #[test]
    fn markers_stay_on_or_after_frame_zero() {
        let mut anim = Animation::default();
        let at = anim.add_marker(-5, "Hit");
        assert_eq!(anim.markers[at].frame, 0);
        anim.add_marker(0, "Swing");
        assert_eq!(anim.marker_names_at(0), ["Hit", "Swing"]);
        assert_eq!(anim.last_marker(), Some(0));
    }
}
