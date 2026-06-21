use egui::{Color32, RichText, ScrollArea};
use crate::app::ClickerApp;
use crate::script::parser;

pub fn render(ui: &mut egui::Ui, app: &mut ClickerApp) {
    let running = app.playing || app.recording;

    // ── Top toolbar ──
    ui.horizontal(|ui| {
        ui.label("📜 脚本:");
        if ui.button("🔄").clicked() {
            app.refresh_scripts();
        }
        if ui.button("📄 新建").clicked() {
            new_script(app);
        }

        ui.separator();

        // Save
        if ui
            .add_enabled(
                app.script_editor_dirty && !app.script_editor_content.is_empty(),
                egui::Button::new("💾 保存"),
            )
            .clicked()
        {
            save_current_script(app);
        }

        // Save As
        if ui.button("📋 另存为").clicked() {
            save_as_dialog(app);
        }

        ui.separator();

        // Import / Export
        if ui.button("📥 导入").clicked() {
            if let Some(source) = crate::utils::dialog::pick_json_file() {
                match crate::script::storage::import_script(&app.scripts_dir, &source) {
                    Ok(()) => {
                        app.refresh_scripts();
                        app.status_message = format!("已导入: {}", source.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
                        app.status_path = None;
                    }
                    Err(e) => {
                        app.status_message = format!("导入失败: {}", e);
                        app.status_path = None;
                    }
                }
            }
        }
        if let Some(ref sel) = app.selected_script.clone() {
            if ui.button("📤 导出").clicked() {
                let dest = app.scripts_dir.join(sel);
                app.status_message = format!("脚本位置: {}", dest.display());
                app.status_path = Some(dest);
            }
        }

        // Delete
        if let Some(ref sel) = app.selected_script.clone() {
            if ui
                .add_enabled(!running, egui::Button::new("🗑 删除"))
                .clicked()
            {
                if let Err(e) = crate::script::storage::delete_script(&app.scripts_dir, sel) {
                    app.status_message = format!("删除失败: {}", e);
                } else {
                    app.selected_script = None;
                    app.script_editor_content.clear();
                    app.script_editor_dirty = false;
                    app.script_editor_error = None;
                    app.refresh_scripts();
                    app.status_message = "脚本已删除".to_string();
                }
            }
        }
    });

    ui.add_space(4.0);

    // ── Main area: script list (left) + editor (right) ──
    ui.columns(2, |cols| {
        // Left column: script list
        ScrollArea::vertical()
            .max_height(300.0)
            .show(&mut cols[0], |ui| {
                ui.label(RichText::new("脚本列表").strong());
                ui.separator();

                let scripts = app.scripts.clone();
                if scripts.is_empty() {
                    ui.label(
                        RichText::new("暂无脚本\n点击「新建」或录制操作")
                            .color(Color32::DARK_GRAY)
                            .small(),
                    );
                }
                for info in &scripts {
                    let is_sel = app.selected_script.as_deref() == Some(&info.filename);
                    let label = format!("{} ({}动作)", info.name, info.action_count);
                    if ui.selectable_label(is_sel, &label).clicked() {
                        load_script_by_name(app, &info.filename);
                    }
                }
            });

        // Right column: editor
        ScrollArea::vertical()
            .id_salt("editor_scroll_area")
            .auto_shrink([false; 2])
            .show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("编辑器").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✅ 校验").clicked() {
                        match parser::parse_script(&app.script_editor_content) {
                            Ok(_) => {
                                app.script_editor_error = None;
                                app.status_message = "校验通过 ✅".to_string();
                            }
                            Err(e) => {
                                app.script_editor_error = Some(parser::format_validation_error(&e));
                            }
                        }
                    }
                });
            });
            ui.separator();

            if let Some(ref err) = app.script_editor_error {
                ui.label(RichText::new(err).color(Color32::RED));
                ui.add_space(2.0);
            }

            if app.script_editor_dirty {
                ui.label(
                    RichText::new("● 已修改")
                        .color(Color32::from_rgb(255, 165, 0))
                        .small(),
                );
            }

            let line_count = app.script_editor_content.lines().count().max(15);
            let mut content = app.script_editor_content.clone();
            let resp = ui.add(
                egui::TextEdit::multiline(&mut content)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(line_count)
                    .desired_width(f32::INFINITY),
            );
            if resp.changed() {
                app.script_editor_content = content;
                app.script_editor_dirty = true;
                app.script_editor_error = None;
            }
        });
    });

    ui.add_space(8.0);

    // ── Playback controls ──
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("▶ 回放:");

        // Play / Stop
        if !app.playing {
            let can_play = !app.script_editor_content.is_empty() && !app.recording;
            if ui
                .add_enabled(
                    can_play,
                    egui::Button::new(RichText::new("▶ 播放 (F9)").color(Color32::GREEN)),
                )
                .clicked()
            {
                app.handle_start();
            }
        } else {
            if ui
                .add_sized(
                    [100.0, 24.0],
                    egui::Button::new(RichText::new("⏹ 停止 (F10)").color(Color32::RED)),
                )
                .clicked()
            {
                app.handle_stop();
            }
            // Progress
            let (iter, idx, total) = app.player_progress;
            if total > 0 {
                ui.label(format!("第{}轮 [{}/{}]", iter + 1, idx + 1, total));
            }
        }

        ui.separator();

        // Record
        if !app.recording {
            if ui
                .add_enabled(!app.playing, egui::Button::new("🔴 录制 (F11)"))
                .clicked()
            {
                app.toggle_recording();
            }
        } else {
            if ui
                .add_sized(
                    [120.0, 24.0],
                    egui::Button::new(RichText::new("⏹ 停止录制 (F11)").color(Color32::RED)),
                )
                .clicked()
            {
                app.toggle_recording();
            }
        }

        ui.separator();

        // Speed
        ui.label("速度:");
        ui.add(
            egui::Slider::new(&mut app.play_speed, 0.1..=10.0)
                .step_by(0.1)
                .text("x"),
        );

        // Loop
        ui.checkbox(&mut app.play_infinite, "无限循环");
        if !app.play_infinite {
            ui.add_sized(
                [50.0, 20.0],
                egui::TextEdit::singleline(&mut app.play_loop_count).hint_text("次数"),
            );
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.label(
        RichText::new("热键: F9 播放 | F10 停止 | F11 录制(切换) | 窗口最小化仍有效")
            .color(Color32::DARK_GRAY)
            .small(),
    );
}

