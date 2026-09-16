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

pub fn get_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "damir", "rustdu")
        .map(|proj: directories::ProjectDirs| proj.config_dir().join("config.toml"))
}

pub fn load_config() -> Config {
    if let Some(path) = get_config_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(toml_str) = toml::to_string_pretty(&config) {
                let _ = fs::write(path, toml_str);
            }
            return config;
        }
    }
    Config::default()
}