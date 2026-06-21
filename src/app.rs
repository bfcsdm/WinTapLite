use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use crate::core::clicker::{self, ClickerEvent, NoStealClicker};
use crate::core::hotkey::{self, HotkeyEvent};
use crate::core::recorder::Recorder;
use crate::script::player::{Player, PlayerEvent};
use crate::script::storage;
use crate::script::types::ScriptInfo;
use crate::utils::bindings::{self, BindingProfile, BindingStore, KeyBinding};
use crate::utils::favorites::FavoritesStore;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    ClickMode,
    ScriptMode,
}

pub struct ClickerApp {
    pub current_tab: Tab,

    // ── Click Mode: Binding Profiles ──
    pub bindings_enabled: bool,
    pub binding_store: BindingStore,
    pub bindings_path: PathBuf,
    pub active_profile_idx: usize,
    /// Per-binding repeat clickers keyed by vk_code
    pub active_clickers: HashMap<u32, NoStealClicker>,
    /// Clicker event receivers keyed by vk_code
    pub clicker_rxs: HashMap<u32, mpsc::Receiver<ClickerEvent>>,
    /// Click progress per binding
    pub clicker_progress: HashMap<u32, u64>,

    // Picked coordinates (from F8 or 📌 button)
    pub picked_x: Option<i32>,
    pub picked_y: Option<i32>,

    // UI state for key capture
    pub capturing_key: bool,
    pub capture_rebind_idx: Option<usize>,
    pub captured_key_name: String,
    pub captured_vk: u32,

    // ── Favorites ──
    pub favorites: FavoritesStore,
    pub favorites_path: PathBuf,
    pub new_fav_name: String,

    // ── Script Mode ──
    pub scripts: Vec<ScriptInfo>,
    pub scripts_dir: PathBuf,
    pub selected_script: Option<String>,
    pub script_editor_content: String,
    pub script_editor_error: Option<String>,
    pub script_editor_dirty: bool,

    // Recording
    pub recording: bool,
    pub recorder: Recorder,

    // Playback
    pub playing: bool,
    pub player: Option<Player>,
    pub player_rx: Option<mpsc::Receiver<PlayerEvent>>,
    pub player_progress: (u32, usize, usize), // (iteration, action_idx, total)

    pub play_speed: f64,
    pub play_loop_count: String,
    pub play_infinite: bool,

    // ── Hotkey ──
    pub hotkey_rx: mpsc::Receiver<HotkeyEvent>,
    #[allow(dead_code)]
    pub hotkey_handle: Option<hotkey::HotkeyHandle>,

    // ── Status ──
    pub status_message: String,
    pub status_path: Option<std::path::PathBuf>,
}

impl ClickerApp {
    pub fn new(hotkey_rx: mpsc::Receiver<HotkeyEvent>, hotkey_handle: hotkey::HotkeyHandle) -> Self {
        let scripts_dir = storage::get_scripts_dir();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let favorites_path = exe_dir.join("favorites.json");
        let bindings_path = exe_dir.join("binding_profiles.json");

        let favorites = FavoritesStore::load(&favorites_path);
        let scripts = storage::list_scripts(&scripts_dir).unwrap_or_default();
        let binding_store = BindingStore::load(&bindings_path);

        // Ensure at least one profile exists
        let mut store = binding_store;
        if store.profiles.is_empty() {
            store.profiles.push(BindingProfile {
                name: "方案1".to_string(),
                bindings: Vec::new(),
            });
        }

        ClickerApp {
            current_tab: Tab::ClickMode,
            bindings_enabled: false,
            binding_store: store,
            bindings_path,
            active_profile_idx: 0,
            active_clickers: HashMap::new(),
            clicker_rxs: HashMap::new(),
            clicker_progress: HashMap::new(),
            picked_x: None,
            picked_y: None,
            capturing_key: false,
            capture_rebind_idx: None,
            captured_key_name: String::new(),
            captured_vk: 0,
            favorites,
            favorites_path,
            new_fav_name: String::new(),
            scripts,
            scripts_dir,
            selected_script: None,
            script_editor_content: String::new(),
            script_editor_error: None,
            script_editor_dirty: false,
            recording: false,
            recorder: Recorder::new(),
            playing: false,
            player: None,
            player_rx: None,
            player_progress: (0, 0, 0),
            play_speed: 1.0,
            play_loop_count: String::new(),
            play_infinite: true,
            hotkey_rx,
            hotkey_handle: Some(hotkey_handle),
            status_message: String::from("就绪 — F8 拾取 | F10 停止 | 绑定按键后直接按下触发"),
            status_path: None,
        }

        // Sync bound keys to hotkey system
        // (handled after construction via sync method)
    }

