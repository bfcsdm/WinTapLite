use crate::script::types::*;
use crate::utils::coordinate;

pub fn parse_script(json: &str) -> Result<Script, ValidationError> {
    let script: Script =
        serde_json::from_str(json).map_err(|e| ValidationError {
            message: format!("JSON 解析失败: {}", e),
            field: None,
            index: None,
        })?;

    validate_script(&script)?;
    Ok(script)
}

pub fn validate_script(script: &Script) -> Result<(), ValidationError> {
    if script.name.trim().is_empty() {
        return Err(ValidationError {
            message: "脚本名称不能为空".to_string(),
            field: Some("name".to_string()),
            index: None,
        });
    }

    if script.version != 1 {
        return Err(ValidationError {
            message: format!("不支持的脚本版本: {}（仅支持版本 1）", script.version),
            field: Some("version".to_string()),
            index: None,
        });
    }

    if script.actions.is_empty() {
        return Err(ValidationError {
            message: "脚本动作列表不能为空".to_string(),
            field: Some("actions".to_string()),
            index: None,
        });
    }

    if script.speed_multiplier < 0.1 || script.speed_multiplier > 10.0 {
        return Err(ValidationError {
            message: "速度倍率必须在 0.1 ~ 10.0 之间".to_string(),
            field: Some("speed_multiplier".to_string()),
            index: None,
        });
    }

    for (i, action) in script.actions.iter().enumerate() {
        validate_action(action, i)?;
    }

    Ok(())
}

fn validate_action(action: &ScriptAction, index: usize) -> Result<(), ValidationError> {
    if action.action_type.needs_coordinate() {
        let x = action.x.ok_or(ValidationError {
            message: format!(
                "动作 #{}: 类型 \"{}\" 缺少 'x' 坐标",
                index + 1,
                action.action_type.type_name()
            ),
            field: Some("x".to_string()),
            index: Some(index),
        })?;

        let y = action.y.ok_or(ValidationError {
            message: format!(
                "动作 #{}: 类型 \"{}\" 缺少 'y' 坐标",
                index + 1,
                action.action_type.type_name()
            ),
            field: Some("y".to_string()),
            index: Some(index),
        })?;

        if let Err(e) = coordinate::validate_coordinate(x, y) {
            return Err(ValidationError {
                message: format!("动作 #{}: {}", index + 1, e),
                field: None,
                index: Some(index),
            });
        }
    }
    Ok(())
}

impl ActionType {
    fn type_name(&self) -> &str {
        match self {
            ActionType::Move => "move",
            ActionType::Click => "click",
            ActionType::DoubleClick => "double_click",
            ActionType::RightClick => "right_click",
            ActionType::Delay => "delay",
        }
    }
}

pub fn format_validation_error(err: &ValidationError) -> String {
    let mut msg = String::new();
    if let Some(ref field) = err.field {
        msg.push_str(&format!("字段 \"{}\": ", field));
    }
    if let Some(idx) = err.index {
        msg.push_str(&format!("(动作 #{}) ", idx + 1));
    }
    msg.push_str(&err.message);
    msg
}
