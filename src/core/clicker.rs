use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

#[derive(Debug, Clone)]
pub enum ClickerEvent {
    Tick { clicks: u64 },
    Done { clicks: u64 },
}

// ── Cursor helpers ──

pub fn move_cursor(x: u32, y: u32) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetCursorPos(x as i32, y as i32);
    }
}

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    let mut pt = windows::Win32::Foundation::POINT::default();
    unsafe {
        match windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt) {
            Ok(()) => Some((pt.x, pt.y)),
            Err(_) => None,
        }
    }
}

// ── Single click helpers ──

pub fn single_click() {
    unsafe {
        mouse_event(MOUSEEVENTF_LEFTDOWN | MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
    }
}

pub fn right_click() {
    unsafe {
        mouse_event(
            MOUSEEVENTF_RIGHTDOWN | MOUSEEVENTF_RIGHTUP,
            0,
            0,
            0,
            0,
        );
    }
}

pub fn double_click() {
    single_click();
    thread::sleep(Duration::from_millis(1));
    single_click();
}

// ── Absolute coordinate clicking (non-cursor-stealing) ──

/// Fire a click at absolute screen coordinates via SendInput.
/// Uses MOUSEEVENTF_ABSOLUTE to target the position without permanently moving the cursor.
pub fn send_click_at(x: i32, y: i32) {
    let (screen_w, screen_h) = crate::utils::coordinate::get_screen_size();
    let norm_x = ((x as f64 / screen_w as f64) * 65535.0) as u32;
    let norm_y = ((y as f64 / screen_h as f64) * 65535.0) as u32;

    let down = MOUSEINPUT {
        dx: norm_x as i32,
        dy: norm_y as i32,
        mouseData: 0,
        dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE | MOUSEEVENTF_LEFTDOWN,
        time: 0,
        dwExtraInfo: 0,
    };
    let up = MOUSEINPUT {
        dx: norm_x as i32,
        dy: norm_y as i32,
        mouseData: 0,
        dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE | MOUSEEVENTF_LEFTUP,
        time: 0,
        dwExtraInfo: 0,
    };

    let mut inputs = [
        INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: down } },
        INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: up } },
    ];

    unsafe {
        SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

// ── Repeat-mode clicker (non-cursor-stealing) ──

pub struct NoStealClicker {
    stop_flag: Arc<AtomicBool>,
}

impl NoStealClicker {
    pub fn start(x: i32, y: i32, interval_ms: u64, event_tx: Option<mpsc::Sender<ClickerEvent>>) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        thread::spawn(move || {
            let interval = Duration::from_millis(interval_ms.max(1));
            let mut clicks_done: u64 = 0;

            loop {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                send_click_at(x, y);
                clicks_done += 1;

                if let Some(ref tx) = event_tx {
                    let _ = tx.send(ClickerEvent::Tick {
                        clicks: clicks_done,
                    });
                }

                let sleep_start = Instant::now();
                loop {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    let elapsed = sleep_start.elapsed();
                    if elapsed >= interval {
                        break;
                    }
                    let remaining = interval.saturating_sub(elapsed);
                    if remaining > Duration::from_millis(1) {
                        thread::sleep(Duration::from_millis(1));
                    } else {
                        std::hint::spin_loop();
                    }
                }
            }

            if let Some(ref tx) = event_tx {
                let _ = tx.send(ClickerEvent::Done {
                    clicks: clicks_done,
                });
            }
        });

        NoStealClicker { stop_flag }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl Drop for NoStealClicker {
    fn drop(&mut self) {
        self.stop();
    }
}
