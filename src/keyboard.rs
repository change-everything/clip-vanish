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
use rdev::{simulate, EventType, Key};
use clipboard::{ClipboardProvider, ClipboardContext};

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

    /// 安全粘贴文本到当前焦点窗口
    ///
    /// 使用临时剪贴板替换的方式来支持所有字符（包括中文、emoji等）
    ///
    /// # 参数
    /// * `text` - 要粘贴的文本
    /// * `clipboard_ctx` - 剪贴板上下文的引用
    ///
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub fn secure_paste_text(
        text: &str,
        clipboard_ctx: &Arc<Mutex<ClipboardContext>>
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("开始安全粘贴文本，长度: {} 字符", text.chars().count());

        // 设置粘贴进行状态，防止递归调用
        Self::set_paste_in_progress(true);

        // 等待一小段时间确保粘贴快捷键释放
        std::thread::sleep(std::time::Duration::from_millis(20));

        // 1. 备份当前剪贴板内容
        let original_content = {
            let mut ctx = clipboard_ctx.lock().unwrap();
            ctx.get_contents().unwrap_or_default()
        };

        // 2. 临时设置要粘贴的内容到剪贴板
        {
            let mut ctx = clipboard_ctx.lock().unwrap();
            ctx.set_contents(text.to_string())?;
        }

        // 3. 等待一小段时间确保剪贴板内容已更新
        std::thread::sleep(std::time::Duration::from_millis(5));

        // 4. 直接发送粘贴命令而不是模拟按键（避免递归调用）
        // 使用系统API发送粘贴命令
        #[cfg(target_os = "windows")]
        {
            // Windows: 发送 WM_PASTE 消息到当前焦点窗口
            use std::process::Command;
            let _ = Command::new("powershell")
                .args(&["-Command", "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')"])
                .output();
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: 使用 osascript 发送粘贴命令
            use std::process::Command;
            let _ = Command::new("osascript")
                .args(&["-e", "tell application \"System Events\" to keystroke \"v\" using command down"])
                .output();
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: 使用 xdotool 发送粘贴命令
            use std::process::Command;
            let _ = Command::new("xdotool")
                .args(&["key", "ctrl+v"])
                .output();
        }

        // 5. 等待粘贴操作完成（减少延迟）
        std::thread::sleep(std::time::Duration::from_millis(30));

        // 6. 立即恢复原始剪贴板内容
        {
            let mut ctx = clipboard_ctx.lock().unwrap();
            if !original_content.is_empty() {
                ctx.set_contents(original_content)?;
            } else {
                // 如果原来是空的，清空剪贴板
                ctx.set_contents("".to_string())?;
            }
        }

        // 清除粘贴进行状态
        Self::set_paste_in_progress(false);

        info!("安全粘贴完成，剪贴板已恢复");
        Ok(())
    }

    /// 设置粘贴进行状态
    ///
    /// # 参数
    /// * `in_progress` - 是否正在进行粘贴操作
    fn set_paste_in_progress(in_progress: bool) {
        #[cfg(target_os = "windows")]
        {
            use crate::keyboard::platform::windows::GLOBAL_PASTE_IN_PROGRESS;
            if let Some(paste_flag) = GLOBAL_PASTE_IN_PROGRESS.get() {
                *paste_flag.lock().unwrap() = in_progress;
            }
        }

        #[cfg(target_os = "macos")]
        {
            use crate::keyboard::platform::macos::GLOBAL_PASTE_IN_PROGRESS;
            if let Some(paste_flag) = GLOBAL_PASTE_IN_PROGRESS.get() {
                *paste_flag.lock().unwrap() = in_progress;
            }
        }

        #[cfg(target_os = "linux")]
        {
            use crate::keyboard::platform::linux::GLOBAL_PASTE_IN_PROGRESS;
            if let Some(paste_flag) = GLOBAL_PASTE_IN_PROGRESS.get() {
                *paste_flag.lock().unwrap() = in_progress;
            }
        }
    }

    /// 直接输入文本到当前焦点窗口（仅支持ASCII字符）
    ///
    /// # 参数
    /// * `text` - 要输入的文本
    ///
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub fn simulate_text_input(text: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("开始模拟文本输入，长度: {} 字符", text.chars().count());

        // 检查是否包含非ASCII字符
        if !text.is_ascii() {
            return Err("文本包含非ASCII字符，无法直接输入".into());
        }

        // 等待一小段时间确保粘贴快捷键释放
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 逐字符输入文本
        for ch in text.chars() {
            // 将字符转换为键盘事件
            if let Some(key) = char_to_key(ch) {
                // 模拟按键按下
                simulate(&EventType::KeyPress(key))?;
                // 模拟按键释放
                simulate(&EventType::KeyRelease(key))?;

                // 在字符之间添加小延迟，避免输入过快
                std::thread::sleep(std::time::Duration::from_millis(1));
            } else {
                // 对于无法直接映射的字符，返回错误
                return Err(format!("字符 '{}' 无法直接映射到按键", ch).into());
            }
        }

        info!("文本输入完成");
        Ok(())
    }


}

