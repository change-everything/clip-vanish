/*!
 * ClipVanish™ 剪贴板监听模块
 * 
 * 实现跨平台剪贴板监听和操作功能
 * 特点：
 * - 跨平台支持（Windows/macOS/Linux）
 * - 实时监听剪贴板变化
 * - 安全的剪贴板内容读取和清除
 * - 支持文本、图片等多种格式（MVP仅支持文本）
 * 
 * 作者: ClipVanish Team
 */

use clipboard::{ClipboardProvider, ClipboardContext};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use log::{info, warn, error, debug};
use crate::config::Config;
use crate::crypto::{CryptoEngine, EncryptedData, CryptoError};
use crate::memory::SecureMemory;

/// 剪贴板操作错误类型
#[derive(Debug)]
pub enum ClipboardError {
    /// 剪贴板访问失败
    AccessFailed(String),
    /// 内容读取失败
    ReadFailed(String),
    /// 内容写入失败
    WriteFailed(String),
    /// 加密操作失败
    CryptoError(CryptoError),
    /// 监听器未初始化
    NotInitialized,
    /// 监听器已停止
    Stopped,
}

impl From<CryptoError> for ClipboardError {
    fn from(error: CryptoError) -> Self {
        ClipboardError::CryptoError(error)
    }
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::AccessFailed(msg) => write!(f, "剪贴板访问失败: {}", msg),
            ClipboardError::ReadFailed(msg) => write!(f, "剪贴板读取失败: {}", msg),
            ClipboardError::WriteFailed(msg) => write!(f, "剪贴板写入失败: {}", msg),
            ClipboardError::CryptoError(err) => write!(f, "加密操作失败: {}", err),
            ClipboardError::NotInitialized => write!(f, "剪贴板监听器未初始化"),
            ClipboardError::Stopped => write!(f, "剪贴板监听器已停止"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// 剪贴板事件类型
#[derive(Debug, Clone)]
pub enum ClipboardEvent {
    /// 内容复制事件
    ContentCopied {
        /// 内容长度（字节）
        length: usize,
        /// 内容类型
        content_type: ContentType,
        /// 时间戳
        timestamp: Instant,
    },
    /// 内容粘贴事件
    ContentPasted {
        /// 时间戳
        timestamp: Instant,
    },
    /// 内容清除事件
    ContentCleared {
        /// 清除原因
        reason: ClearReason,
        /// 时间戳
        timestamp: Instant,
    },
}

/// 剪贴板内容类型
#[derive(Debug, Clone)]
pub enum ContentType {
    /// 文本内容
    Text,
    /// 图片内容（暂未实现）
    Image,
    /// 文件路径（暂未实现）
    Files,
    /// 未知类型
    Unknown,
}

/// 清除原因
#[derive(Debug, Clone)]
pub enum ClearReason {
    /// 倒计时到期
    TimerExpired,
    /// 用户手动清除
    ManualClear,
    /// 紧急销毁
    EmergencyNuke,
    /// 程序退出
    Shutdown,
}

/// 剪贴板操作类型
#[derive(Debug, Clone)]
pub enum ClipboardOperation {
    /// 复制
    Copy,
    /// 粘贴
    Paste,
    /// 清除（带原因）
    Clear(ClearReason),
}

/// 剪贴板事件回调函数类型
pub type EventCallback = Arc<dyn Fn(ClipboardEvent) + Send + Sync>;

/// 剪贴板历史记录项
#[derive(Debug, Clone)]
pub struct ClipboardHistoryItem {
    /// 操作时间
    pub timestamp: Instant,
    /// 内容长度（字节）
    pub length: usize,
    /// 内容类型
    pub content_type: ContentType,
    /// 操作类型
    pub operation: ClipboardOperation,
    /// 明文内容（如果是复制操作）
    pub content: Option<String>,
}

/// 剪贴板监听器状态
#[derive(Debug, Clone)]
pub struct ClipboardState {
    /// 是否正在运行
    pub is_running: bool,
    /// 最后一次内容变化时间
    pub last_change: Option<Instant>,
    /// 当前加密内容长度
    pub encrypted_content_length: usize,
    /// 监听开始时间
    pub start_time: Instant,
    /// 处理的事件总数
    pub total_events: u64,
}

/// 剪贴板监听器
/// 
/// 负责监听剪贴板变化，加密存储内容，并在适当时机清除
pub struct ClipboardMonitor {
    /// 剪贴板上下文
    clipboard_ctx: Arc<Mutex<ClipboardContext>>,
    /// 加密引擎
    crypto_engine: Arc<Mutex<CryptoEngine>>,
    /// 当前加密的剪贴板内容
    encrypted_content: Arc<Mutex<Option<EncryptedData>>>,
    /// 事件回调函数
    event_callback: Arc<Mutex<Option<EventCallback>>>,
    /// 是否应该停止监听
    should_stop: Arc<Mutex<bool>>,
    /// 上次剪贴板内容的哈希（用于检测变化）
    last_content_hash: Arc<Mutex<u64>>,
    /// 监听器状态
    state: Arc<Mutex<ClipboardState>>,
    /// 历史记录
    history: Arc<Mutex<Vec<ClipboardHistoryItem>>>,
    /// 配置
    config: Arc<Config>,
}

impl ClipboardMonitor {
    /// 创建新的剪贴板监听器
    /// 
    /// # 返回值
    /// * `Result<ClipboardMonitor, ClipboardError>` - 成功返回监听器实例
    pub fn new(config: Config) -> Result<Self, ClipboardError> {
        let clipboard_ctx = ClipboardContext::new()
            .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;
        
        let crypto_engine = CryptoEngine::new()
            .map_err(ClipboardError::CryptoError)?;
        
        let state = ClipboardState {
            is_running: false,
            last_change: None,
            encrypted_content_length: 0,
            start_time: Instant::now(),
            total_events: 0,
        };
        
        Ok(ClipboardMonitor {
            clipboard_ctx: Arc::new(Mutex::new(clipboard_ctx)),
            crypto_engine: Arc::new(Mutex::new(crypto_engine)),
            encrypted_content: Arc::new(Mutex::new(None)),
            event_callback: Arc::new(Mutex::new(None)),
            should_stop: Arc::new(Mutex::new(false)),
            last_content_hash: Arc::new(Mutex::new(0)),
            state: Arc::new(Mutex::new(state)),
            history: Arc::new(Mutex::new(Vec::new())),
            config: Arc::new(config),
        })
    }
    
    /// 设置事件回调函数
    /// 
    /// # 参数
    /// * `callback` - 事件回调函数
    pub fn set_event_callback(&self, callback: EventCallback) {
        let mut event_callback = self.event_callback.lock().unwrap();
        *event_callback = Some(callback);
    }
    
    /// 开始监听剪贴板
    /// 
    /// # 参数
    /// * `poll_interval` - 轮询间隔（毫秒）
    /// 
    /// # 返回值
    /// * `Result<(), ClipboardError>` - 操作结果
    pub async fn start_monitoring(&self, poll_interval: Duration) -> Result<(), ClipboardError> {
        info!("开始监听剪贴板变化，轮询间隔: {:?}", poll_interval);
        
        // 重置停止标志
        *self.should_stop.lock().unwrap() = false;
        
        // 主监听循环
        while !*self.should_stop.lock().unwrap() {
            if let Err(e) = self.check_clipboard_change().await {
                warn!("剪贴板检查失败: {}", e);
            }
            
            sleep(poll_interval).await;
        }
        
        info!("剪贴板监听已停止");
        Ok(())
    }
    
    /// 停止监听
    pub fn stop_monitoring(&self) {
        info!("请求停止剪贴板监听");
        *self.should_stop.lock().unwrap() = true;
    }
    
    /// 检查剪贴板内容变化
    async fn check_clipboard_change(&self) -> Result<(), ClipboardError> {
        let current_content = self.read_clipboard_content()?;
        
        if let Some(content) = current_content {
            let content_hash = self.calculate_content_hash(&content);
            let last_hash = *self.last_content_hash.lock().unwrap();
            
            // 检查内容是否发生变化
            if content_hash != last_hash {
                debug!("检测到剪贴板内容变化");
                
                // 显示复制的内容预览（最多显示50个字符）
                let preview = if content.len() > 50 {
                    format!("{}...", &content[..47])
                } else {
                    content.clone()
                };
                println!("📋 检测到复制内容: \"{}\"", preview);
                
                // 加密新内容
                let encrypted = {
                    let crypto = self.crypto_engine.lock().unwrap();
                    crypto.encrypt(content.as_bytes())?
                };
                
                // 存储加密内容
                {
                    let mut encrypted_content = self.encrypted_content.lock().unwrap();
                    *encrypted_content = Some(encrypted.clone());
                }
                
                // 更新状态
                {
                    let mut state = self.state.lock().unwrap();
                    state.last_change = Some(Instant::now());
                    state.encrypted_content_length = encrypted.total_length();
                    state.total_events += 1;
                }
                
                // 更新内容哈希
                *self.last_content_hash.lock().unwrap() = content_hash;
                
                // 添加历史记录
                self.add_history(ClipboardHistoryItem {
                    timestamp: Instant::now(),
                    length: content.len(),
                    content_type: ContentType::Text,
                    operation: ClipboardOperation::Copy,
                    content: Some(content.clone()), // 存储明文内容
                });
                
                // 触发事件回调
                if let Some(callback) = &*self.event_callback.lock().unwrap() {
                    let event = ClipboardEvent::ContentCopied {
                        length: content.len(),
                        content_type: ContentType::Text,
                        timestamp: Instant::now(),
                    };
                    callback(event);
                }
                
                info!("剪贴板内容已加密存储，长度: {} 字节", content.len());
            }
        }
        
        Ok(())
    }
    
    /// 读取剪贴板内容
    fn read_clipboard_content(&self) -> Result<Option<String>, ClipboardError> {
        let mut ctx = self.clipboard_ctx.lock().unwrap();
        
        match ctx.get_contents() {
            Ok(content) => {
                if content.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(content))
                }
            },
            Err(e) => {
                // 剪贴板为空或无法访问时不报错，这是正常情况
                debug!("剪贴板读取: {}", e);
                Ok(None)
            }
        }
    }
    