    /// Notify the hotkey system of the current set of bound keys.
    pub fn sync_bound_keys(&self) {
        let keys: std::collections::HashSet<u32> = self
            .current_profile()
            .map(|p| p.bindings.iter().map(|b| b.vk_code).collect())
            .unwrap_or_default();
        hotkey::update_bound_keys(keys);
    }

    pub fn current_profile(&self) -> Option<&BindingProfile> {
        self.binding_store.profiles.get(self.active_profile_idx)
    }

    pub fn current_profile_mut(&mut self) -> Option<&mut BindingProfile> {
        self.binding_store.profiles.get_mut(self.active_profile_idx)
    }

    /// Add a binding to the current profile.
    pub fn add_binding(&mut self, vk_code: u32, key_name: &str, x: i32, y: i32) {
        if let Some(profile) = self.current_profile_mut() {
            // Don't duplicate key
            if profile.bindings.iter().any(|b| b.vk_code == vk_code) {
                self.status_message = format!("按键 {} 已绑定", key_name);
                return;
            }
            profile.bindings.push(KeyBinding {
                key_name: key_name.to_string(),
                vk_code,
                x,
                y,
                repeat_mode: false,
                interval_ms: 100,
            });
            self.sync_bound_keys();
            self.status_message = format!("已绑定 {} → ({}, {})", key_name, x, y);
        }
    }

    /// Remove a binding by index.
    pub fn remove_binding(&mut self, index: usize) {
        let vk_code = self
            .current_profile()
            .and_then(|p| p.bindings.get(index).map(|b| b.vk_code));

        if let Some(vk) = vk_code {
            self.stop_binding_clicker(vk);
        }

        if let Some(profile) = self.current_profile_mut() {
            if index < profile.bindings.len() {
                profile.bindings.remove(index);
                self.sync_bound_keys();
            }
        }
    }

    /// Update a binding's fields.
    pub fn update_binding(&mut self, index: usize, x: Option<i32>, y: Option<i32>, repeat_mode: Option<bool>, interval_ms: Option<u64>) {
        if let Some(profile) = self.current_profile_mut() {
            if let Some(b) = profile.bindings.get_mut(index) {
                if let Some(x) = x { b.x = x; }
                if let Some(y) = y { b.y = y; }
                if let Some(rm) = repeat_mode { b.repeat_mode = rm; }
                if let Some(iv) = interval_ms { b.interval_ms = iv.max(1); }
            }
        }
        self.save_bindings();
    }

    /// Set binding key (for re-binding an existing entry).
    pub fn rebind_key(&mut self, index: usize, vk_code: u32, key_name: &str) {
        if let Some(profile) = self.current_profile_mut() {
            if let Some(b) = profile.bindings.get_mut(index) {
                b.vk_code = vk_code;
                b.key_name = key_name.to_string();
            }
        }
        self.stop_binding_clicker(vk_code);
        self.sync_bound_keys();
        self.save_bindings();
    }

    /// Toggle a repeat-mode clicker for a binding.
    fn toggle_binding_clicker(&mut self, vk_code: u32, x: i32, y: i32, interval_ms: u64) {
        if self.active_clickers.remove(&vk_code).is_some() {
            self.clicker_rxs.remove(&vk_code);
            self.clicker_progress.remove(&vk_code);
            self.status_message = format!("已停止 {}", bindings::vk_to_name(vk_code));
            return;
        }

        let (etx, erx) = mpsc::channel();
        let clicker = NoStealClicker::start(x, y, interval_ms, Some(etx));
        self.active_clickers.insert(vk_code, clicker);
        self.clicker_rxs.insert(vk_code, erx);
        self.clicker_progress.insert(vk_code, 0);
        self.status_message = format!(
            "连点中 {} → ({}, {}) {}ms",
            bindings::vk_to_name(vk_code),
            x,
            y,
            interval_ms
        );
    }

    #[allow(dead_code)]
    fn fire_single_click(&self, x: i32, y: i32) {
        clicker::send_click_at(x, y);
    }

    fn stop_binding_clicker(&mut self, vk_code: u32) {
        self.active_clickers.remove(&vk_code);
        self.clicker_rxs.remove(&vk_code);
        self.clicker_progress.remove(&vk_code);
    }

    pub fn stop_all_binding_clickers(&mut self) {
        self.active_clickers.clear();
        self.clicker_rxs.clear();
        self.clicker_progress.clear();
    }

