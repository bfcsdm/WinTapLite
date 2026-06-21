use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use crate::core::clicker;
use crate::script::types::*;

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// Progress update: (iteration, action_index, total_actions)
    Progress {
        iteration: u32,
        action_index: usize,
        total_actions: usize,
    },
    /// Playback finished
    Done,
}

pub struct Player {
    stop_flag: Arc<AtomicBool>,
}

impl Player {
    /// Start playing a script in a background thread.
    /// If `event_tx` is provided, progress events are sent during playback.
    pub fn start(script: Script, event_tx: Option<mpsc::Sender<PlayerEvent>>) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        thread::spawn(move || {
            play_script_with_loops(&script, &stop_clone, event_tx);
        });

        Player { stop_flag }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        !self.stop_flag.load(Ordering::Relaxed)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

fn play_script_with_loops(
    script: &Script,
    stop_flag: &AtomicBool,
    event_tx: Option<mpsc::Sender<PlayerEvent>>,
) {
    let speed = script.speed_multiplier.max(0.1).min(10.0);
    let max_iterations = if script.loop_config.enabled {
        if script.loop_config.count == 0 {
            u32::MAX
        } else {
            script.loop_config.count
        }
    } else {
        1
    };

    let total_actions = script.actions.len();

    for iteration in 0..max_iterations {
        for (action_idx, action) in script.actions.iter().enumerate() {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }

            // Report progress
            if let Some(ref tx) = event_tx {
                let _ = tx.send(PlayerEvent::Progress {
                    iteration,
                    action_index: action_idx,
                    total_actions,
                });
            }

            // Apply delay before the action (adjusted by speed)
            let delay = Duration::from_millis((action.delay_ms as f64 / speed) as u64);
            if delay > Duration::ZERO {
                sleep_precise(delay, stop_flag);
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
            }

            // Move cursor if coordinates are provided and action needs it
            if action.action_type.needs_coordinate() {
                if let (Some(x), Some(y)) = (action.x, action.y) {
                    if action.absolute.unwrap_or(true) {
                        clicker::move_cursor(x, y);
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            }

            // Execute the action
            match action.action_type {
                ActionType::Move => {}
                ActionType::Click => clicker::single_click(),
                ActionType::DoubleClick => clicker::double_click(),
                ActionType::RightClick => clicker::right_click(),
                ActionType::Delay => {}
            }
        }

        if stop_flag.load(Ordering::Relaxed) {
            return;
        }
    }

    // Playback done
    if let Some(ref tx) = event_tx {
        let _ = tx.send(PlayerEvent::Done);
    }
}

fn sleep_precise(duration: Duration, stop_flag: &AtomicBool) {
    let start = Instant::now();
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        let elapsed = start.elapsed();
        if elapsed >= duration {
            break;
        }
        let remaining = duration.saturating_sub(elapsed);
        if remaining > Duration::from_millis(1) {
            thread::sleep(Duration::from_millis(1));
        } else {
            std::hint::spin_loop();
        }
    }
}
