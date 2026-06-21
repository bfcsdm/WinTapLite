use crate::script::parser;
use crate::script::types::*;
use std::fs;
use std::path::{Path, PathBuf};

/// List all scripts in the scripts directory
pub fn list_scripts(scripts_dir: &Path) -> Result<Vec<ScriptInfo>, String> {
    if !scripts_dir.exists() {
        fs::create_dir_all(scripts_dir).map_err(|e| e.to_string())?;
    }

    let mut scripts = Vec::new();
    let entries = fs::read_dir(scripts_dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            match path.file_stem() {
                Some(stem) => {
                    let filename = stem.to_string_lossy().to_string();
                    // Try to read script name from file
                    let action_count = match fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<Script>(&content) {
                                Ok(script) => script.actions.len(),
                                Err(_) => 0,
                            }
                        }
                        Err(_) => 0,
                    };
                    scripts.push(ScriptInfo {
                        name: filename.clone(),
                        filename: format!("{}.json", filename),
                        action_count,
                    });
                }
                None => continue,
            }
        }
    }

    scripts.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(scripts)
}

#[allow(dead_code)]
pub fn load_script(scripts_dir: &Path, filename: &str) -> Result<Script, String> {
    let path = scripts_dir.join(filename);
    let content = fs::read_to_string(&path).map_err(|e| format!("无法读取脚本文件: {}", e))?;
    parser::parse_script(&content).map_err(|e| e.message)
}

/// Load raw JSON content from a script file
pub fn load_script_raw(scripts_dir: &Path, filename: &str) -> Result<String, String> {
    let path = scripts_dir.join(filename);
    fs::read_to_string(&path).map_err(|e| format!("无法读取脚本文件: {}", e))
}

#[allow(dead_code)]
pub fn save_script(scripts_dir: &Path, filename: &str, script: &Script) -> Result<(), String> {
    if !scripts_dir.exists() {
        fs::create_dir_all(scripts_dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(script).map_err(|e| e.to_string())?;
    let path = scripts_dir.join(filename);
    fs::write(&path, json).map_err(|e| e.to_string())
}

/// Save raw JSON string to a script file
pub fn save_script_raw(scripts_dir: &Path, filename: &str, json: &str) -> Result<(), String> {
    if !scripts_dir.exists() {
        fs::create_dir_all(scripts_dir).map_err(|e| e.to_string())?;
    }
    // Validate before saving
    parser::parse_script(json).map_err(|e| e.message)?;
    let path = scripts_dir.join(filename);
    fs::write(&path, json).map_err(|e| e.to_string())
}

/// Delete a script file
pub fn delete_script(scripts_dir: &Path, filename: &str) -> Result<(), String> {
    let path = scripts_dir.join(filename);
    fs::remove_file(&path).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn rename_script(scripts_dir: &Path, old_name: &str, new_name: &str) -> Result<(), String> {
    let old_path = scripts_dir.join(old_name);
    let new_path = scripts_dir.join(new_name);
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn import_script(scripts_dir: &Path, source_path: &Path) -> Result<(), String> {
    if !scripts_dir.exists() {
        fs::create_dir_all(scripts_dir).map_err(|e| e.to_string())?;
    }

    // Validate the file is a valid script
    let content = fs::read_to_string(source_path).map_err(|e| format!("无法读取导入文件: {}", e))?;
    parser::parse_script(&content).map_err(|e| e.message)?;

    let filename = source_path
        .file_name()
        .ok_or("无效的文件名")?
        .to_string_lossy();
    let dest_path = scripts_dir.join(filename.as_ref());
    fs::copy(source_path, &dest_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn export_script(scripts_dir: &Path, filename: &str, dest_path: &Path) -> Result<(), String> {
    let src = scripts_dir.join(filename);
    fs::copy(&src, dest_path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get the default scripts directory
pub fn get_scripts_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("scripts")
}