    /// Whether the app needs frequent (20fps) repaint for live progress updates.
    pub fn needs_frequent_repaint(&self) -> bool {
        !self.active_clickers.is_empty() || self.recording || self.playing
    }

    pub fn toggle_bindings_enabled(&mut self) {
        self.bindings_enabled = !self.bindings_enabled;
        if self.bindings_enabled {
            self.status_message = "按键绑定已启用 — 按下绑定的按键触发点击".to_string();
        } else {
            self.stop_all_binding_clickers();
            self.status_message = "按键绑定已禁用".to_string();
        }
    }

    pub fn save_bindings(&mut self) {
        if let Err(e) = self.binding_store.save(&self.bindings_path) {
            self.status_message = format!("保存方案失败: {}", e);
        }
    }

    // ── Profile management ──

    pub fn switch_profile(&mut self, idx: usize) {
        self.stop_all_binding_clickers();
        self.active_profile_idx = idx;
        self.sync_bound_keys();
        if let Some(p) = self.current_profile() {
            self.status_message = format!("已切换到 {}", p.name);
        }
    }

    pub fn new_profile(&mut self, name: &str) {
        self.stop_all_binding_clickers();
        self.binding_store.profiles.push(BindingProfile {
            name: name.to_string(),
            bindings: Vec::new(),
        });
        self.active_profile_idx = self.binding_store.profiles.len() - 1;
        self.sync_bound_keys();
        self.save_bindings();
        self.status_message = format!("已创建方案「{}」", name);
    }

    pub fn delete_profile(&mut self, idx: usize) {
        if self.binding_store.profiles.len() <= 1 {
            self.status_message = "至少保留一个方案".to_string();
            return;
        }
        self.stop_all_binding_clickers();
        self.binding_store.profiles.remove(idx);
        if self.active_profile_idx >= self.binding_store.profiles.len() {
            self.active_profile_idx = self.binding_store.profiles.len() - 1;
        }
        self.sync_bound_keys();
        self.save_bindings();
        self.status_message = "方案已删除".to_string();
    }

    pub fn rename_profile(&mut self, idx: usize, new_name: &str) {
        if let Some(profile) = self.binding_store.profiles.get_mut(idx) {
            profile.name = new_name.to_string();
        }
        self.save_bindings();
    }

    // ── Hotkey processing ──

    pub fn process_hotkeys(&mut self) {
        while let Ok(event) = self.hotkey_rx.try_recv() {
            match event {
                HotkeyEvent::PickCoordinate => self.handle_pick_coordinate(),
                HotkeyEvent::Start => {
                    // F9: toggle bindings master switch (click mode only)
                    if self.current_tab == Tab::ClickMode {
                        self.toggle_bindings_enabled();
                    }
                }
                HotkeyEvent::Stop => {
                    // F10: disable bindings + stop all clickers + stop player
                    self.bindings_enabled = false;
                    self.stop_all_binding_clickers();
                    if self.playing {
                        if let Some(p) = self.player.take() {
                            p.stop();
                        }
                        self.player_rx = None;
                        self.playing = false;
                    }
                    if self.recording {
                        self.stop_recording();
                    }
                    self.status_message = "已停止 — 按键绑定已禁用".to_string();
                }
                HotkeyEvent::RecordStartStop => self.toggle_recording(),
                HotkeyEvent::UserKey { vk_code, key_down } => {
                    self.handle_user_key(vk_code, key_down);
                }
            }
        }
    }

    fn handle_user_key(&mut self, vk_code: u32, key_down: bool) {
        if self.current_tab != Tab::ClickMode {
            return;
        }
        // If capturing a key for binding, handle that (always allowed)
        if self.capturing_key && key_down {
            self.captured_vk = vk_code;
            self.captured_key_name = bindings::vk_to_name(vk_code);
            self.capturing_key = false;
            self.status_message = format!("已捕获按键: {}", self.captured_key_name);

            // If rebinding an existing binding
            if let Some(idx) = self.capture_rebind_idx.take() {
                self.rebind_key(idx, vk_code, &self.captured_key_name.clone());
                self.captured_key_name.clear();
                self.captured_vk = 0;
            }
            return;
        }

        // If bindings are disabled, ignore
        if !self.bindings_enabled {
            return;
        }

        // Look up binding
        let binding = {
            self.current_profile()
                .and_then(|p| p.bindings.iter().find(|b| b.vk_code == vk_code))
                .cloned()
        };

        if let Some(b) = binding {
            if !key_down {
                return; // Only act on key-down
            }
            if b.repeat_mode {
                self.toggle_binding_clicker(b.vk_code, b.x, b.y, b.interval_ms);
            } else {
                clicker::send_click_at(b.x, b.y);
                self.status_message = format!("{} → ({}, {})", b.key_name, b.x, b.y);
            }
        }
    }

