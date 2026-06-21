use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone)]
pub struct RawMouseEvent {
    pub timestamp_ms: u64,
    pub event_type: RawEventType,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawEventType {
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    Move,
}

static mut RECORDER_STATE: *const RecorderState = std::ptr::null();
static mut RECORDER_BASE_TIME: Option<Instant> = None;

struct RecorderState {
    active: AtomicBool,
    events: Mutex<Vec<RawMouseEvent>>,
}

pub struct Recorder {
    state: Arc<RecorderState>,
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            state: Arc::new(RecorderState {
                active: AtomicBool::new(false),
                events: Mutex::new(Vec::new()),
            }),
            stop_tx: None,
        }
    }

    pub fn start(&mut self) {
        if self.is_recording() {
            return;
        }

        self.state.active.store(true, Ordering::SeqCst);
        if let Ok(mut events) = self.state.events.lock() {
            events.clear();
        }

        let state_clone = Arc::clone(&self.state);
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        self.stop_tx = Some(stop_tx);

        thread::spawn(move || unsafe {
            let state_ref: &RecorderState = &state_clone;
            RECORDER_STATE = state_ref as *const RecorderState;
            RECORDER_BASE_TIME = Some(Instant::now());

            let hmodule = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(_) => {
                    cleanup_recorder_state(&state_clone);
                    return;
                }
            };

            let hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_hook_proc),
                Some(HINSTANCE(hmodule.0)),
                0,
            );

            let hook = match hook {
                Ok(h) => h,
                Err(_) => {
                    cleanup_recorder_state(&state_clone);
                    return;
                }
            };

            let mut msg = MSG::default();
            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                if !state_clone.active.load(Ordering::SeqCst) {
                    break;
                }

                let ret = PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE);
                if ret.as_bool() {
                    if msg.message == WM_QUIT {
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    thread::sleep(std::time::Duration::from_millis(5));
                }
            }

            let _ = UnhookWindowsHookEx(hook);
            cleanup_recorder_state(&state_clone);
        });
    }

    pub fn stop(&mut self) -> Vec<RawMouseEvent> {
        self.state.active.store(false, Ordering::SeqCst);
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Ok(events) = self.state.events.lock() {
            events.clone()
        } else {
            Vec::new()
        }
    }

    pub fn is_recording(&self) -> bool {
        self.state.active.load(Ordering::SeqCst)
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe fn cleanup_recorder_state(state: &RecorderState) {
    state.active.store(false, Ordering::SeqCst);
    RECORDER_STATE = std::ptr::null();
    RECORDER_BASE_TIME = None;
}

unsafe extern "system" fn mouse_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let state_ptr = RECORDER_STATE;
        if !state_ptr.is_null() {
            let state = &*state_ptr;
            if state.active.load(Ordering::SeqCst) {
                if let Some(base) = RECORDER_BASE_TIME {
                    let msll = *(lparam.0 as *const MSLLHOOKSTRUCT);
                    let timestamp_ms = base.elapsed().as_millis() as u64;

                    let event_type = match wparam.0 as u32 {
                        WM_LBUTTONDOWN => Some(RawEventType::LeftDown),
                        WM_LBUTTONUP => Some(RawEventType::LeftUp),
                        WM_RBUTTONDOWN => Some(RawEventType::RightDown),
                        WM_RBUTTONUP => Some(RawEventType::RightUp),
                        WM_MOUSEMOVE => Some(RawEventType::Move),
                        _ => None,
                    };

                    if let Some(et) = event_type {
                        let event = RawMouseEvent {
                            timestamp_ms,
                            event_type: et,
                            x: msll.pt.x as u32,
                            y: msll.pt.y as u32,
                        };
                        if let Ok(mut events) = state.events.lock() {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
