/*!
 * 键盘事件监听测试程序
 * 
 * 用于测试 Ctrl+V/Cmd+V 快捷键检测功能
 */

use std::sync::{Arc, Mutex};
use std::time::Instant;
use log::{info, debug};

// 模拟 ClipVanish 的键盘事件类型
#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    PasteDetected {
        timestamp: Instant,
        key_combination: String,
    },
    OtherShortcut {
        timestamp: Instant,
        keys: Vec<String>,
    },
}

pub type KeyboardEventCallback = Arc<dyn Fn(KeyboardEvent) + Send + Sync>;

#[tokio::main]
async fn main() {
    // 初始化日志
    env_logger::init();
    
    println!("🔍 键盘事件监听测试");
    println!("请按 Ctrl+V (Windows/Linux) 或 Cmd+V (macOS) 来测试粘贴检测");
    println!("按 Ctrl+C 退出程序");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 创建事件回调
    let event_callback = Arc::new(|event: KeyboardEvent| {
        match event {
            KeyboardEvent::PasteDetected { timestamp: _, key_combination } => {
                println!("✅ 检测到粘贴快捷键: {}", key_combination);
                println!("   时间: {:?}", Instant::now());
            },
            KeyboardEvent::OtherShortcut { keys, .. } => {
                debug!("检测到其他快捷键: {:?}", keys);
            },
        }
    });

    // 创建停止标志
    let should_stop = Arc::new(Mutex::new(false));

    // 启动键盘监听
    #[cfg(target_os = "windows")]
    {
        use clipvanish::keyboard::platform::windows::start_keyboard_monitoring;
        info!("启动 Windows 键盘监听");
        if let Err(e) = start_keyboard_monitoring(should_stop.clone(), event_callback).await {
            eprintln!("键盘监听失败: {}", e);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use clipvanish::keyboard::platform::macos::start_keyboard_monitoring;
        info!("启动 macOS 键盘监听");
        if let Err(e) = start_keyboard_monitoring(should_stop.clone(), event_callback).await {
            eprintln!("键盘监听失败: {}", e);
        }
    }

    #[cfg(target_os = "linux")]
    {
        use clipvanish::keyboard::platform::linux::start_keyboard_monitoring;
        info!("启动 Linux 键盘监听");
        if let Err(e) = start_keyboard_monitoring(should_stop.clone(), event_callback).await {
            eprintln!("键盘监听失败: {}", e);
        }
    }

    println!("测试完成");
}
