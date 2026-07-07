use rdev::Key;
use std::collections::VecDeque;
use std::time::Instant;

use crate::capture::KeyEvent;
use crate::config::{ComboConfig, Config, TimelineConfig};

/// Render output a mode produces — a flat list of key labels to display.
#[derive(Debug, Clone)]
pub struct KeyLabel {
    pub text: String,
    /// 0.0 = fully transparent, 1.0 = fully opaque (for fade animations).
    pub opacity: f32,
}

/// Common interface for display modes.
pub trait DisplayMode: Send {
    /// Push a captured key event into the mode.
    fn push_event(&mut self, event: KeyEvent);
    /// Produce the current list of labels to render, given elapsed time.
    fn render(&self, now: Instant) -> Vec<KeyLabel>;
    /// Reset mode state (e.g. on mode switch).
    fn reset(&mut self);
}

// ─── Key → Human-readable Label ───────────────────────────

/// Map an rdev `Key` to a short human-readable label.
pub fn key_to_label(key: Key) -> String {
    use Key::*;
    let s = match key {
        // Letters
        KeyA => "A", KeyB => "B", KeyC => "C", KeyD => "D", KeyE => "E",
        KeyF => "F", KeyG => "G", KeyH => "H", KeyI => "I", KeyJ => "J",
        KeyK => "K", KeyL => "L", KeyM => "M", KeyN => "N", KeyO => "O",
        KeyP => "P", KeyQ => "Q", KeyR => "R", KeyS => "S", KeyT => "T",
        KeyU => "U", KeyV => "V", KeyW => "W", KeyX => "X", KeyY => "Y", KeyZ => "Z",

        // Digits
        Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4",
        Num5 => "5", Num6 => "6", Num7 => "7", Num8 => "8", Num9 => "9",

        // Modifiers
        ControlLeft | ControlRight => "Ctrl",
        Alt | AltGr => "Alt",
        ShiftLeft | ShiftRight => "Shift",
        MetaLeft | MetaRight => "Win",

        // Whitespace
        Space => "␣",
        Return => "↵",
        Tab => "⇥",
        Backspace => "⌫",
        Escape => "Esc",
        Delete => "Del",

        // Navigation
        UpArrow => "↑", DownArrow => "↓",
        LeftArrow => "←", RightArrow => "→",
        PageUp => "PgUp", PageDown => "PgDn",
        Home => "Home", End => "End",

        // Function keys
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4",
        F5 => "F5", F6 => "F6", F7 => "F7", F8 => "F8",
        F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",

        // Numpad
        Kp0 => "N0", Kp1 => "N1", Kp2 => "N2", Kp3 => "N3", Kp4 => "N4",
        Kp5 => "N5", Kp6 => "N6", Kp7 => "N7", Kp8 => "N8", Kp9 => "N9",
        KpPlus => "N+", KpMinus => "N−", KpMultiply => "N×", KpDivide => "N÷",
        KpDelete => "N.", KpReturn => "N↵",

        // Punctuation
        Minus => "-", Equal => "=",
        LeftBracket => "[", RightBracket => "]",
        BackSlash => "\\", SemiColon => ";", Quote => "'",
        Comma => ",", Dot => ".", Slash => "/",
        BackQuote => "`",

        // Lock keys
        CapsLock => "⇪", NumLock => "⇭", ScrollLock => "⇳",

        // Media / special
        PrintScreen => "PrtSc", Pause => "Pause",
        Insert => "Ins",

        // Catch-all: strip "Key" prefix, early-return to keep match arms uniform
        _ => {
            let dbg = format!("{:?}", key);
            return dbg.strip_prefix("Key").unwrap_or(&dbg).to_owned();
        }
    };
    s.to_string()
}

// ─── Timeline Mode ────────────────────────────────────────

struct Entry {
    label: String,
    /// When this key was first pressed (or released for linger timing).
    timestamp: Instant,
}

pub struct TimelineMode {
    entries: VecDeque<Entry>,
    config: TimelineConfig,
    fade_ms: f32,
    linger_ms: f32,
}

impl TimelineMode {
    pub fn new(config: &Config) -> Self {
        Self {
            entries: VecDeque::new(),
            config: config.timeline.clone(),
            fade_ms: config.timeline.fade_duration_ms as f32,
            linger_ms: config.timeline.linger_duration_ms as f32,
        }
    }

    pub fn update_config(&mut self, config: &Config) {
        self.config = config.timeline.clone();
        self.fade_ms = config.timeline.fade_duration_ms as f32;
        self.linger_ms = config.timeline.linger_duration_ms as f32;
    }