/// 将字符转换为对应的按键
///
/// # 参数
/// * `ch` - 要转换的字符
///
/// # 返回值
/// * `Option<Key>` - 对应的按键，如果无法映射则返回None
fn char_to_key(ch: char) -> Option<Key> {
    match ch {
        // 字母
        'a' | 'A' => Some(Key::KeyA),
        'b' | 'B' => Some(Key::KeyB),
        'c' | 'C' => Some(Key::KeyC),
        'd' | 'D' => Some(Key::KeyD),
        'e' | 'E' => Some(Key::KeyE),
        'f' | 'F' => Some(Key::KeyF),
        'g' | 'G' => Some(Key::KeyG),
        'h' | 'H' => Some(Key::KeyH),
        'i' | 'I' => Some(Key::KeyI),
        'j' | 'J' => Some(Key::KeyJ),
        'k' | 'K' => Some(Key::KeyK),
        'l' | 'L' => Some(Key::KeyL),
        'm' | 'M' => Some(Key::KeyM),
        'n' | 'N' => Some(Key::KeyN),
        'o' | 'O' => Some(Key::KeyO),
        'p' | 'P' => Some(Key::KeyP),
        'q' | 'Q' => Some(Key::KeyQ),
        'r' | 'R' => Some(Key::KeyR),
        's' | 'S' => Some(Key::KeyS),
        't' | 'T' => Some(Key::KeyT),
        'u' | 'U' => Some(Key::KeyU),
        'v' | 'V' => Some(Key::KeyV),
        'w' | 'W' => Some(Key::KeyW),
        'x' | 'X' => Some(Key::KeyX),
        'y' | 'Y' => Some(Key::KeyY),
        'z' | 'Z' => Some(Key::KeyZ),

        // 数字
        '0' => Some(Key::Num0),
        '1' => Some(Key::Num1),
        '2' => Some(Key::Num2),
        '3' => Some(Key::Num3),
        '4' => Some(Key::Num4),
        '5' => Some(Key::Num5),
        '6' => Some(Key::Num6),
        '7' => Some(Key::Num7),
        '8' => Some(Key::Num8),
        '9' => Some(Key::Num9),

        // 常用符号
        ' ' => Some(Key::Space),
        '\n' => Some(Key::Return),
        '\t' => Some(Key::Tab),
        '.' => Some(Key::Dot),
        ',' => Some(Key::Comma),
        ';' => Some(Key::SemiColon),
        '\'' => Some(Key::Quote),
        '[' => Some(Key::LeftBracket),
        ']' => Some(Key::RightBracket),
        '\\' => Some(Key::BackSlash),
        '/' => Some(Key::Slash),
        '=' => Some(Key::Equal),
        '-' => Some(Key::Minus),
        '`' => Some(Key::BackQuote),

        // 其他字符暂不支持直接映射
        _ => None,
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
