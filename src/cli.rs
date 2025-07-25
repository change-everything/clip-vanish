/*!
 * ClipVanish™ 命令行接口模块
 * 
 * 实现命令行界面的核心逻辑，整合各个功能模块
 * 特点：
 * - 统一的命令处理接口
 * - 实时状态显示
 * - 用户友好的输出格式
 * - 错误处理和恢复
 * 
 * 作者: ClipVanish Team
 */

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::signal;
use tokio::time::sleep;
use log::{info, warn, error, debug};
use global_hotkey::{GlobalHotKeyManager, HotKeyState, GlobalHotKeyEvent};
use global_hotkey::hotkey::{HotKey, Modifiers, Code};

use crate::config::Config;
use crate::clipboard::{ClipboardMonitor, ClipboardEvent, ClearReason};
use crate::timer::{DestructTimer, TimerEvent, TimerState};
use crate::memory::SecureMemory;

/// CLI错误类型
#[derive(Debug)]
pub enum CliError {
    /// 剪贴板操作失败
    ClipboardError(String),
    /// 定时器操作失败
    TimerError(String),
    /// 配置错误
    ConfigError(String),
    /// 热键注册失败
    HotkeyError(String),
    /// 服务未运行
    ServiceNotRunning,
    /// 操作被用户取消
    OperationCancelled,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::ClipboardError(msg) => write!(f, "剪贴板错误: {}", msg),
            CliError::TimerError(msg) => write!(f, "定时器错误: {}", msg),
            CliError::ConfigError(msg) => write!(f, "配置错误: {}", msg),
            CliError::HotkeyError(msg) => write!(f, "热键错误: {}", msg),
            CliError::ServiceNotRunning => write!(f, "ClipVanish服务未运行"),
            CliError::OperationCancelled => write!(f, "操作被用户取消"),
        }
    }
}

impl std::error::Error for CliError {}

/// 服务运行状态
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    /// 是否正在运行
    pub is_running: bool,
    /// 启动时间
    pub start_time: Option<Instant>,
    /// 当前倒计时状态
    pub timer_state: TimerState,
    /// 剩余时间
    pub remaining_time: Option<Duration>,
    /// 处理的事件总数
    pub total_events: u64,
    /// 当前加密内容长度
    pub encrypted_content_length: usize,
}

/// CLI处理器
/// 
/// 负责处理所有命令行操作，整合各个功能模块
pub struct CliHandler {
    /// 配置
    config: Config,
    /// 剪贴板监听器
    clipboard_monitor: Option<Arc<ClipboardMonitor>>,
    /// 自毁定时器
    destruct_timer: Option<Arc<Mutex<DestructTimer>>>,
    /// 全局热键管理器
    hotkey_manager: Option<GlobalHotKeyManager>,
    /// 服务状态
    service_status: Arc<Mutex<ServiceStatus>>,
    /// 是否应该停止服务
    should_stop: Arc<Mutex<bool>>,
}

impl CliHandler {
    /// 创建新的CLI处理器
    /// 
    /// # 参数
    /// * `config` - 配置实例
    /// 
    /// # 返回值
    /// * `CliHandler` - CLI处理器实例
    pub fn new(config: Config) -> Self {
        let service_status = ServiceStatus {
            is_running: false,
            start_time: None,
            timer_state: TimerState::Idle,
            remaining_time: None,
            total_events: 0,
            encrypted_content_length: 0,
        };
        
        CliHandler {
            config,
            clipboard_monitor: None,
            destruct_timer: None,
            hotkey_manager: None,
            service_status: Arc::new(Mutex::new(service_status)),
            should_stop: Arc::new(Mutex::new(false)),
        }
    }
    
