use std::{fs, path::PathBuf};

use crate::state::TuiPrefs;

fn prefs_path() -> PathBuf {
    utils::assets::asset_dir().join("tui.json")
}

pub(crate) fn load_prefs() -> TuiPrefs {
    let path = prefs_path();
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => TuiPrefs::default(),
    }
}

pub(crate) fn save_prefs(prefs: &TuiPrefs) {
    let path = prefs_path();
    match serde_json::to_string_pretty(prefs) {
        Ok(raw) => {
            let _ = fs::write(path, raw);
        }
        Err(_) => {}
    }
}
