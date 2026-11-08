//! Persistent user settings: `%APPDATA%\open-msi-control\settings.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::i18n::Lang;
use crate::profile::Profile;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `None`: follow the Windows display language.
    pub language: Option<Lang>,
    pub close_to_tray: bool,
    /// Re-apply the profile after boot, sleep and power-source changes.
    pub restore: bool,
    /// Keep a separate profile while on battery.
    pub per_power_source: bool,
    /// Profile on AC power (or the only profile). `None` until the first reading is captured.
    pub ac: Option<Profile>,
    pub battery: Option<Profile>,
    /// Stop-charge level in % (100 = no limit). `None` until captured.
    pub charge_limit: Option<u8>,
    pub check_updates: bool,
    /// Unix seconds of the last successful update check.
    pub last_update_check: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            close_to_tray: true,
            restore: true,
            per_power_source: false,
            ac: None,
            battery: None,
            charge_limit: None,
            check_updates: true,
            last_update_check: 0,
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        std::env::var_os("APPDATA")
            .map(|dir| PathBuf::from(dir).join("open-msi-control").join("settings.json"))
    }

    /// Missing or unreadable files fall back to defaults; a corrupt file is kept aside.
    pub fn load() -> Self {
        let Some(path) = Self::path() else { return Self::default() };
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                eprintln!("settings.json is invalid ({e}); starting with defaults");
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Writes atomically (temp file + rename). Failures are reported, not fatal.
    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let result = (|| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
            std::fs::rename(&tmp, &path)
        })();
        if let Err(e) = result {
            eprintln!("cannot save settings: {e}");
        }
    }

    /// The profile that applies on the given power source.
    pub fn slot(&mut self, on_ac: bool) -> &mut Option<Profile> {
        if self.per_power_source && !on_ac { &mut self.battery } else { &mut self.ac }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_or_partial_files_fill_missing_fields_with_defaults() {
        let s: Settings = serde_json::from_str(r#"{ "language": "en" }"#).unwrap_or_default();
        assert_eq!(s.language, Some(Lang::En));
        assert!(s.close_to_tray && s.restore && s.check_updates);
        assert_eq!(s.ac, None);
    }

    #[test]
    fn battery_slot_is_used_only_when_enabled() {
        let mut s = Settings::default();
        *s.slot(false) = Some(Profile::default());
        assert!(s.ac.is_some() && s.battery.is_none(), "single profile while disabled");
        s.per_power_source = true;
        assert!(s.slot(false).is_none());
        assert!(s.slot(true).is_some());
    }
}
