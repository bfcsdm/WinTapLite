use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key_name: String,
    pub vk_code: u32,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub repeat_mode: bool,
    #[serde(default = "default_interval")]
    pub interval_ms: u64,
}

fn default_interval() -> u64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingProfile {
    pub name: String,
    pub bindings: Vec<KeyBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BindingStore {
    pub profiles: Vec<BindingProfile>,
}

impl BindingStore {
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }
}

/// Convert a Windows virtual key code to a human-readable name
pub fn vk_to_name(vk: u32) -> String {
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x10 => "Shift".into(),
        0x11 => "Ctrl".into(),
        0x12 => "Alt".into(),
        0x13 => "Pause".into(),
        0x14 => "CapsLock".into(),
        0x1B => "Esc".into(),
        0x20 => "Space".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x30..=0x39 => format!("{}", vk - 0x30),
        0x41..=0x5A => {
            let c = char::from_u32(vk).unwrap_or('?');
            c.to_string()
        }
        0x60..=0x69 => format!("Numpad{}", vk - 0x60),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0xA0 => "LShift".into(),
        0xA1 => "RShift".into(),
        0xA2 => "LCtrl".into(),
        0xA3 => "RCtrl".into(),
        0xA4 => "LAlt".into(),
        0xA5 => "RAlt".into(),
        _ => format!("VK{:X}", vk),
    }
}

/// Map a key name back to a VK code (for display names that were converted)
#[allow(dead_code)]
pub fn name_to_vk(name: &str) -> Option<u32> {
    match name {
        "Backspace" => Some(0x08),
        "Tab" => Some(0x09),
        "Enter" => Some(0x0D),
        "Shift" => Some(0x10),
        "Ctrl" => Some(0x11),
        "Alt" => Some(0x12),
        "Pause" => Some(0x13),
        "CapsLock" => Some(0x14),
        "Esc" => Some(0x1B),
        "Space" => Some(0x20),
        "PageUp" => Some(0x21),
        "PageDown" => Some(0x22),
        "End" => Some(0x23),
        "Home" => Some(0x24),
        "Left" => Some(0x25),
        "Up" => Some(0x26),
        "Right" => Some(0x27),
        "Down" => Some(0x28),
        "Insert" => Some(0x2D),
        "Delete" => Some(0x2E),
        "LShift" => Some(0xA0),
        "RShift" => Some(0xA1),
        "LCtrl" => Some(0xA2),
        "RCtrl" => Some(0xA3),
        "LAlt" => Some(0xA4),
        "RAlt" => Some(0xA5),
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            if c.is_ascii_digit() {
                Some(0x30 + (c as u32 - '0' as u32))
            } else if c.is_ascii_uppercase() {
                Some(0x41 + (c as u32 - 'A' as u32))
            } else if c.is_ascii_lowercase() {
                Some(0x41 + (c as u32 - 'a' as u32))
            } else {
                None
            }
        }
        other => {
            if let Some(num) = other.strip_prefix("Numpad").and_then(|n| n.parse::<u32>().ok()) {
                if num <= 9 {
                    return Some(0x60 + num);
                }
            }
            if let Some(num) = other.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
                if num <= 24 {
                    return Some(0x70 + num - 1);
                }
            }
            None
        }
    }
}

/// Check if a VK code is a "valid bindable key" (exclude system keys)
#[allow(dead_code)]
pub fn is_bindable_key(vk: u32) -> bool {
    // Exclude F8 (pick), F9 (start), F10 (stop), F11 (record) to avoid conflicts
    if (0x77..=0x7A).contains(&vk) {
        return false;
    }
    // Exclude modifier keys (they're used as modifiers, not standalone triggers)
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5) {
        return false;
    }
    // Exclude system keys
    if matches!(vk, 0x5B | 0x5C | 0x13 | 0x2D | 0x2E) {
        return false; // Win keys, Pause, Insert, Delete
    }
    true
}
