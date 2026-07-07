use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Top-level configuration for keycap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub timeline: TimelineConfig,
    #[serde(default)]
    pub combo: ComboConfig,
    #[serde(default)]
    pub screens: Vec<ScreenConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub launch_at_startup: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_text_color")]
    pub text_color: String,
    #[serde(default = "default_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_border_radius")]
    pub border_radius: u32,
    #[serde(default = "default_padding")]
    pub padding: u32,
    #[serde(default = "default_key_spacing")]
    pub key_spacing: u32,
    #[serde(default = "default_shadow")]
    pub shadow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineConfig {
    #[serde(default = "default_max_keys")]
    pub max_keys: usize,
    #[serde(default = "default_fade_duration_ms")]
    pub fade_duration_ms: u64,
    #[serde(default = "default_linger_duration_ms")]
    pub linger_duration_ms: u64,
    #[serde(default = "default_scroll_direction")]
    pub scroll_direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboConfig {
    #[serde(default)]
    pub show_only_combos: bool,
    #[serde(default = "default_modifier_style")]
    pub modifier_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenConfig {
    pub monitor_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_anchor")]
    pub anchor: String,
    #[serde(default)]
    pub offset_x: i32,
    #[serde(default)]
    pub offset_y: i32,
}

// ─── Defaults ────────────────────────────────────────────

fn default_mode() -> String {
    "timeline".into()
}
fn default_log_level() -> String {
    "info".into()
}
fn default_font_size() -> u32 {
    24
}
fn default_font_family() -> String {
    "monospace".into()
}
fn default_text_color() -> String {
    "#ffffff".into()
}
fn default_bg_color() -> String {
    "#00000080".into()
}
fn default_border_radius() -> u32 {
    8
}
fn default_padding() -> u32 {
    12
}
fn default_key_spacing() -> u32 {
    6
}
fn default_shadow() -> bool {
    true
}
fn default_max_keys() -> usize {
    10
}
fn default_fade_duration_ms() -> u64 {
    300
}
fn default_linger_duration_ms() -> u64 {
    2000
}
fn default_scroll_direction() -> String {
    "left".into()
}
fn default_modifier_style() -> String {
    "symbol".into()
}
fn default_anchor() -> String {
    "bottom-center".into()
}
fn default_true() -> bool {
    true
}

// ─── Default trait ────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            display: DisplayConfig::default(),
            timeline: TimelineConfig::default(),
            combo: ComboConfig::default(),
            screens: vec![],
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            launch_at_startup: false,
            log_level: default_log_level(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            font_family: default_font_family(),
            text_color: default_text_color(),
            bg_color: default_bg_color(),
            border_radius: default_border_radius(),
            padding: default_padding(),
            key_spacing: default_key_spacing(),
            shadow: default_shadow(),
        }
    }
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            max_keys: default_max_keys(),
            fade_duration_ms: default_fade_duration_ms(),
            linger_duration_ms: default_linger_duration_ms(),
            scroll_direction: default_scroll_direction(),
        }
    }
}

impl Default for ComboConfig {
    fn default() -> Self {
        Self {
            show_only_combos: false,
            modifier_style: default_modifier_style(),
        }
    }
}

// ─── Load / Save ──────────────────────────────────────────

impl Config {
    /// Return the path to `keycap.toml` next to the current executable.
    pub fn config_path() -> PathBuf {
        let mut p = Self::exe_dir();
        p.push("keycap.toml");
        p
    }

    /// Return the path to `keycap.log` next to the current executable.
    pub fn log_path() -> PathBuf {
        let mut p = Self::exe_dir();
        p.push("keycap.log");
        p
    }

    fn exe_dir() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Load config from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist current config to disk.
    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = fs::write(&path, s);
        }
    }

    /// Validate config values and clamp to sensible ranges.
    pub fn validate(&mut self) {
        self.display.font_size = self.display.font_size.clamp(8, 200);
        self.display.border_radius = self.display.border_radius.clamp(0, 100);
        self.display.padding = self.display.padding.clamp(0, 200);
        self.display.key_spacing = self.display.key_spacing.clamp(0, 200);
        self.timeline.max_keys = self.timeline.max_keys.clamp(1, 50);
        self.timeline.fade_duration_ms = self.timeline.fade_duration_ms.clamp(0, 5000);
        self.timeline.linger_duration_ms = self.timeline.linger_duration_ms.clamp(100, 30000);
    }
}
