// Preferences, stored as plain JSON in ~/.config/coucou/settings.json.
// No secret ever lands here — API keys live in the Secret Service (KWallet,
// GNOME Keyring, KeePassXC…).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub auto_close_interval: f64,
    pub absence_interval: f64,
    pub active_integrations: Vec<String>,
    /// Which screen edge the island is attached to: top, bottom, left or right.
    /// Always centred along that edge. Defaulted so an older settings.json loads.
    #[serde(default = "default_edge")]
    pub edge: String,
    pub autostart: bool,
    pub hooks_installed: bool,
    /// Claude model used by the chat. Changeable in the settings window.
    /// Defaulted explicitly so a settings.json written by an older build still loads.
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_edge() -> String {
    "top".to_string()
}

fn default_model() -> String {
    crate::claude::DEFAULT_MODEL.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            sound_volume: 0.12,
            auto_close_interval: 15.0,
            absence_interval: 180.0,
            active_integrations: vec![
                "integration_resend".into(),
                "integration_n8n".into(),
                "integration_vercel".into(),
                "integration_github".into(),
            ],
            edge: default_edge(),
            autostart: false,
            hooks_installed: false,
            model: default_model(),
        }
    }
}

fn xdg_dir(var: &str, fallback: &[&str]) -> PathBuf {
    if let Some(dir) = std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()) {
        return dir;
    }
    let mut home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    for part in fallback {
        home.push(part);
    }
    home
}

/// ~/.config/coucou
pub fn config_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", &[".config"]).join("coucou")
}

/// ~/.local/share/coucou — where coucou-hook, the inbox and the log live.
pub fn local_dir() -> PathBuf {
    xdg_dir("XDG_DATA_HOME", &[".local", "share"]).join("coucou")
}

pub fn hook_exe_path() -> PathBuf {
    local_dir().join("bin").join("coucou-hook")
}

/// The Unix socket coucou-hook talks to: `$XDG_RUNTIME_DIR/coucou.sock`, which
/// is a per-user 0700 directory. coucou-hook computes the same path
/// (hook/src/main.rs); the two must stay in step.
pub fn socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute()) {
        Some(dir) => dir.join("coucou.sock"),
        None => PathBuf::from(format!("/tmp/coucou-{}.sock", unsafe { libc::geteuid() })),
    }
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> Settings {
    match std::fs::read(settings_path()) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(settings_path(), json)
}
