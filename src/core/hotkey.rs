use std::collections::HashSet;
use std::sync::{mpsc, LazyLock, Mutex};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    PickCoordinate,  // F8
    Start,           // F9
    Stop,            // F10
    RecordStartStop, // F11 — toggle recording
    UserKey { vk_code: u32, key_down: bool },
}

pub struct HotkeyHandle {
    stop_tx: mpsc::Sender<()>,
}

impl Drop for HotkeyHandle {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
    }
}

/// Global pointer to the sender. Set before hook is installed, cleared on cleanup.
static mut HOTKEY_TX_PTR: *mut mpsc::Sender<HotkeyEvent> = std::ptr::null_mut();

/// Global set of user-bound VK codes the hook should forward.
static BOUND_KEYS: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Update the set of user-bound keys the hook should watch for.
pub fn update_bound_keys(keys: HashSet<u32>) {
    if let Ok(mut guard) = BOUND_KEYS.lock() {
        *guard = keys;
    }
}

pub fn start_hotkey_listener(
    event_tx: mpsc::Sender<HotkeyEvent>,
) -> HotkeyHandle {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    // Leak the sender onto the heap so the hook proc can access it
    let tx_box = Box::new(event_tx);
    unsafe {
        HOTKEY_TX_PTR = Box::into_raw(tx_box);
    }

    thread::spawn(move || unsafe {
        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(_) => {
                cleanup_tx();
                return;
            }
        };

        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            Some(HINSTANCE(hinstance.0)),
            0,
        );
        let hook = match hook {
            Ok(h) => h,
            Err(_) => {
                cleanup_tx();
                return;
            }
        };

        // Message pump
        let mut msg = MSG::default();
        loop {
            if stop_rx.try_recv().is_ok() {
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
                thread::sleep(Duration::from_millis(10));
            }
        }

        let _ = UnhookWindowsHookEx(hook);
        cleanup_tx();
    });

    HotkeyHandle { stop_tx }
}

unsafe fn cleanup_tx() {
    if !HOTKEY_TX_PTR.is_null() {
        let _ = Box::from_raw(HOTKEY_TX_PTR);
        HOTKEY_TX_PTR = std::ptr::null_mut();
    }
}

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let tx_ptr = HOTKEY_TX_PTR;
        if !tx_ptr.is_null() {
            if let Some(tx) = tx_ptr.as_ref() {
                let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
                let vk_code = kbd.vkCode;
                let key_down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
                let key_up = wparam.0 as u32 == WM_KEYUP || wparam.0 as u32 == WM_SYSKEYUP;

                // Always handle system hotkeys on key-down
                if key_down {
                    let event = if vk_code == VK_F8.0 as u32 {
                        Some(HotkeyEvent::PickCoordinate)
                    } else if vk_code == VK_F9.0 as u32 {
                        Some(HotkeyEvent::Start)
                    } else if vk_code == VK_F10.0 as u32 {
                        Some(HotkeyEvent::Stop)
                    } else if vk_code == VK_F11.0 as u32 {
                        Some(HotkeyEvent::RecordStartStop)
                    } else {
                        None
                    };

                    if let Some(evt) = event {
                        let _ = tx.send(evt);
                        return CallNextHookEx(None, code, wparam, lparam);
                    }
                }

                // Forward key events to the app.
                // On key-down: always forward (needed for key capture).
                // On key-up: only forward for bound keys (needed for toggle logic).
                let is_bound = {
                    if let Ok(guard) = BOUND_KEYS.lock() {
                        guard.contains(&(vk_code as u32))
                    } else {
                        false
                    }
                };

                if key_down {
                    // Forward all key-down events so key capture works
                    let _ = tx.send(HotkeyEvent::UserKey {
                        vk_code: vk_code as u32,
                        key_down: true,
                    });
                    if is_bound {
                        return LRESULT(1); // Consume bound keys
                    }
                } else if key_up && is_bound {
                    let _ = tx.send(HotkeyEvent::UserKey {
                        vk_code: vk_code as u32,
                        key_down: false,
                    });
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
