#![windows_subsystem = "windows"]

mod app;
mod core;
mod script;
mod ui;
mod utils;

use app::ClickerApp;
use core::hotkey;

fn main() {
    let (hotkey_tx, hotkey_rx) = std::sync::mpsc::channel();
    let hotkey_handle = hotkey::start_hotkey_listener(hotkey_tx);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 550.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "自定义连点器",
        native_options,
        Box::new(move |cc| {
            setup_chinese_fonts(&cc.egui_ctx);
            let app = ClickerApp::new(hotkey_rx, hotkey_handle);
            Ok(Box::new(ClickerAppWrapper { app }))
        }),
    );
}

/// Load Windows system Chinese fonts and register with egui
fn setup_chinese_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Try multiple Windows Chinese font paths
    let font_paths = [
        "C:\\Windows\\Fonts\\msyh.ttc",   // Microsoft YaHei (微软雅黑)
        "C:\\Windows\\Fonts\\msyhbd.ttc",  // Microsoft YaHei Bold
        "C:\\Windows\\Fonts\\simsun.ttc",  // SimSun (宋体)
        "C:\\Windows\\Fonts\\simhei.ttf",  // SimHei (黑体)
    ];

    let mut loaded = false;
    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("chinese".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            loaded = true;
            break;
        }
    }

    if loaded {
        // Put Chinese font first in the proportional family so it's used by default
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "chinese".to_owned());

        // Also add to Monospace for the script editor
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("chinese".to_owned());
    }

    ctx.set_fonts(fonts);
}

struct ClickerAppWrapper {
    app: ClickerApp,
}

impl eframe::App for ClickerAppWrapper {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.process_hotkeys();
        self.app.process_clicker_events();
        self.app.process_player_events();

        egui::CentralPanel::default().show(ctx, |ui| {
            crate::ui::main_window::render(ui, &mut self.app);
        });

        // Fast refresh (20fps) when active, slow (2fps) when idle
        if self.app.needs_frequent_repaint() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}
