use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub transparency: u8,
    #[serde(default)]
    pub keep_text_opaque: bool,
    #[serde(default = "default_show_stats")]
    pub show_stats: bool,
    #[serde(default)]
    pub close_behavior: CloseBehavior,
    #[serde(default)]
    pub theme: ThemePreference,
    #[serde(default)]
    pub project_app: Option<String>,
    #[serde(default)]
    pub project_apps: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseBehavior {
    #[default]
    Hide,
    Quit,
}

fn default_show_stats() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            transparency: 0,
            keep_text_opaque: false,
            show_stats: default_show_stats(),
            close_behavior: CloseBehavior::default(),
            theme: ThemePreference::default(),
            project_app: None,
            project_apps: BTreeMap::new(),
        }
    }
}

pub fn config_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|p| p.join("repostatus").join("projects.json"))
        .ok_or_else(|| "Could not locate the user configuration directory.".into())
}

pub fn load(path: &Path) -> Result<Vec<PathBuf>, String> {
    load_config(path).map(|config| config.paths)
}

pub fn load_config(path: &Path) -> Result<Config, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Config>(&bytes)
            .and_then(|config| {
                if config.transparency > 50 {
                    return Err(<serde_json::Error as serde::de::Error>::custom(
                        "transparency must be between 0 and 50",
                    ));
                }
                Ok(config)
            })
            .map_err(|e| format!("Cannot read saved projects: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("Cannot load projects: {e}")),
    }
}

pub fn save(path: &Path, paths: Vec<PathBuf>) -> Result<(), String> {
    let mut config = load_config(path)?;
    config.paths = paths;
    save_config(path, &config)
}

pub fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    if config.transparency > 50 {
        return Err("Transparency must be between 0 and 50.".into());
    }
    let parent = path.parent().ok_or("Invalid configuration path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|e| format!("Cannot save projects: {e}"))?;
    fs::rename(temporary, path).map_err(|e| format!("Cannot save projects: {e}"))
}
