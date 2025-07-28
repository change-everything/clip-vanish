/*!
 * macOS 键盘事件监听实现
 *
 * 使用 rdev 库监听全局键盘事件
 * 检测 Cmd+V 粘贴快捷键
 */

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use log::{info, warn, debug, error};
use rdev::{listen, Event, EventType, Key};
use crate::keyboard::{KeyboardEvent, KeyboardEventCallback};

/// 修饰键状态
#[derive(Debug, Clone, Default)]
struct ModifierState {
    cmd_pressed: bool,
    alt_pressed: bool,
    shift_pressed: bool,
}

// 全局状态，用于在回调函数中访问
static GLOBAL_MODIFIER_STATE: OnceLock<Arc<Mutex<ModifierState>>> = OnceLock::new();
static GLOBAL_SHOULD_STOP: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static GLOBAL_EVENT_CALLBACK: OnceLock<Arc<Mutex<Option<KeyboardEventCallback>>>> = OnceLock::new();
pub static GLOBAL_PASTE_IN_PROGRESS: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();

/// 全局键盘事件回调函数
fn global_keyboard_callback(event: Event) {
    // 检查是否应该停止
    if let Some(should_stop) = GLOBAL_SHOULD_STOP.get() {
        if *should_stop.lock().unwrap() {
            return;
        }
    }

    // 检查是否正在进行粘贴操作，避免递归调用
    if let Some(paste_in_progress) = GLOBAL_PASTE_IN_PROGRESS.get() {
        if *paste_in_progress.lock().unwrap() {
            return;
        }
    }

    let modifier_state = GLOBAL_MODIFIER_STATE.get();
    let event_callback = GLOBAL_EVENT_CALLBACK.get();

    if let (Some(state_arc), Some(callback_arc)) = (modifier_state, event_callback) {
        match event.event_type {
            EventType::KeyPress(key) => {
                let mut state = state_arc.lock().unwrap();

                match key {
                    Key::MetaLeft | Key::MetaRight => {
                        state.cmd_pressed = true;
                        debug!("Cmd 键按下");
                    },
                    Key::Alt | Key::AltGr => {
                        state.alt_pressed = true;
                        debug!("Alt 键按下");
                    },
                    Key::ShiftLeft | Key::ShiftRight => {
                        state.shift_pressed = true;
                        debug!("Shift 键按下");
                    },
                    Key::KeyV => {
                        if state.cmd_pressed && !state.alt_pressed {
                            info!("🔍 检测到 Cmd+V 粘贴快捷键");
                            let paste_event = KeyboardEvent::PasteDetected {
                                timestamp: Instant::now(),
                                key_combination: "Cmd+V".to_string(),
                            };

                            if let Some(callback) = &*callback_arc.lock().unwrap() {
                                callback(paste_event);
                            }
                        }
                    },
                    _ => {}
                }
            },
            EventType::KeyRelease(key) => {
                let mut state = state_arc.lock().unwrap();

                match key {
                    Key::MetaLeft | Key::MetaRight => {
                        state.cmd_pressed = false;
                        debug!("Cmd 键释放");
                    },
                    Key::Alt | Key::AltGr => {
                        state.alt_pressed = false;
                        debug!("Alt 键释放");
                    },
                    Key::ShiftLeft | Key::ShiftRight => {
                        state.shift_pressed = false;
                        debug!("Shift 键释放");
                    },
                    _ => {}
                }
            },
            _ => {}
        }
    }
}

/// 启动 macOS 键盘监听
pub async fn start_keyboard_monitoring(
    should_stop: Arc<Mutex<bool>>,
    event_callback: KeyboardEventCallback,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("macOS 键盘监听已启动 (使用 rdev)");

    // 初始化全局状态
    let modifier_state = Arc::new(Mutex::new(ModifierState::default()));
    let callback_wrapper = Arc::new(Mutex::new(Some(event_callback)));
    let paste_in_progress = Arc::new(Mutex::new(false));

    GLOBAL_MODIFIER_STATE.set(modifier_state).map_err(|_| "Failed to set global modifier state")?;
    GLOBAL_SHOULD_STOP.set(should_stop.clone()).map_err(|_| "Failed to set global should stop")?;
    GLOBAL_EVENT_CALLBACK.set(callback_wrapper).map_err(|_| "Failed to set global event callback")?;
    GLOBAL_PASTE_IN_PROGRESS.set(paste_in_progress).map_err(|_| "Failed to set global paste in progress")?;

    // 在单独的线程中启动键盘监听
    let handle = std::thread::spawn(move || {
        // 启动事件监听
        if let Err(e) = listen(global_keyboard_callback) {
            error!("键盘事件监听失败: {:?}", e);
        }
    });

    // 等待停止信号
    while !*should_stop.lock().unwrap() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    info!("macOS 键盘监听已停止");

    // 注意：rdev 的 listen 函数会阻塞线程，这里我们无法优雅地停止它
    // 在实际应用中，可能需要使用其他方法来停止监听

    Ok(())
}