const DEFAULT_SCRIPT_TEMPLATE: &str = r#"{
  "name": "新建脚本",
  "version": 1,
  "actions": [
    {
      "type": "click",
      "x": 500,
      "y": 300,
      "delay_ms": 1000,
      "absolute": true
    },
    {
      "type": "click",
      "x": 600,
      "y": 400,
      "delay_ms": 500,
      "absolute": true
    }
  ],
  "loop": {
    "enabled": false,
    "count": 0
  },
  "speed_multiplier": 1.0
}"#;

fn new_script(app: &mut ClickerApp) {
    app.selected_script = None;
    app.script_editor_content = DEFAULT_SCRIPT_TEMPLATE.to_string();
    app.script_editor_error = None;
    app.script_editor_dirty = true;
}

fn load_script_by_name(app: &mut ClickerApp, filename: &str) {
    match crate::script::storage::load_script_raw(&app.scripts_dir, filename) {
        Ok(content) => {
            app.script_editor_content = content;
            app.script_editor_error = None;
            app.script_editor_dirty = false;
            app.selected_script = Some(filename.to_string());
        }
        Err(e) => {
            app.status_message = e;
        }
    }
}

fn save_current_script(app: &mut ClickerApp) {
    if app.script_editor_content.is_empty() {
        return;
    }
    // Validate
    if let Err(e) = parser::parse_script(&app.script_editor_content) {
        app.script_editor_error = Some(parser::format_validation_error(&e));
        return;
    }

    let filename = match app.selected_script.clone() {
        Some(f) => f,
        None => {
            // Generate filename from script name
            if let Ok(script) =
                serde_json::from_str::<crate::script::types::Script>(&app.script_editor_content)
            {
                let safe = script.name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
                format!("{}.json", safe)
            } else {
                "untitled.json".to_string()
            }
        }
    };

    match crate::script::storage::save_script_raw(&app.scripts_dir, &filename, &app.script_editor_content) {
        Ok(()) => {
            app.selected_script = Some(filename);
            app.script_editor_dirty = false;
            app.script_editor_error = None;
            app.refresh_scripts();
            app.status_message = "已保存".to_string();
        }
        Err(e) => {
            app.status_message = format!("保存失败: {}", e);
        }
    }
}

fn save_as_dialog(app: &mut ClickerApp) {
    // Simple: use a dialog to get a new name
    // For now, generate a new filename from script name
    if app.script_editor_content.is_empty() {
        return;
    }
    if let Ok(script) = serde_json::from_str::<crate::script::types::Script>(&app.script_editor_content) {
        let safe = script.name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
        let filename = format!("{}.json", safe);
        match crate::script::storage::save_script_raw(&app.scripts_dir, &filename, &app.script_editor_content) {
            Ok(()) => {
                app.selected_script = Some(filename);
                app.script_editor_dirty = false;
                app.script_editor_error = None;
                app.refresh_scripts();
                app.status_message = "已另存为新文件".to_string();
            }
            Err(e) => {
                app.status_message = format!("保存失败: {}", e);
            }
        }
    }
}
