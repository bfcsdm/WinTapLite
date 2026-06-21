use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub name: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub actions: Vec<ScriptAction>,
    #[serde(rename = "loop")]
    pub loop_config: LoopConfig,
    #[serde(default = "default_speed")]
    pub speed_multiplier: f64,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: 1,
            description: None,
            created_at: None,
            updated_at: None,
            actions: vec![ScriptAction {
                action_type: ActionType::Delay,
                delay_ms: 0,
                x: None,
                y: None,
                absolute: None,
            }],
            loop_config: LoopConfig::default(),
            speed_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAction {
    #[serde(rename = "type")]
    pub action_type: ActionType,
    pub delay_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default = "default_absolute")]
    pub absolute: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Move,
    Click,
    DoubleClick,
    RightClick,
    Delay,
}

impl ActionType {
    pub fn needs_coordinate(&self) -> bool {
        matches!(self, ActionType::Move | ActionType::Click | ActionType::DoubleClick | ActionType::RightClick)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    pub enabled: bool,
    #[serde(default)]
    pub count: u32, // 0 = infinite
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self { enabled: false, count: 0 }
    }
}

fn default_speed() -> f64 {
    1.0
}

fn default_absolute() -> Option<bool> {
    Some(true)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptInfo {
    pub name: String,
    pub filename: String,
    pub action_count: usize,
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
    pub field: Option<String>,
    pub index: Option<usize>,
}
