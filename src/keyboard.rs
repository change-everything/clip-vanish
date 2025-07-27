/*!
 * ClipVanish™ 键盘事件监听模块
 *
 * 实现全局键盘事件监听，特别是粘贴快捷键的检测
 * 支持：
 * - macOS: Cmd+V (使用 CGEventTap)
 * - Windows: Ctrl+V (使用 SetWindowsHookEx)
 * - Linux: Ctrl+V (使用 X11)
 *
 * 作者: ClipVanish Team
 */

use std::sync::{Arc, Mutex};
use std::time::Instant;
use log::{info, warn, debug, error};
use tokio::sync::mpsc;

// 平台特定的模块
mod platform;

/// 键盘事件类型
#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    /// 粘贴操作检测到
    PasteDetected {
        timestamp: Instant,
        /// 使用的快捷键组合
        key_combination: String,
    },
    /// 其他快捷键
    OtherShortcut {
        timestamp: Instant,
        keys: Vec<String>,
    },
}

/// 键盘事件回调函数类型
pub type KeyboardEventCallback = Arc<dyn Fn(KeyboardEvent) + Send + Sync>;

/// 键盘监听器
pub struct KeyboardMonitor {
    /// 事件回调
    event_callback: Arc<Mutex<Option<KeyboardEventCallback>>>,
    /// 是否应该停止监听
    should_stop: Arc<Mutex<bool>>,
    /// 当前按下的修饰键状态
    modifier_state: Arc<Mutex<ModifierState>>,
}

/// 修饰键状态
#[derive(Debug, Clone, Default)]
struct ModifierState {
    cmd_pressed: bool,    // macOS Command键
    ctrl_pressed: bool,   // Ctrl键
    alt_pressed: bool,    // Alt键
    shift_pressed: bool,  // Shift键
}

impl KeyboardMonitor {
    /// 创建新的键盘监听器
    pub fn new() -> Self {
        KeyboardMonitor {
            event_callback: Arc::new(Mutex::new(None)),
            should_stop: Arc::new(Mutex::new(false)),
            modifier_state: Arc::new(Mutex::new(ModifierState::default())),
        }
    }

    /// 设置事件回调
    pub fn set_event_callback(&self, callback: KeyboardEventCallback) {
        let mut cb = self.event_callback.lock().unwrap();
        *cb = Some(callback);
    }

    /// 开始监听键盘事件
    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("开始监听键盘事件");

        // 重置停止标志
        *self.should_stop.lock().unwrap() = false;

        let should_stop = self.should_stop.clone();
        let event_callback = self.event_callback.lock().unwrap().clone();

        // 使用平台特定的键盘监听实现
        #[cfg(target_os = "macos")]
        {
            info!("启动 macOS 键盘监听 (CGEventTap)");
            if let Some(callback) = event_callback {
                platform::macos::start_keyboard_monitoring(should_stop, callback).await?;
            } else {
                warn!("键盘事件回调未设置");
            }
        }

        #[cfg(target_os = "windows")]
        {
            info!("启动 Windows 键盘监听 (SetWindowsHookEx)");
            if let Some(callback) = event_callback {
                platform::windows::start_keyboard_monitoring(should_stop, callback).await?;
            } else {
                warn!("键盘事件回调未设置");
            }
        }

        #[cfg(target_os = "linux")]
        {
            info!("启动 Linux 键盘监听 (X11)");
            if let Some(callback) = event_callback {
                platform::linux::start_keyboard_monitoring(should_stop, callback).await?;
            } else {
                warn!("键盘事件回调未设置");
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            warn!("当前平台不支持键盘监听，使用简化模式");
            while !*should_stop.lock().unwrap() {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }

        info!("键盘监听已停止");
        Ok(())
    }

    /// 停止监听
    pub fn stop_monitoring(&self) {
        info!("请求停止键盘监听");
        *self.should_stop.lock().unwrap() = true;
    }

    /// 手动触发粘贴检测
    /// 这个方法可以被外部调用来模拟粘贴事件
    pub fn trigger_paste_detection(&self, key_combination: &str) {
        debug!("手动触发粘贴检测: {}", key_combination);

        if let Some(callback) = &*self.event_callback.lock().unwrap() {
            let event = KeyboardEvent::PasteDetected {
                timestamp: Instant::now(),
                key_combination: key_combination.to_string(),
            };
            callback(event);
        }
    }


}

impl Drop for KeyboardMonitor {
    fn drop(&mut self) {
        info!("键盘监听器正在销毁");
        self.stop_monitoring();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_keyboard_monitor_creation() {
        let monitor = KeyboardMonitor::new();
        assert!(!*monitor.should_stop.lock().unwrap());
    }

    #[tokio::test]
    async fn test_event_callback() {
        let monitor = KeyboardMonitor::new();
        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();

        let callback = Arc::new(move |_event: KeyboardEvent| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        monitor.set_event_callback(callback);

        // 注意：实际的键盘事件测试需要模拟真实的键盘输入
        // 这里只测试回调设置是否正常
        assert!(monitor.event_callback.lock().unwrap().is_some());
    }
}
