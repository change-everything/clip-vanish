/*!
 * 键盘监听模块
 * 
 * 包含平台特定的键盘事件监听实现
 */

// 平台特定的模块
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

// 重新导出平台特定的函数
#[cfg(target_os = "macos")]
pub use macos::start_keyboard_monitoring;

#[cfg(target_os = "windows")]
pub use windows::start_keyboard_monitoring;

#[cfg(target_os = "linux")]
pub use linux::start_keyboard_monitoring;