    /// 启动剪贴板监听服务
    /// 
    /// # 参数
    /// * `timer_duration` - 自毁倒计时（秒）
    /// * `daemon_mode` - 是否以后台模式运行
    /// 
    /// # 返回值
    /// * `Result<(), CliError>` - 操作结果
    pub async fn start_monitoring(&mut self, timer_duration: u64, daemon_mode: bool) -> Result<(), CliError> {
        info!("启动ClipVanish监听服务");
        
        // 检查是否已经在运行
        if self.service_status.lock().unwrap().is_running {
            println!("⚠️  ClipVanish服务已在运行");
            return Ok(());
        }
        
        // 显示启动信息
        if !daemon_mode {
            self.display_startup_info(timer_duration);
        }
        
        // 创建和初始化必要的组件
        let clipboard_monitor = Arc::new(
            ClipboardMonitor::new()
                .map_err(|e| CliError::ClipboardError(e.to_string()))?
        );
        
        let destruct_timer = Arc::new(Mutex::new({
            let mut timer = DestructTimer::new();
            timer.start_service().await
                .map_err(|e| CliError::TimerError(e.to_string()))?;
            timer
        }));
        
        // 设置事件回调
        self.setup_event_callbacks(&clipboard_monitor, &destruct_timer, timer_duration);
        
        // 注册全局热键
        if self.config.hotkeys.enable_global_hotkeys {
            self.register_global_hotkeys(&clipboard_monitor, &destruct_timer)?;
        }
        
        // 更新服务状态
        {
            let mut status = self.service_status.lock().unwrap();
            status.is_running = true;
            status.start_time = Some(Instant::now());
            status.total_events = 0;
        }
        
        // 保存组件引用
        self.clipboard_monitor = Some(clipboard_monitor.clone());
        self.destruct_timer = Some(destruct_timer.clone());
        
        // 启动监听循环（在后台）
        let poll_interval = self.config.get_poll_interval();
        let status_clone = self.service_status.clone();
        let should_stop_clone = self.should_stop.clone();
        
        tokio::spawn(async move {
            let result = clipboard_monitor.start_monitoring(poll_interval).await;
            if let Err(e) = result {
                error!("剪贴板监听任务失败: {}", e);
                // 更新状态为非运行
                if let Ok(mut status) = status_clone.lock() {
                    status.is_running = false;
                }
            }
        });
        
        println!("✅ ClipVanish服务已启动");
        println!("   自毁倒计时: {}秒", timer_duration);
        println!("   紧急销毁热键: {}", self.config.hotkeys.emergency_nuke_key);
        
        if !daemon_mode {
            println!("\n📊 实时状态 (按 Ctrl+C 退出):");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        }
        
        Ok(())
    }
    
    /// 紧急销毁所有数据
    /// 
    /// # 参数
    /// * `force` - 是否强制执行（跳过确认）
    /// 
    /// # 返回值
    /// * `Result<(), CliError>` - 操作结果
    pub async fn emergency_nuke(&self, force: bool) -> Result<(), CliError> {
        if !force {
            println!("⚠️  紧急销毁操作");
            println!("   这将立即清除所有剪贴板数据和内存中的敏感信息");
            print!("   确认执行? (y/N): ");
            
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            
            if !input.trim().to_lowercase().starts_with('y') {
                println!("❌ 操作已取消");
                return Err(CliError::OperationCancelled);
            }
        }
        
        println!("🔥 执行紧急销毁...");
        
        // 如果有剪贴板监听器，执行紧急销毁
        if let Some(monitor) = &self.clipboard_monitor {
            monitor.emergency_nuke()
                .map_err(|e| CliError::ClipboardError(e.to_string()))?;
        }
        
        // 停止定时器
        if let Some(timer) = &self.destruct_timer {
            let timer = timer.lock().unwrap();
            timer.stop_countdown()
                .map_err(|e| CliError::TimerError(e.to_string()))?;
        }
        
        // 执行全局内存清理
        SecureMemory::secure_zero_memory();
        
        println!("✅ 紧急销毁完成");
        println!("   - 剪贴板已清除");
        println!("   - 内存已安全擦除");
        println!("   - 加密密钥已重新生成");
        
        Ok(())
    }
    
