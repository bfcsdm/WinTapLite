use std::path::{Path, PathBuf};

/// Open a native Windows file picker for .json files.
/// Returns Some(path) if the user selected a file, None if cancelled.
pub fn pick_json_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("JSON 脚本文件", &["json"])
        .set_title("导入脚本文件")
        .pick_file()
}

/// Open a folder in Windows Explorer.
pub fn open_folder(path: &Path) {
    let _ = std::process::Command::new("explorer")
        .arg(path)
        .spawn();
}
