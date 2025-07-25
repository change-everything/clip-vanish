/*!
 * ClipVanish™ 定时器模块
 * 
 * 实现倒计时自毁功能，负责在指定时间后自动销毁剪贴板内容
 * 特点：
 * - 精确的倒计时控制
 * - 可配置的销毁时间
 * - 支持紧急停止和重置
 * - 实时状态监控
 * 
 * 作者: ClipVanish Team
 */

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tokio::sync::mpsc;
use log::{info, warn, debug};

/// 定时器状态
#[derive(Debug, Clone, PartialEq)]
pub enum TimerState {
    /// 未启动
    Idle,
    /// 运行中
    Running {
        /// 开始时间
        start_time: Instant,
        /// 总持续时间
        total_duration: Duration,
    },
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 发生错误
    Error(String),
}

/// 定时器事件类型
#[derive(Debug, Clone)]
pub enum TimerEvent {
    /// 定时器启动
    Started {
        duration: Duration,
        timestamp: Instant,
    },
    /// 倒计时更新（每秒触发）
    Tick {
        remaining: Duration,
        elapsed: Duration,
        timestamp: Instant,
    },
    /// 定时器完成（时间到）
    Completed {
        total_duration: Duration,
        timestamp: Instant,
    },
    /// 定时器被取消
    Cancelled {
        remaining: Duration,
        timestamp: Instant,
    },
    /// 定时器重置
    Reset {
        timestamp: Instant,
    },
}

/// 定时器事件回调函数类型
pub type TimerCallback = Arc<dyn Fn(TimerEvent) + Send + Sync>;

/// 定时器控制命令
#[derive(Debug)]
pub enum TimerCommand {
    /// 启动定时器
    Start(Duration),
    /// 停止定时器
    Stop,
    /// 重置定时器
    Reset,
    /// 获取状态
    GetStatus,
    /// 关闭定时器
    Shutdown,
}

/// 自毁定时器
/// 
/// 负责管理剪贴板内容的自动销毁倒计时
pub struct DestructTimer {
    /// 当前状态
    state: Arc<Mutex<TimerState>>,
    /// 事件回调函数
    callback: Option<TimerCallback>,
    /// 命令发送通道
    command_sender: Option<mpsc::UnboundedSender<TimerCommand>>,
    /// 是否正在运行
    is_running: Arc<Mutex<bool>>,
}