    /// 显示服务状态
    /// 
    /// # 返回值
    /// * `Result<(), CliError>` - 操作结果
    pub async fn show_status(&self) -> Result<(), CliError> {
        let status = self.service_status.lock().unwrap().clone();
        
        println!("📊 ClipVanish™ 服务状态");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        if status.is_running {
            println!("🟢 状态: 运行中");
            
            if let Some(start_time) = status.start_time {
                let uptime = start_time.elapsed();
                println!("⏱️  运行时间: {}", Self::format_duration(uptime));
            }
            
            println!("📈 处理事件: {} 次", status.total_events);
            
            if status.encrypted_content_length > 0 {
                println!("🔒 加密内容: {} 字节", status.encrypted_content_length);
            } else {
                println!("📋 剪贴板: 空");
            }
            
            match status.timer_state {
                TimerState::Idle => println!("⏰ 定时器: 待机"),
                TimerState::Running { .. } => {
                    if let Some(remaining) = status.remaining_time {
                        println!("⏰ 倒计时: {}", Self::format_duration(remaining));
                        
                        // 显示进度条
                        if self.config.ui.show_progress {
                            let total_duration = self.config.get_default_countdown_duration();
                            let progress = 1.0 - (remaining.as_secs_f64() / total_duration.as_secs_f64());
                            self.display_progress_bar(progress);
                        }
                    }
                },
                TimerState::Completed => println!("⏰ 定时器: 已完成"),
                TimerState::Cancelled => println!("⏰ 定时器: 已取消"),
                TimerState::Error(ref msg) => println!("⏰ 定时器: 错误 - {}", msg),
            }
        } else {
            println!("🔴 状态: 未运行");
        }
        
        println!();
        println!("🔧 配置信息:");
        println!("   默认倒计时: {}秒", self.config.timer.default_countdown);
        println!("   内存锁定: {}", if self.config.security.enable_memory_locking { "启用" } else { "禁用" });
        println!("   全局热键: {}", if self.config.hotkeys.enable_global_hotkeys { "启用" } else { "禁用" });
        
        Ok(())
    }
    
    /// 停止服务
    /// 
    /// # 返回值
    /// * `Result<(), CliError>` - 操作结果
    pub async fn stop_service(&self) -> Result<(), CliError> {
        let is_running = self.service_status.lock().unwrap().is_running;
        
        if !is_running {
            println!("ℹ️  ClipVanish服务未运行");
            return Ok(());
        }
        
        println!("🛑 正在停止ClipVanish服务...");
        
        // 设置停止标志
        *self.should_stop.lock().unwrap() = true;
        
        // 停止剪贴板监听
        if let Some(monitor) = &self.clipboard_monitor {
            monitor.stop_monitoring();
        }
        
        // 停止定时器
        if let Some(timer) = &self.destruct_timer {
            let timer = timer.lock().unwrap();
            timer.shutdown()
                .map_err(|e| CliError::TimerError(e.to_string()))?;
        }
        
        println!("✅ ClipVanish服务已停止");
        Ok(())
    }
    
    /// 管理配置
    /// 
    /// # 参数
    /// * `reset` - 是否重置为默认配置
    /// 
    /// # 返回值
    /// * `Result<(), CliError>` - 操作结果
    pub async fn manage_config(&mut self, reset: bool) -> Result<(), CliError> {
        if reset {
            self.config.reset_to_default()
                .map_err(|e| CliError::ConfigError(e.to_string()))?;
            println!("✅ 配置已重置为默认值");
        } else {
            self.config.display();
        }
        
        Ok(())
    }
    
