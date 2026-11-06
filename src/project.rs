use crate::anim::Animation;
use crate::curve::Handle;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub struct Session {
    pub path: Option<PathBuf>,
    pub anim: Animation,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub new_handle: Handle,
    #[serde(default)]
    pub key_names: bool,
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("WANIM_DATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("welffi animate")))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&base);
    base
}

fn autosave_path() -> PathBuf {
    data_dir().join("autosave.json")
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn load_session() -> Session {
    let mut session: Session = std::fs::read_to_string(autosave_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    session.anim.recalc();
    session
}

pub fn save_session(session: &Session) -> std::io::Result<()> {
    write_atomic(&autosave_path(), &serde_json::to_string(session)?)
}

pub fn load_settings() -> Settings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    write_atomic(&settings_path(), &serde_json::to_string_pretty(settings)?)
}

pub fn save_animation(path: &Path, anim: &Animation) -> std::io::Result<()> {
    write_atomic(path, &serde_json::to_string_pretty(anim)?)
}

pub fn load_animation(path: &Path) -> std::io::Result<Animation> {
    let text = std::fs::read_to_string(path)?;
    let mut anim: Animation = serde_json::from_str(&text).map_err(std::io::Error::other)?;
    anim.recalc();
    Ok(anim)
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Handle;
    use crate::rig::Pose;

    #[test]
    fn animation_survives_save_and_load() {
        let mut anim = Animation::default();
        anim.tracks[3].set(12, Pose::IDENTITY, Handle::AutoClamped);
        let path = std::env::temp_dir().join("welffi_animate_test.wanim");
        save_animation(&path, &anim).unwrap();
        assert_eq!(load_animation(&path).unwrap(), anim);
        let _ = std::fs::remove_file(path);
    }
}
