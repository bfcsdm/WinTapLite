use egui::{CentralPanel, TopBottomPanel};
use crate::app::{ClickerApp, Tab};

pub fn render(ui: &mut egui::Ui, app: &mut ClickerApp) {
    TopBottomPanel::top("tab_bar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.current_tab, Tab::ClickMode, "🎯 绑定按键点击");
            ui.selectable_value(&mut app.current_tab, Tab::ScriptMode, "📜 脚本连点");
        });
    });

    CentralPanel::default().show_inside(ui, |ui| {
        match app.current_tab {
            Tab::ClickMode => {
                super::click_mode::render(ui, app);
            }
            Tab::ScriptMode => {
                super::script_mode::render(ui, app);
            }
        }
    });

    TopBottomPanel::bottom("status_bar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(&app.status_message);
            // Clickable folder link
            if let Some(ref path) = app.status_path.clone() {
                if ui
                    .add_sized(
                        [24.0, 18.0],
                        egui::Button::new("📂").fill(egui::Color32::TRANSPARENT),
                    )
                    .on_hover_text(format!("打开: {}", path.display()))
                    .clicked()
                {
                    crate::utils::dialog::open_folder(path);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !app.active_clickers.is_empty() {
                    ui.label("⏺ 连点运行中");
                } else if app.bindings_enabled {
                    ui.label("🟢 按键监听中");
                } else if app.recording {
                    ui.label("🔴 录制中");
                } else if app.playing {
                    ui.label("▶ 回放中");
                } else {
                    ui.label("⏸ 空闲");
                }
            });
        });
    });
}