    /// 设置事件回调
    fn setup_event_callbacks(
        &self,
        clipboard_monitor: &Arc<ClipboardMonitor>,
        destruct_timer: &Arc<Mutex<DestructTimer>>,
        timer_duration: u64,
    ) {
        let timer_clone = destruct_timer.clone();
        let status_clone = self.service_status.clone();
        let show_progress = self.config.ui.show_progress;
        
        // 剪贴板事件回调
        let clipboard_callback = Arc::new(move |event: ClipboardEvent| {
            match event {
                ClipboardEvent::ContentCopied { length, timestamp, .. } => {
                    println!("🔒 检测到剪贴板内容 ({}字节) - 已加密存储", length);
                    
                    // 启动倒计时
                    if let Ok(timer) = timer_clone.lock() {
                        if let Err(e) = timer.start_countdown(Duration::from_secs(timer_duration)) {
                            error!("启动倒计时失败: {}", e);
                        }
                    }
                    
                    // 更新状态
                    let mut status = status_clone.lock().unwrap();
                    status.total_events += 1;
                    status.encrypted_content_length = length;
                },
                ClipboardEvent::ContentPasted { .. } => {
                    debug!("用户粘贴操作");
                },
                ClipboardEvent::ContentCleared { reason, .. } => {
                    match reason {
                        ClearReason::TimerExpired => println!("🔥 倒计时结束 - 剪贴板已自动清除"),
                        ClearReason::ManualClear => println!("🧹 剪贴板已手动清除"),
                        ClearReason::EmergencyNuke => println!("💥 紧急销毁 - 所有数据已清除"),
                        ClearReason::Shutdown => debug!("程序退出时清除剪贴板"),
                    }
                    
                    // 更新状态
                    let mut status = status_clone.lock().unwrap();
                    status.encrypted_content_length = 0;
                },
            }
        });
        
        clipboard_monitor.set_event_callback(clipboard_callback);
        
        // 定时器事件回调
        let status_clone2 = self.service_status.clone();
        let timer_callback = Arc::new(move |event: TimerEvent| {
            match event {
                TimerEvent::Started { duration, .. } => {
                    println!("⏰ 自毁倒计时已启动: {}", Self::format_duration(duration));
                },
                TimerEvent::Tick { remaining, .. } => {
                    // 更新状态中的剩余时间
                    {
                        let mut status = status_clone2.lock().unwrap();
                        status.remaining_time = Some(remaining);
                    }
                    
                    // 显示倒计时（仅在最后几秒）
                    if remaining.as_secs() <= 10 && remaining.as_secs() > 0 {
                        if show_progress {
                            print!("\r⏰ 倒计时: {}秒 ", remaining.as_secs());
                            use std::io::{self, Write};
                            io::stdout().flush().unwrap();
                        }
                    }
                },
                TimerEvent::Completed { .. } => {
                    println!("\n🔥 倒计时完成 - 执行自动销毁");
                    
                    // 更新状态
                    let mut status = status_clone2.lock().unwrap();
                    status.remaining_time = None;
                },
                TimerEvent::Cancelled { .. } => {
                    debug!("定时器被取消");
                    
                    // 更新状态
                    let mut status = status_clone2.lock().unwrap();
                    status.remaining_time = None;
                },
                TimerEvent::Reset { .. } => {
                    debug!("定时器已重置");
                },
            }
        });
        
        destruct_timer.lock().unwrap().set_callback(timer_callback);
    }
    
    /// 注册全局热键
    fn register_global_hotkeys(
        &mut self,
        clipboard_monitor: &Arc<ClipboardMonitor>,
        _destruct_timer: &Arc<Mutex<DestructTimer>>,
    ) -> Result<(), CliError> {
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| CliError::HotkeyError(e.to_string()))?;
        