impl DestructTimer {
    /// 创建新的自毁定时器
    /// 
    /// # 返回值
    /// * `DestructTimer` - 定时器实例
    pub fn new() -> Self {
        DestructTimer {
            state: Arc::new(Mutex::new(TimerState::Idle)),
            callback: None,
            command_sender: None,
            is_running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// 设置事件回调函数
    /// 
    /// # 参数
    /// * `callback` - 事件回调函数
    pub fn set_callback(&mut self, callback: TimerCallback) {
        self.callback = Some(callback);
    }
    
    /// 启动定时器服务
    /// 
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub async fn start_service(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if *self.is_running.lock().unwrap() {
            warn!("定时器服务已在运行");
            return Ok(());
        }
        
        info!("启动定时器服务");
        
        // 创建命令通道
        let (tx, mut rx) = mpsc::unbounded_channel::<TimerCommand>();
        self.command_sender = Some(tx);
        
        // 克隆必要的数据用于异步任务
        let state = self.state.clone();
        let callback = self.callback.clone();
        let is_running = self.is_running.clone();
        
        // 标记为运行状态
        *is_running.lock().unwrap() = true;
        
        // 启动定时器服务任务
        tokio::spawn(async move {
            let mut current_timer_handle: Option<tokio::task::JoinHandle<()>> = None;
            
            while let Some(command) = rx.recv().await {
                match command {
                    TimerCommand::Start(duration) => {
                        debug!("收到启动定时器命令，持续时间: {:?}", duration);
                        
                        // 取消现有定时器
                        if let Some(handle) = current_timer_handle.take() {
                            handle.abort();
                        }
                        
                        // 更新状态
                        {
                            let mut state_guard = state.lock().unwrap();
                            *state_guard = TimerState::Running {
                                start_time: Instant::now(),
                                total_duration: duration,
                            };
                        }
                        
                        // 触发启动事件
                        if let Some(ref cb) = callback {
                            let event = TimerEvent::Started {
                                duration,
                                timestamp: Instant::now(),
                            };
                            cb(event);
                        }
                        
                        // 启动新的定时器任务
                        let state_clone = state.clone();
                        let callback_clone = callback.clone();
                        
                        current_timer_handle = Some(tokio::spawn(async move {
                            Self::run_timer(duration, state_clone, callback_clone).await;
                        }));
                    },
                    
                    TimerCommand::Stop => {
                        debug!("收到停止定时器命令");
                        
                        if let Some(handle) = current_timer_handle.take() {
                            handle.abort();
                            
                            // 计算剩余时间
                            let remaining = {
                                let state_guard = state.lock().unwrap();
                                if let TimerState::Running { start_time, total_duration } = *state_guard {
                                    let elapsed = start_time.elapsed();
                                    if elapsed < total_duration {
                                        total_duration - elapsed
                                    } else {
                                        Duration::from_secs(0)
                                    }
                                } else {
                                    Duration::from_secs(0)
                                }
                            };
                            
                            // 更新状态
                            {
                                let mut state_guard = state.lock().unwrap();
                                *state_guard = TimerState::Cancelled;
                            }
                            
                            // 触发取消事件
                            if let Some(ref cb) = callback {
                                let event = TimerEvent::Cancelled {
                                    remaining,
                                    timestamp: Instant::now(),
                                };
                                cb(event);
                            }
                        }
                    },
                    
                    TimerCommand::Reset => {
                        debug!("收到重置定时器命令");
                        
                        // 取消现有定时器
                        if let Some(handle) = current_timer_handle.take() {
                            handle.abort();
                        }
                        
                        // 重置状态
                        {
                            let mut state_guard = state.lock().unwrap();
                            *state_guard = TimerState::Idle;
                        }
                        
                        // 触发重置事件
                        if let Some(ref cb) = callback {
                            let event = TimerEvent::Reset {
                                timestamp: Instant::now(),
                            };
                            cb(event);
                        }
                    },
                    
                    TimerCommand::GetStatus => {
                        debug!("收到获取状态命令");
                        // 状态查询通过get_state方法处理
                    },
                    
                    TimerCommand::Shutdown => {
                        info!("收到关闭定时器服务命令");
                        
                        // 取消现有定时器
                        if let Some(handle) = current_timer_handle.take() {
                            handle.abort();
                        }
                        
                        // 标记为非运行状态
                        *is_running.lock().unwrap() = false;
                        break;
                    },
                }
            }
            
            info!("定时器服务已关闭");
        });
        
        Ok(())
    }
    
    /// 启动倒计时
    /// 
    /// # 参数
    /// * `duration` - 倒计时持续时间
    /// 
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub fn start_countdown(&self, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref sender) = self.command_sender {
            sender.send(TimerCommand::Start(duration))?;
            info!("启动倒计时，持续时间: {:?}", duration);
        } else {
            return Err("定时器服务未启动".into());
        }
        Ok(())
    }
    
    /// 停止倒计时
    /// 
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub fn stop_countdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref sender) = self.command_sender {
            sender.send(TimerCommand::Stop)?;
            info!("停止倒计时");
        } else {
            return Err("定时器服务未启动".into());
        }
        Ok(())
    }
    
    /// 重置定时器
    /// 
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub fn reset(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref sender) = self.command_sender {
            sender.send(TimerCommand::Reset)?;
            info!("重置定时器");
        } else {
            return Err("定时器服务未启动".into());
        }
        Ok(())
    }
    
    /// 获取当前状态
    /// 
    /// # 返回值
    /// * `TimerState` - 当前状态
    pub fn get_state(&self) -> TimerState {
        self.state.lock().unwrap().clone()
    }
    
    /// 获取剩余时间
    /// 
    /// # 返回值
    /// * `Option<Duration>` - 剩余时间，如果定时器未运行则返回None
    pub fn get_remaining_time(&self) -> Option<Duration> {
        let state = self.state.lock().unwrap();
        if let TimerState::Running { start_time, total_duration } = *state {
            let elapsed = start_time.elapsed();
            if elapsed < total_duration {
                Some(total_duration - elapsed)
            } else {
                Some(Duration::from_secs(0))
            }
        } else {
            None
        }
    }
    
    /// 检查定时器是否正在运行
    /// 
    /// # 返回值
    /// * `bool` - 是否正在运行
    pub fn is_running(&self) -> bool {
        matches!(*self.state.lock().unwrap(), TimerState::Running { .. })
    }
    
    /// 关闭定时器服务
    /// 
    /// # 返回值
    /// * `Result<(), Box<dyn std::error::Error>>` - 操作结果
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref sender) = self.command_sender {
            sender.send(TimerCommand::Shutdown)?;
            info!("关闭定时器服务");
        }
        Ok(())
    }
    
    /// 运行定时器的内部方法
    async fn run_timer(
        duration: Duration,
        state: Arc<Mutex<TimerState>>,
        callback: Option<TimerCallback>,
    ) {
        let start_time = Instant::now();
        let total_seconds = duration.as_secs();
        
        // 倒计时循环，每秒更新一次
        for remaining_seconds in (0..=total_seconds).rev() {
            let remaining = Duration::from_secs(remaining_seconds);
            let elapsed = start_time.elapsed();
            
            // 检查是否被取消
            {
                let state_guard = state.lock().unwrap();
                if !matches!(*state_guard, TimerState::Running { .. }) {
                    debug!("定时器被取消，退出倒计时循环");
                    return;
                }
            }
            
            // 触发tick事件
            if let Some(ref cb) = callback {
                let event = TimerEvent::Tick {
                    remaining,
                    elapsed,
                    timestamp: Instant::now(),
                };
                cb(event);
            }
            
            // 如果还有剩余时间，等待1秒
            if remaining_seconds > 0 {
                if let Err(_) = timeout(Duration::from_secs(1), sleep(Duration::from_secs(1))).await {
                    debug!("定时器等待被中断");
                    return;
                }
            }
        }
        
        // 定时器完成
        {
            let mut state_guard = state.lock().unwrap();
            *state_guard = TimerState::Completed;
        }
        
        // 触发完成事件
        if let Some(ref cb) = callback {
            let event = TimerEvent::Completed {
                total_duration: duration,
                timestamp: Instant::now(),
            };
            cb(event);
        }
        
        info!("定时器倒计时完成，持续时间: {:?}", duration);
    }
    
    /// 格式化剩余时间为可读字符串
    /// 
    /// # 参数
    /// * `duration` - 时间长度
    /// 
    /// # 返回值
    /// * `String` - 格式化后的时间字符串
    pub fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        
        if total_seconds >= 3600 {
            // 超过1小时，显示时:分:秒
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else if total_seconds >= 60 {
            // 超过1分钟，显示分:秒
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            format!("{}:{:02}", minutes, seconds)
        } else {
            // 少于1分钟，只显示秒
            format!("{}s", total_seconds)
        }
    }
}