    pub fn process_clicker_events(&mut self) {
        let mut completed: Vec<u32> = Vec::new();
        let mut new_progress: Vec<(u32, u64)> = Vec::new();

        for (&vk, rx) in &self.clicker_rxs {
            loop {
                match rx.try_recv() {
                    Ok(ClickerEvent::Tick { clicks }) => {
                        new_progress.push((vk, clicks));
                    }
                    Ok(ClickerEvent::Done { clicks }) => {
                        new_progress.push((vk, 0)); // reset progress
                        completed.push(vk);
                        self.status_message = format!(
                            "{} 连点完成 — {} 次",
                            bindings::vk_to_name(vk),
                            clicks
                        );
                        break;
                    }
                    Err(_) => break,
                }
            }
        }

        for (vk, prog) in new_progress {
            self.clicker_progress.insert(vk, prog);
        }
        for vk in completed {
            self.active_clickers.remove(&vk);
            self.clicker_rxs.remove(&vk);
        }
    }

    pub fn process_player_events(&mut self) {
        let mut done = false;
        let mut progress = self.player_progress;

        if let Some(ref rx) = self.player_rx {
            loop {
                match rx.try_recv() {
                    Ok(PlayerEvent::Progress { iteration, action_index, total_actions }) => {
                        progress = (iteration, action_index, total_actions);
                    }
                    Ok(PlayerEvent::Done) => {
                        done = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }

        self.player_progress = progress;
        if done {
            self.playing = false;
            self.player = None;
            self.player_rx = None;
            self.status_message = "脚本回放完成".to_string();
        }
    }

    // ── Actions ──

    fn handle_pick_coordinate(&mut self) {
        match clicker::get_cursor_pos() {
            Some((x, y)) => {
                self.picked_x = Some(x);
                self.picked_y = Some(y);
                self.status_message = format!("已拾取坐标 ({}, {})", x, y);
            }
            None => {
                self.status_message = "坐标拾取失败".to_string();
            }
        }
    }

    /// Pick current cursor position and return it (for UI to use).
    pub fn pick_coordinate(&self) -> Option<(i32, i32)> {
        clicker::get_cursor_pos()
    }

    pub fn handle_start(&mut self) {
        match self.current_tab {
            Tab::ClickMode => {
                // In click mode, start is handled per-binding via key presses.
                // F9 could be used to trigger all repeat-mode bindings at once.
                self.toggle_all_repeat_bindings();
            }
            Tab::ScriptMode => self.start_player(),
        }
    }

    fn toggle_all_repeat_bindings(&mut self) {
        let bindings: Vec<KeyBinding> = self
            .current_profile()
            .map(|p| {
                p.bindings
                    .iter()
                    .filter(|b| b.repeat_mode)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        if bindings.is_empty() {
            self.status_message = "没有配置连点模式的按键绑定".to_string();
            return;
        }

        // If any repeat clicker is running, stop all. Otherwise start all.
        let any_running = !self.active_clickers.is_empty();
        if any_running {
            self.stop_all_binding_clickers();
            self.status_message = "已停止所有连点".to_string();
        } else {
            for b in &bindings {
                self.toggle_binding_clicker(b.vk_code, b.x, b.y, b.interval_ms);
            }
            self.status_message = format!("已启动 {} 个连点绑定", bindings.len());
        }
    }

    fn start_player(&mut self) {
        if self.playing || self.script_editor_content.is_empty() {
            return;
        }
        let mut script: crate::script::types::Script = match serde_json::from_str(&self.script_editor_content) {
            Ok(s) => s,
            Err(e) => {
                self.script_editor_error = Some(format!("JSON 解析失败: {}", e));
                return;
            }
        };

        script.loop_config.enabled = self.play_infinite || !self.play_loop_count.trim().is_empty();
        if !self.play_infinite {
            if let Ok(n) = self.play_loop_count.trim().parse::<u32>() {
                script.loop_config.count = n;
            }
        } else {
            script.loop_config.count = 0;
        }
        script.speed_multiplier = self.play_speed;

        let (etx, erx) = mpsc::channel();
        self.player = Some(Player::start(script, Some(etx)));
        self.player_rx = Some(erx);
        self.playing = true;
        self.player_progress = (0, 0, 0);
        self.status_message = "脚本回放中...".to_string();
    }

    pub fn handle_stop(&mut self) {
        self.bindings_enabled = false;
        self.stop_all_binding_clickers();
        if self.playing {
            if let Some(p) = self.player.take() {
                p.stop();
            }
            self.player_rx = None;
            self.playing = false;
        }
        if self.recording {
            self.stop_recording();
        }
        self.status_message = "已停止".to_string();
    }

    pub fn toggle_recording(&mut self) {
        if self.playing {
            return;
        }
        if self.recording {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    pub fn start_recording(&mut self) {
        if self.recording {
            return;
        }
        self.recorder.start();
        self.recording = true;
        self.status_message = "🔴 正在录制 — 按 F11 停止".to_string();
    }

    pub fn stop_recording(&mut self) {
        let events = self.recorder.stop();
        self.recording = false;

        let script = convert_events_to_script(&events);
        match serde_json::to_string_pretty(&script) {
            Ok(json) => {
                self.script_editor_content = json;
                self.script_editor_error = None;
                self.script_editor_dirty = true;
                self.selected_script = None;
                self.current_tab = Tab::ScriptMode;
                self.status_message = format!("录制完成 — {} 个操作", script.actions.len());
            }
            Err(e) => {
                self.status_message = format!("脚本生成失败: {}", e);
            }
        }
    }

    pub fn save_favorites(&mut self) {
        if let Err(e) = self.favorites.save(&self.favorites_path) {
            self.status_message = format!("保存收藏失败: {}", e);
        }
    }

    pub fn refresh_scripts(&mut self) {
        self.scripts = storage::list_scripts(&self.scripts_dir).unwrap_or_default();
    }
}

// ── Raw events → Script conversion ──

fn convert_events_to_script(events: &[crate::core::recorder::RawMouseEvent]) -> crate::script::types::Script {
    use crate::core::recorder::{RawEventType, RawMouseEvent};
    use crate::script::types::*;

    let mut actions: Vec<ScriptAction> = Vec::new();
    let mut prev_time: u64 = 0;
    let mut pending_left: Option<&RawMouseEvent> = None;

    for event in events {
        match event.event_type {
            RawEventType::LeftDown => {
                pending_left = Some(event);
            }
            RawEventType::LeftUp => {
                let delay_ms = if pending_left.take().is_some() {
                    event.timestamp_ms.saturating_sub(prev_time)
                } else {
                    event.timestamp_ms.saturating_sub(prev_time)
                };
                prev_time = event.timestamp_ms;
                actions.push(ScriptAction {
                    action_type: ActionType::Click,
                    delay_ms,
                    x: Some(event.x),
                    y: Some(event.y),
                    absolute: Some(true),
                });
            }
            RawEventType::RightDown => {}
            RawEventType::RightUp => {
                let delay_ms = event.timestamp_ms.saturating_sub(prev_time);
                prev_time = event.timestamp_ms;
                actions.push(ScriptAction {
                    action_type: ActionType::RightClick,
                    delay_ms,
                    x: Some(event.x),
                    y: Some(event.y),
                    absolute: Some(true),
                });
            }
            RawEventType::Move => {
                let delay_ms = event.timestamp_ms.saturating_sub(prev_time);
                prev_time = event.timestamp_ms;

                if let Some(last) = actions.last() {
                    if last.action_type == ActionType::Move
                        && last.x == Some(event.x)
                        && last.y == Some(event.y)
                    {
                        continue;
                    }
                }

                if actions.is_empty() || delay_ms > 0 {
                    actions.push(ScriptAction {
                        action_type: ActionType::Move,
                        delay_ms,
                        x: Some(event.x),
                        y: Some(event.y),
                        absolute: Some(true),
                    });
                }
            }
        }
    }

    if pending_left.is_some() {
        if let Some(last_event) = events.iter().rev().find(|e| matches!(e.event_type, RawEventType::LeftUp | RawEventType::Move)) {
            actions.push(ScriptAction {
                action_type: ActionType::Click,
                delay_ms: 10,
                x: Some(last_event.x),
                y: Some(last_event.y),
                absolute: Some(true),
            });
        }
    }

    if actions.is_empty() {
        actions.push(ScriptAction {
            action_type: ActionType::Delay,
            delay_ms: 1000,
            x: None,
            y: None,
            absolute: None,
        });
    }

    Script {
        name: "录制的脚本".to_string(),
        version: 1,
        description: None,
        created_at: Some(chrono_now()),
        updated_at: None,
        actions,
        loop_config: LoopConfig::default(),
        speed_multiplier: 1.0,
    }
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => format!("unix_{}", d.as_secs()),
        Err(_) => String::from("unknown"),
    }
}