    /// 删除指定的历史记录
    pub fn remove_history_item(&self, content: &str) {
        let mut history = self.history.lock().unwrap();
        if let Some(index) = history.iter().position(|item| {
            item.content.as_ref().map_or(false, |c| c == content)
        }) {
            history.remove(index);
            debug!("已删除历史记录项");
        }
    }

    /// 清除超时的历史记录
    pub fn clear_expired_history(&self) {
        let mut history = self.history.lock().unwrap();
        history.retain(|item| {
            if let Some(content) = &item.content {
                // 在倒计时结束时删除对应记录
                if item.timestamp.elapsed() >= Duration::from_secs(30) {
                    debug!("删除已过期的历史记录: {}", content);
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });
    }

    /// 处理粘贴操作
    pub fn handle_paste(&self, content: &str) -> Result<(), ClipboardError> {
        if let Some(decrypted) = self.get_decrypted_content()? {
            // 如果配置了粘贴即销毁，则删除对应记录
            if self.config.security.destroy_on_paste {
                self.remove_history_item(&decrypted);
                // 清除剪贴板内容
                self.clear_clipboard(ClearReason::ManualClear)?;
            }
        }
        Ok(())
    }

    /// 获取解密后的剪贴板内容并处理粘贴操作
    /// 
    /// # 返回值
    /// * `Result<Option<String>, ClipboardError>` - 解密后的内容
    pub fn get_decrypted_content(&self) -> Result<Option<String>, ClipboardError> {
        let encrypted_content = self.encrypted_content.lock().unwrap();
        
        if let Some(encrypted) = encrypted_content.as_ref() {
            let crypto = self.crypto_engine.lock().unwrap();
            let decrypted_bytes = crypto.decrypt(encrypted)?;
            let content = String::from_utf8_lossy(&decrypted_bytes).to_string();
            
            // 记录粘贴操作日志
            info!("检测到粘贴操作，内容长度: {} 字节", content.len());
            debug!("粘贴的内容预览: \"{}\"", if content.len() > 50 {
                format!("{}...", &content[..47])
            } else {
                content.clone()
            });

            // 触发粘贴事件
            if let Some(callback) = &*self.event_callback.lock().unwrap() {
                let event = ClipboardEvent::ContentPasted {
                    timestamp: Instant::now(),
                };
                callback(event);
            }
            
            // 如果配置了粘贴即销毁，则删除对应记录并清除剪贴板
            if self.config.security.destroy_on_paste {
                // 删除对应的历史记录
                self.remove_history_item(&content);
                
                // 异步清除剪贴板
                let monitor = self.clone();
                info!("粘贴完成，正在清除剪贴板内容");
                tokio::spawn(async move {
                    if let Err(e) = monitor.clear_clipboard(ClearReason::ManualClear) {
                        error!("粘贴后清除剪贴板失败: {}", e);
                    } else {
                        info!("剪贴板内容已成功清除");
                    }
                });
            }
            
            Ok(Some(content))
        } else {
            debug!("剪贴板中没有加密内容");
            Ok(None)
        }
    }
    
    /// 清除剪贴板内容
    /// 
    /// # 参数
    /// * `reason` - 清除原因
    /// 
    /// # 返回值
    /// * `Result<(), ClipboardError>` - 操作结果
    pub fn clear_clipboard(&self, reason: ClearReason) -> Result<(), ClipboardError> {
        info!("清除剪贴板内容，原因: {:?}", reason);
        
        // 清除系统剪贴板
        {
            let mut ctx = self.clipboard_ctx.lock().unwrap();
            ctx.set_contents("".to_string())
                .map_err(|e| ClipboardError::WriteFailed(e.to_string()))?;
        }
        
        // 清除加密内容
        {
            let mut encrypted_content = self.encrypted_content.lock().unwrap();
            *encrypted_content = None;
        }
        
        // 重置内容哈希
        *self.last_content_hash.lock().unwrap() = 0;
        
        // 触发事件回调
        if let Some(callback) = &*self.event_callback.lock().unwrap() {
            let event = ClipboardEvent::ContentCleared {
                reason: reason.clone(),
                timestamp: Instant::now(),
            };
            callback(event);
        }
        
        // 执行安全内存清理
        SecureMemory::secure_zero_memory();
        
        Ok(())
    }
    
    /// 获取当前状态
    /// 
    /// # 返回值
    /// * `ClipboardState` - 当前状态的副本
    pub fn get_state(&self) -> ClipboardState {
        self.state.lock().unwrap().clone()
    }
    
    /// 获取历史记录
    pub fn get_history(&self) -> Vec<ClipboardHistoryItem> {
        self.history.lock().unwrap().clone()
    }

    /// 添加历史记录
    fn add_history(&self, item: ClipboardHistoryItem) {
        let mut history = self.history.lock().unwrap();
        history.push(item);
        // 保持最近100条记录
        if history.len() > 100 {
            history.remove(0);
        }
    }
    
    /// 计算内容哈希（用于检测变化）
    fn calculate_content_hash(&self, content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }
    
    /// 紧急销毁所有数据
    /// 
    /// # 返回值
    /// * `Result<(), ClipboardError>` - 操作结果
    pub fn emergency_nuke(&self) -> Result<(), ClipboardError> {
        warn!("执行紧急销毁操作");
        
        // 清除剪贴板
        self.clear_clipboard(ClearReason::EmergencyNuke)?;
        
        // 重新生成加密密钥
        {
            let mut crypto = self.crypto_engine.lock().unwrap();
            crypto.regenerate_key()
                .map_err(ClipboardError::CryptoError)?;
        }
        
        // 执行多重内存清理
        for i in 0..3 {
            SecureMemory::secure_zero_memory();
            debug!("内存清理第 {} 轮完成", i + 1);
        }
        
        info!("紧急销毁操作完成");
        Ok(())
    }
}

/// 实现Drop trait确保资源清理
impl Drop for ClipboardMonitor {
    fn drop(&mut self) {
        info!("剪贴板监听器正在销毁");
        
        // 停止监听
        self.stop_monitoring();
        
        // 清除剪贴板内容
        if let Err(e) = self.clear_clipboard(ClearReason::Shutdown) {
            error!("销毁时清除剪贴板失败: {}", e);
        }
    }
}

impl Clone for ClipboardMonitor {
    fn clone(&self) -> Self {
        ClipboardMonitor {
            clipboard_ctx: self.clipboard_ctx.clone(),
            crypto_engine: self.crypto_engine.clone(),
            encrypted_content: self.encrypted_content.clone(),
            event_callback: self.event_callback.clone(),
            should_stop: self.should_stop.clone(),
            last_content_hash: self.last_content_hash.clone(),
            state: self.state.clone(),
            history: self.history.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    #[tokio::test]
    async fn test_clipboard_monitor_creation() {
        let config = Config::default();
        let monitor = ClipboardMonitor::new(config);
        assert!(monitor.is_ok());
    }
    
    #[tokio::test]
    async fn test_event_callback() {
        let config = Config::default();
        let mut monitor = ClipboardMonitor::new(config).unwrap();
        let event_count = Arc::new(AtomicUsize::new(0));
        let event_count_clone = event_count.clone();
        
        let callback = Arc::new(move |_event: ClipboardEvent| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        });
        
        monitor.set_event_callback(callback);
        
        // 测试清除操作会触发事件
        monitor.clear_clipboard(ClearReason::ManualClear).unwrap();
        
        assert_eq!(event_count.load(Ordering::SeqCst), 1);
    }
    
    #[test]
    fn test_content_hash_calculation() {
        let config = Config::default();
        let monitor = ClipboardMonitor::new(config).unwrap();
        
        let content1 = "Hello, World!";
        let content2 = "Hello, World!";
        let content3 = "Different content";
        
        let hash1 = monitor.calculate_content_hash(content1);
        let hash2 = monitor.calculate_content_hash(content2);
        let hash3 = monitor.calculate_content_hash(content3);
        
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