        // 注册紧急销毁热键 (Ctrl+Alt+V)
        let emergency_hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::ALT),
            Code::KeyV,
        );
        
        manager.register(emergency_hotkey)
            .map_err(|e| CliError::HotkeyError(e.to_string()))?;
        
        // 启动热键事件处理
        let monitor_clone = Arc::clone(clipboard_monitor);
        tokio::spawn(async move {
            let receiver = GlobalHotKeyEvent::receiver();
            
            loop {
                if let Ok(event) = receiver.try_recv() {
                    if event.state == HotKeyState::Pressed {
                        info!("检测到紧急销毁热键");
                        
                        if let Err(e) = monitor_clone.emergency_nuke() {
                            error!("热键触发的紧急销毁失败: {}", e);
                        } else {
                            println!("\n💥 热键触发紧急销毁 - 所有数据已清除");
                        }
                    }
                }
                
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        
        self.hotkey_manager = Some(manager);
        info!("全局热键已注册: {}", self.config.hotkeys.emergency_nuke_key);
        
        Ok(())
    }
    
    /// 启动状态更新任务
    async fn start_status_update_task(&self) {
        let status_clone = self.service_status.clone();
        let should_stop = self.should_stop.clone();
        let timer_clone = self.destruct_timer.clone();
        
        tokio::spawn(async move {
            while !*should_stop.lock().unwrap() {
                // 更新定时器状态
                if let Some(timer) = &timer_clone {
                    let timer = timer.lock().unwrap();
                    let timer_state = timer.get_state();
                    let remaining_time = timer.get_remaining_time();
                    
                    let mut status = status_clone.lock().unwrap();
                    status.timer_state = timer_state;
                    status.remaining_time = remaining_time;
                }
                
                sleep(Duration::from_millis(500)).await;
            }
        });
    }
    
    /// 启动信号处理任务
    async fn start_signal_handler(&self) {
        let should_stop = self.should_stop.clone();
        
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                    .expect("无法注册SIGINT处理器");
                
                let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("无法注册SIGTERM处理器");
                
                tokio::select! {
                    _ = sigint.recv() => {
                        info!("收到SIGINT信号");
                    },
                    _ = sigterm.recv() => {
                        info!("收到SIGTERM信号");
                    },
                }
            }
            
            #[cfg(windows)]
            {
                match signal::ctrl_c().await {
                    Ok(_) => {
                        info!("收到Ctrl+C信号");
                    },
                    Err(e) => {
                        error!("信号处理错误: {}", e);
                    }
                }
            }
            
            *should_stop.lock().unwrap() = true;
        });
    }
    
    /// 清理服务资源
    async fn cleanup_service(&mut self) -> Result<(), CliError> {
        info!("清理服务资源");
        
        // 如果配置要求，在退出时清除剪贴板
        if self.config.security.auto_clear_on_exit {
            if let Some(monitor) = &self.clipboard_monitor {
                monitor.clear_clipboard(ClearReason::Shutdown)
                    .map_err(|e| CliError::ClipboardError(e.to_string()))?;
            }
        }
        
        // 更新服务状态
        {
            let mut status = self.service_status.lock().unwrap();
            status.is_running = false;
            status.start_time = None;
        }
        
        // 清理组件引用
        self.clipboard_monitor = None;
        self.destruct_timer = None;
        self.hotkey_manager = None;
        
        println!("🧹 资源清理完成");
        Ok(())
    }
    
    /// 显示启动信息
    fn display_startup_info(&self, timer_duration: u64) {
        println!("🚀 启动ClipVanish™监听服务");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔒 加密算法: AES-256-GCM-SIV");
        println!("⏰ 自毁倒计时: {}秒", timer_duration);
        println!("🛡️ 内存保护: {}", if self.config.security.enable_memory_locking { "启用" } else { "禁用" });
        println!("⌨️ 紧急热键: {}", self.config.hotkeys.emergency_nuke_key);
        println!();
    }
    
    /// 显示进度条
    fn display_progress_bar(&self, progress: f64) {
        let width = 30;
        let filled = (progress * width as f64) as usize;
        let empty = width - filled;
        
        print!("\r📊 进度: [{}{}] {:.1}%",
            "█".repeat(filled),
            "░".repeat(empty),
            progress * 100.0
        );
        
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
    }
    
    /// 格式化时间长度
    fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        
        if total_seconds >= 3600 {
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else if total_seconds >= 60 {
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            format!("{}:{:02}", minutes, seconds)
        } else {
            format!("{}秒", total_seconds)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[test]
    fn test_cli_handler_creation() {
        let config = Config::default();
        let handler = CliHandler::new(config);
        
        let status = handler.service_status.lock().unwrap();
        assert!(!status.is_running);
        assert!(status.start_time.is_none());
    }
    
    #[test]
    fn test_duration_formatting() {
        assert_eq!(CliHandler::format_duration(Duration::from_secs(30)), "30秒");
        assert_eq!(CliHandler::format_duration(Duration::from_secs(90)), "1:30");
        assert_eq!(CliHandler::format_duration(Duration::from_secs(3661)), "1:01:01");
    }
    
    #[tokio::test]
    async fn test_config_management() {
        let config = Config::default();
        let mut handler = CliHandler::new(config);
        
        // 测试显示配置（不重置）
        let result = handler.manage_config(false).await;
        assert!(result.is_ok());
    }
}