    fn key_to_label(key: Key) -> String {
        crate::mode::key_to_label(key)
    }
}

impl DisplayMode for TimelineMode {
    fn push_event(&mut self, event: KeyEvent) {
        if let KeyEvent::Press(key) = event {
            let label = Self::key_to_label(key);
            self.entries.push_back(Entry {
                label,
                timestamp: Instant::now(),
            });
            // Trim to max_keys
            while self.entries.len() > self.config.max_keys {
                self.entries.pop_front();
            }
        }
    }

    fn render(&self, now: Instant) -> Vec<KeyLabel> {
        let total_duration = (self.linger_ms + self.fade_ms) as f32;
        self.entries
            .iter()
            .map(|e| {
                let age_ms = (now - e.timestamp).as_millis() as f32;
                let opacity = if age_ms <= self.linger_ms {
                    1.0
                } else if age_ms < total_duration {
                    1.0 - (age_ms - self.linger_ms) / self.fade_ms
                } else {
                    0.0
                };
                KeyLabel {
                    text: e.label.clone(),
                    opacity: opacity.clamp(0.0, 1.0),
                }
            })
            .collect()
    }

    fn reset(&mut self) {
        self.entries.clear();
    }
}

// ─── Combo Mode ───────────────────────────────────────────

pub struct ComboMode {
    /// Currently held keys (key → press timestamp).
    held: Vec<(Key, Instant)>,
    config: ComboConfig,
}

impl ComboMode {
    pub fn new(config: &Config) -> Self {
        Self {
            held: Vec::new(),
            config: config.combo.clone(),
        }
    }

    pub fn update_config(&mut self, config: &Config) {
        self.config = config.combo.clone();
    }

    fn format_key(&self, key: Key) -> String {
        use Key::*;
        match self.config.modifier_style.as_str() {
            "symbol" => match key {
                ControlLeft | ControlRight => "⌃".into(),
                Alt | AltGr => "⌥".into(),
                ShiftLeft | ShiftRight => "⇧".into(),
                MetaLeft | MetaRight => "⌘".into(),
                _ => key_to_label(key),
            },
            "text" => match key {
                ControlLeft | ControlRight => "Ctrl".into(),
                Alt | AltGr => "Alt".into(),
                ShiftLeft | ShiftRight => "Shift".into(),
                MetaLeft | MetaRight => "Cmd".into(),
                _ => key_to_label(key),
            },
            _ => key_to_label(key),
        }
    }

    fn is_modifier(key: Key) -> bool {
        use Key::*;
        matches!(
            key,
            ControlLeft
                | ControlRight
                | Alt
                | AltGr
                | ShiftLeft
                | ShiftRight
                | MetaLeft
                | MetaRight
        )
    }
}

impl DisplayMode for ComboMode {
    fn push_event(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Press(key) => {
                // Keep only the most recent press for each key
                self.held.retain(|(k, _)| *k != key);
                self.held.push((key, Instant::now()));
            }
            KeyEvent::Release(key) => {
                self.held.retain(|(k, _)| *k != key);
            }
        }
    }

    fn render(&self, _now: Instant) -> Vec<KeyLabel> {
        if self.held.is_empty() {
            return vec![];
        }

        // Sort: modifiers first
        let mut keys: Vec<&Key> = self.held.iter().map(|(k, _)| k).collect();
        keys.sort_by(|a, b| {
            let a_mod = Self::is_modifier(**a);
            let b_mod = Self::is_modifier(**b);
            b_mod.cmp(&a_mod) // modifiers first
        });

        if self.config.show_only_combos {
            let has_mod = keys.iter().any(|k| Self::is_modifier(**k));
            let has_key = keys.iter().any(|k| !Self::is_modifier(**k));
            if !(has_mod && has_key) {
                return vec![];
            }
        }

        let text = keys
            .iter()
            .map(|k| self.format_key(**k))
            .collect::<Vec<_>>()
            .join(" + ");

        vec![KeyLabel {
            text,
            opacity: 1.0,
        }]
    }

    fn reset(&mut self) {
        self.held.clear();
    }
}

// ─── Factory ──────────────────────────────────────────────

pub fn create_mode(config: &Config) -> Box<dyn DisplayMode> {
    match config.global.mode.as_str() {
        "combo" => Box::new(ComboMode::new(config)),
        _ => Box::new(TimelineMode::new(config)),
    }
}
