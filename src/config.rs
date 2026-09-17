use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub default_lang: String,
    pub show_hidden: bool,
    pub ignored_dirs: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_lang: "en".to_string(),
            show_hidden: false,
            ignored_dirs: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                ".cache".to_string(),
            ],
        }
    }
}

impl Config {
    /// `$XDG_CONFIG_HOME/rustdu/config.json` or `~/.config/rustdu/config.json`.
    /// `$XDG_CONFIG_HOME/rustdu/config.json` or `~/.config/rustdu/config.json`.
    pub fn path() -> Option<PathBuf> {
        let base = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(xdg) => PathBuf::from(xdg),
            None => {
                let home = std::env::var_os("HOME")?;
                PathBuf::from(home).join(".config")
            }
        };
        Some(base.join("rustdu").join("config.json"))
    }

    /// Load config from disk, or create it with defaults on first run.
    /// Never fails: on error, returns `Config::default()`.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };

        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<Config>(&s).ok())
                .unwrap_or_default()
        } else {
            let cfg = Self::default();
            let _ = cfg.save();
            cfg
        }
    }

    /// Persist config to disk, creating parent directories if needed.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path().ok_or_else(|| anyhow::anyhow!("no config directory available"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let s = serde_json::to_string_pretty(self)?;
        fs::write(path, s)?;
        Ok(())
    }
}
