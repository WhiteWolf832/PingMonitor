// Module "config" — configuration JSON dans XDG_CONFIG_HOME (= config.py)
//
// Concepts Rust introduits :
//   - serde : (dé)sérialisation automatique via #[derive(Serialize, Deserialize)]
//   - #[serde(default = "...")] : valeur par défaut si la clé manque dans le JSON
//   - écriture atomique (fichier .tmp puis rename) pour ne pas corrompre le fichier

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// Un hôte surveillé. Les noms de champs correspondent aux clés JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCfg {
    pub address: String,
    #[serde(default)]
    pub name: String,
}

// La config complète. Chaque champ a une valeur par défaut (fonction `default_*`)
// utilisée si la clé est absente du fichier — comme le dict DEFAULTS côté Python.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_interval")]
    pub interval: f64,
    #[serde(default = "default_window")]
    pub window: u32,
    #[serde(default = "default_degraded_latency")]
    pub degraded_latency: f64,
    #[serde(default = "default_degraded_loss")]
    pub degraded_loss: f64,
    #[serde(default = "default_offline_loss")]
    pub offline_loss: f64,
    #[serde(default = "default_quality_good_latency")]
    pub quality_good_latency: f64,
    #[serde(default = "default_quality_good_jitter")]
    pub quality_good_jitter: f64,
    #[serde(default = "default_quality_loss_per_point")]
    pub quality_loss_per_point: f64,
    #[serde(default = "default_hosts")]
    pub hosts: Vec<HostCfg>,
}

// Valeurs par défaut (équivalent du dict DEFAULTS).
fn default_language() -> String {
    "auto".to_string()
}
fn default_interval() -> f64 {
    2.0
}
fn default_window() -> u32 {
    60
}
fn default_degraded_latency() -> f64 {
    80.0
}
fn default_degraded_loss() -> f64 {
    5.0
}
fn default_offline_loss() -> f64 {
    100.0
}
fn default_quality_good_latency() -> f64 {
    50.0
}
fn default_quality_good_jitter() -> f64 {
    15.0
}
fn default_quality_loss_per_point() -> f64 {
    5.0
}
fn default_hosts() -> Vec<HostCfg> {
    vec![
        HostCfg {
            address: "8.8.8.8".to_string(),
            name: "Google DNS".to_string(),
        },
        HostCfg {
            address: "1.1.1.1".to_string(),
            name: "Cloudflare".to_string(),
        },
    ]
}

// Default permet `Config::default()` — utilisé en repli si le fichier est absent.
// On construit un JSON vide "{}" et serde remplit tout avec les default_*.
impl Default for Config {
    fn default() -> Self {
        serde_json::from_str("{}").expect("les valeurs par défaut sont valides")
    }
}

impl Config {
    /// Charge la config depuis le disque, ou renvoie les valeurs par défaut.
    pub fn load() -> Self {
        match std::fs::read_to_string(config_path()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Enregistre la config (écriture atomique : .tmp puis rename).
    pub fn save(&self) -> std::io::Result<()> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        let tmp = dir.join("config.json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, config_path())?;
        Ok(())
    }
}

/// $XDG_CONFIG_HOME/ping-monitor-rs (ou ~/.config/ping-monitor-rs).
fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".config")
        });
    base.join("ping-monitor-rs")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}