/// 实现Drop trait确保资源清理
impl Drop for DestructTimer {
    fn drop(&mut self) {
        debug!("自毁定时器正在销毁");
        
        // 尝试关闭服务
        if let Err(e) = self.shutdown() {
            warn!("定时器销毁时关闭服务失败: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_timer_creation() {
        let timer = DestructTimer::new();
        assert_eq!(timer.get_state(), TimerState::Idle);
        assert!(!timer.is_running());
    }
    
    #[tokio::test]
    async fn test_timer_start_and_complete() {
        let mut timer = DestructTimer::new();
        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();
        
        let callback = Arc::new(move |event: TimerEvent| {
            match event {
                TimerEvent::Started { .. } => {
                    event_count_clone.fetch_add(1, Ordering::SeqCst);
                },
                TimerEvent::Completed { .. } => {
                    event_count_clone.fetch_add(10, Ordering::SeqCst);
                },
                _ => {},
            }
        });
        
        timer.set_callback(callback);
        timer.start_service().await.unwrap();
        
        // 启动短时间的倒计时
        timer.start_countdown(Duration::from_secs(1)).unwrap();
        assert!(timer.is_running());
        
        // 等待倒计时完成
        sleep(Duration::from_millis(1200)).await;
        
        // 检查事件是否被触发
        let final_count = event_count.load(Ordering::SeqCst);
        assert!(final_count >= 11); // 至少包含Started(1) + Completed(10)
        
        timer.shutdown().unwrap();
    }
    
    #[tokio::test]
    async fn test_timer_cancellation() {
        let mut timer = DestructTimer::new();
        let cancelled = Arc::new(AtomicUsize::new(0));
        let cancelled_clone = cancelled.clone();
        
        let callback = Arc::new(move |event: TimerEvent| {
            if let TimerEvent::Cancelled { .. } = event {
                cancelled_clone.fetch_add(1, Ordering::SeqCst);
            }
        });
        
        timer.set_callback(callback);
        timer.start_service().await.unwrap();
        
        // 启动较长时间的倒计时
        timer.start_countdown(Duration::from_secs(10)).unwrap();
        assert!(timer.is_running());
        
        // 等待一小段时间后取消
        sleep(Duration::from_millis(100)).await;
        timer.stop_countdown().unwrap();
        
        // 等待取消操作完成
        sleep(Duration::from_millis(100)).await;
        
        // 检查取消事件是否被触发
        assert_eq!(cancelled.load(Ordering::SeqCst), 1);
        assert_eq!(timer.get_state(), TimerState::Cancelled);
        
        timer.shutdown().unwrap();
    }
    
    #[test]
    fn test_duration_formatting() {
        assert_eq!(DestructTimer::format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(DestructTimer::format_duration(Duration::from_secs(90)), "1:30");
        assert_eq!(DestructTimer::format_duration(Duration::from_secs(3661)), "1:01:01");
    }
    
    #[tokio::test]
    async fn test_remaining_time() {
        let mut timer = DestructTimer::new();
        timer.start_service().await.unwrap();
        
        // 启动5秒倒计时
        timer.start_countdown(Duration::from_secs(5)).unwrap();
        
        // 等待1秒
        sleep(Duration::from_millis(1000)).await;
        
        // 检查剩余时间
        if let Some(remaining) = timer.get_remaining_time() {
            assert!(remaining.as_secs() <= 4);
            assert!(remaining.as_secs() >= 3);
        } else {
            panic!("应该有剩余时间");
        }
        
        timer.shutdown().unwrap();
    }
}
