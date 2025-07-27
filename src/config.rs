/*!
 * ClipVanish™ 配置管理模块
 * 
 * 负责应用程序配置的加载、保存和管理
 * 特点：
 * - JSON格式配置文件
 * - 默认配置自动生成
 * - 配置验证和错误处理
 * - 跨平台配置目录支持
 * 
 * 作者: ClipVanish Team
 */

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use log::{info, warn, debug, error};

/// 配置错误类型
#[derive(Debug)]
pub enum ConfigError {
    /// 文件读取失败
    FileReadError(std::io::Error),
    /// 文件写入失败
    FileWriteError(std::io::Error),
    /// JSON解析失败
    ParseError(serde_json::Error),
    /// 配置验证失败
    ValidationError(String),
    /// 配置目录创建失败
    DirectoryCreationError(std::io::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileReadError(e) => write!(f, "配置文件读取失败: {}", e),
            ConfigError::FileWriteError(e) => write!(f, "配置文件写入失败: {}", e),
            ConfigError::ParseError(e) => write!(f, "配置文件解析失败: {}", e),
            ConfigError::ValidationError(msg) => write!(f, "配置验证失败: {}", msg),
            ConfigError::DirectoryCreationError(e) => write!(f, "配置目录创建失败: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

/// 定时器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerConfig {
    /// 默认自毁倒计时（秒）
    pub default_countdown: u64,
    /// 最小倒计时（秒）
    pub min_countdown: u64,
    /// 最大倒计时（秒）
    pub max_countdown: u64,
    /// 是否启用倒计时提醒
    pub enable_warnings: bool,
    /// 提醒阈值（秒）- 剩余时间少于此值时开始提醒
    pub warning_threshold: u64,
}

impl Default for TimerConfig {
    fn default() -> Self {
        TimerConfig {
            default_countdown: 30,
            min_countdown: 5,
            max_countdown: 3600, // 1小时
            enable_warnings: true,
            warning_threshold: 10,
        }
    }
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// 是否启用内存锁定
    pub enable_memory_locking: bool,
    /// 内存擦除轮数
    pub memory_erase_rounds: u32,
    /// 是否在程序退出时自动清除剪贴板
    pub auto_clear_on_exit: bool,
    /// 是否启用加密密钥自动轮换
    pub enable_key_rotation: bool,
    /// 密钥轮换间隔（分钟）
    pub key_rotation_interval: u64,
    /// 是否在粘贴后立即销毁
    pub destroy_on_paste: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            enable_memory_locking: true,
            memory_erase_rounds: 3,
            auto_clear_on_exit: true,
            enable_key_rotation: false,
            key_rotation_interval: 60, // 1小时
            destroy_on_paste: true,  // 默认启用粘贴即销毁
        }
    }
}

/// 界面配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// 是否启用详细输出
    pub verbose_output: bool,
    /// 是否显示进度条
    pub show_progress: bool,
    /// 是否启用颜色输出
    pub enable_colors: bool,
    /// 日志级别
    pub log_level: String,
    /// 是否启用系统托盘图标
    pub enable_tray_icon: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig {
            verbose_output: false,
            show_progress: true,
            enable_colors: true,
            log_level: "info".to_string(),
            enable_tray_icon: true,
        }
    }
}

/// 热键配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// 是否启用全局热键
    pub enable_global_hotkeys: bool,
    /// 紧急销毁热键
    pub emergency_nuke_key: String,
    /// 显示状态热键
    pub show_status_key: String,
    /// 暂停/恢复监听热键
    pub toggle_monitoring_key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            enable_global_hotkeys: true,
            emergency_nuke_key: "Ctrl+Alt+V".to_string(),
            show_status_key: "Ctrl+Alt+S".to_string(),
            toggle_monitoring_key: "Ctrl+Alt+M".to_string(),
        }
    }
}

/// 剪贴板配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    /// 轮询间隔（毫秒）
    pub poll_interval_ms: u64,
    /// 支持的内容类型
    pub supported_types: Vec<String>,
    /// 最大内容长度（字节）
    pub max_content_length: usize,
    /// 是否启用内容长度限制
    pub enable_length_limit: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        ClipboardConfig {
            poll_interval_ms: 100,
            supported_types: vec!["text".to_string()],
            max_content_length: 1024 * 1024, // 1MB
            enable_length_limit: true,
        }
    }
}

/// 主配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 配置版本
    pub version: String,
    /// 定时器配置
    pub timer: TimerConfig,
    /// 安全配置
    pub security: SecurityConfig,
    /// 界面配置
    pub ui: UiConfig,
    /// 热键配置
    pub hotkeys: HotkeyConfig,
    /// 剪贴板配置
    pub clipboard: ClipboardConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: "0.1.0".to_string(),
            timer: TimerConfig::default(),
            security: SecurityConfig::default(),
            ui: UiConfig::default(),
            hotkeys: HotkeyConfig::default(),
            clipboard: ClipboardConfig::default(),
        }
    }
}

impl Config {
    /// 加载配置文件
    /// 
    /// # 返回值
    /// * `Result<Config, ConfigError>` - 成功返回配置实例
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::get_config_file_path()?;
        
        if config_path.exists() {
            debug!("从文件加载配置: {:?}", config_path);
            Self::load_from_file(&config_path)
        } else {
            info!("配置文件不存在，创建默认配置");
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }
    
    /// 从指定文件加载配置
    /// 
    /// # 参数
    /// * `path` - 配置文件路径
    /// 
    /// # 返回值
    /// * `Result<Config, ConfigError>` - 成功返回配置实例
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(ConfigError::FileReadError)?;
        
        // 尝试加载配置
        let result: Result<Config, _> = serde_json::from_str(&content);
        
        let config = match result {
            Ok(config) => {
                // 配置加载成功，验证并返回
                config.validate()?;
                config
            },
            Err(e) => {
                warn!("配置加载出现问题: {}", e);
                info!("尝试使用现有值并添加缺失的字段...");
                
                // 创建默认配置
                let default_config = Config::default();
                
                // 尝试解析现有配置
                if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(obj) = json.as_object_mut() {
                        // 添加缺失的安全配置字段
                        if let Some(security) = obj.get_mut("security") {
                            if let Some(security_obj) = security.as_object_mut() {
                                if !security_obj.contains_key("destroy_on_paste") {
                                    security_obj.insert(
                                        "destroy_on_paste".to_string(),
                                        serde_json::Value::Bool(default_config.security.destroy_on_paste)
                                    );
                                }
                            }
                        }
                        
                        // 保存更新后的配置并重新加载
                        let path_ref = path.as_ref();
                        let updated_content = serde_json::to_string_pretty(&json)
                            .map_err(ConfigError::ParseError)?;
                        fs::write(path_ref, &updated_content)
                            .map_err(ConfigError::FileWriteError)?;
                        
                        // 使用更新后的内容直接解析，而不是重新读取文件
                        return serde_json::from_str(&updated_content)
                            .map_err(ConfigError::ParseError);
                    }
                }
                
                // 如果更新失败，返回默认配置
                warn!("无法修复配置文件，使用默认配置");
                default_config
            }
        };
        
        info!("配置加载成功");
        Ok(config)
    }
    
    /// 保存配置到文件
    /// 
    /// # 返回值
    /// * `Result<(), ConfigError>` - 操作结果
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::get_config_file_path()?;
        self.save_to_file(&config_path)
    }
    
    /// 保存配置到指定文件
    /// 
    /// # 参数
    /// * `path` - 配置文件路径
    /// 
    /// # 返回值
    /// * `Result<(), ConfigError>` - 操作结果
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        // 确保配置目录存在
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(ConfigError::DirectoryCreationError)?;
        }
        
        let content = serde_json::to_string_pretty(self)
            .map_err(ConfigError::ParseError)?;
        
        fs::write(path, content)
            .map_err(ConfigError::FileWriteError)?;
        
        debug!("配置保存成功");
        Ok(())
    }
    
    /// 验证配置的有效性
    /// 
    /// # 返回值
    /// * `Result<(), ConfigError>` - 验证结果
    pub fn validate(&self) -> Result<(), ConfigError> {
        // 验证定时器配置
        if self.timer.min_countdown > self.timer.max_countdown {
            return Err(ConfigError::ValidationError(
                "最小倒计时不能大于最大倒计时".to_string()
            ));
        }
        
        if self.timer.default_countdown < self.timer.min_countdown ||
           self.timer.default_countdown > self.timer.max_countdown {
            return Err(ConfigError::ValidationError(
                "默认倒计时必须在最小值和最大值之间".to_string()
            ));
        }
        
        if self.timer.warning_threshold > self.timer.default_countdown {
            return Err(ConfigError::ValidationError(
                "警告阈值不能大于默认倒计时".to_string()
            ));
        }
        
        // 验证安全配置
        if self.security.memory_erase_rounds == 0 {
            return Err(ConfigError::ValidationError(
                "内存擦除轮数必须大于0".to_string()
            ));
        }
        
        if self.security.memory_erase_rounds > 10 {
            warn!("内存擦除轮数过多可能影响性能: {}", self.security.memory_erase_rounds);
        }
        
        // 验证剪贴板配置
        if self.clipboard.poll_interval_ms == 0 {
            return Err(ConfigError::ValidationError(
                "轮询间隔必须大于0".to_string()
            ));
        }
        
        if self.clipboard.poll_interval_ms < 50 {
            warn!("轮询间隔过短可能影响性能: {}ms", self.clipboard.poll_interval_ms);
        }
        
        // 验证日志级别
        let valid_log_levels = ["error", "warn", "info", "debug", "trace"];
        if !valid_log_levels.contains(&self.ui.log_level.as_str()) {
            return Err(ConfigError::ValidationError(
                format!("无效的日志级别: {}", self.ui.log_level)
            ));
        }
        
        debug!("配置验证通过");
        Ok(())
    }
    
    /// 重置为默认配置
    /// 
    /// # 返回值
    /// * `Result<(), ConfigError>` - 操作结果
    pub fn reset_to_default(&mut self) -> Result<(), ConfigError> {
        *self = Config::default();
        self.save()?;
        info!("配置已重置为默认值");
        Ok(())
    }
    
    /// 获取配置文件路径
    /// 
    /// # 返回值
    /// * `Result<PathBuf, ConfigError>` - 配置文件路径
    fn get_config_file_path() -> Result<PathBuf, ConfigError> {
        let config_dir = Self::get_config_directory()?;
        Ok(config_dir.join("config.json"))
    }
    
    /// 获取配置目录路径
    /// 
    /// # 返回值
    /// * `Result<PathBuf, ConfigError>` - 配置目录路径
    fn get_config_directory() -> Result<PathBuf, ConfigError> {
        // 跨平台配置目录
        let config_dir = if cfg!(windows) {
            // Windows: %APPDATA%\ClipVanish
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("ClipVanish")
        } else if cfg!(target_os = "macos") {
            // macOS: ~/Library/Application Support/ClipVanish
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("Library")
                .join("Application Support")
                .join("ClipVanish")
        } else {
            // Linux/Unix: ~/.config/clipvanish
            std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    std::env::var("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(".config")
                })
                .join("clipvanish")
        };
        
        Ok(config_dir)
    }
    
    /// 获取默认倒计时时长
    /// 
    /// # 返回值
    /// * `Duration` - 默认倒计时时长
    pub fn get_default_countdown_duration(&self) -> Duration {
        Duration::from_secs(self.timer.default_countdown)
    }
    
    /// 获取轮询间隔
    /// 
    /// # 返回值
    /// * `Duration` - 轮询间隔
    pub fn get_poll_interval(&self) -> Duration {
        Duration::from_millis(self.clipboard.poll_interval_ms)
    }
    
    /// 检查是否启用内存锁定
    /// 
    /// # 返回值
    /// * `bool` - 是否启用内存锁定
    pub fn is_memory_locking_enabled(&self) -> bool {
        self.security.enable_memory_locking
    }
    
    /// 获取内存擦除轮数
    /// 
    /// # 返回值
    /// * `u32` - 内存擦除轮数
    pub fn get_memory_erase_rounds(&self) -> u32 {
        self.security.memory_erase_rounds
    }
    
    /// 显示当前配置
    pub fn display(&self) {
        println!("📋 ClipVanish™ 配置信息");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔧 版本: {}", self.version);
        println!();
        
        println!("⏰ 定时器配置:");
        println!("   默认倒计时: {}秒", self.timer.default_countdown);
        println!("   倒计时范围: {}-{}秒", self.timer.min_countdown, self.timer.max_countdown);
        println!("   警告阈值: {}秒", self.timer.warning_threshold);
        println!("   启用警告: {}", if self.timer.enable_warnings { "是" } else { "否" });
        println!();
        
        println!("🛡️ 安全配置:");
        println!("   内存锁定: {}", if self.security.enable_memory_locking { "启用" } else { "禁用" });
        println!("   擦除轮数: {}轮", self.security.memory_erase_rounds);
        println!("   退出时清除: {}", if self.security.auto_clear_on_exit { "是" } else { "否" });
        println!("   密钥轮换: {}", if self.security.enable_key_rotation { "启用" } else { "禁用" });
        println!("   粘贴即销毁: {}", if self.security.destroy_on_paste { "启用" } else { "禁用" });
        println!();
        
        println!("🎨 界面配置:");
        println!("   详细输出: {}", if self.ui.verbose_output { "是" } else { "否" });
        println!("   显示进度: {}", if self.ui.show_progress { "是" } else { "否" });
        println!("   彩色输出: {}", if self.ui.enable_colors { "是" } else { "否" });
        println!("   日志级别: {}", self.ui.log_level);
        println!();
        
        println!("⌨️ 热键配置:");
        println!("   全局热键: {}", if self.hotkeys.enable_global_hotkeys { "启用" } else { "禁用" });
        println!("   紧急销毁: {}", self.hotkeys.emergency_nuke_key);
        println!("   显示状态: {}", self.hotkeys.show_status_key);
        println!("   切换监听: {}", self.hotkeys.toggle_monitoring_key);
        println!();
        
        println!("📋 剪贴板配置:");
        println!("   轮询间隔: {}ms", self.clipboard.poll_interval_ms);
        println!("   支持类型: {}", self.clipboard.supported_types.join(", "));
        println!("   最大长度: {} 字节", self.clipboard.max_content_length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.timer.default_countdown, 30);
        assert!(config.security.enable_memory_locking);
        assert!(config.ui.show_progress);
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // 测试有效配置
        assert!(config.validate().is_ok());
        
        // 测试无效配置 - 最小倒计时大于最大倒计时
        config.timer.min_countdown = 100;
        config.timer.max_countdown = 50;
        assert!(config.validate().is_err());
        
        // 修复配置
        config.timer.min_countdown = 5;
        config.timer.max_countdown = 3600;
        assert!(config.validate().is_ok());
        
        // 测试无效日志级别
        config.ui.log_level = "invalid".to_string();
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_config_save_load() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.json");
        
        let original_config = Config::default();
        
        // 保存配置
        original_config.save_to_file(&config_path).unwrap();
        
        // 加载配置
        let loaded_config = Config::load_from_file(&config_path).unwrap();
        
        // 验证配置一致性
        assert_eq!(original_config.version, loaded_config.version);
        assert_eq!(original_config.timer.default_countdown, loaded_config.timer.default_countdown);
        assert_eq!(original_config.security.memory_erase_rounds, loaded_config.security.memory_erase_rounds);
    }
    
    #[test]
    fn test_duration_helpers() {
        let config = Config::default();
        
        let countdown_duration = config.get_default_countdown_duration();
        assert_eq!(countdown_duration, Duration::from_secs(30));
        
        let poll_interval = config.get_poll_interval();
        assert_eq!(poll_interval, Duration::from_millis(100));
    }
    
    #[test]
    fn test_config_directory() {
        let config_dir = Config::get_config_directory().unwrap();
        assert!(!config_dir.as_os_str().is_empty());
        
        // 配置目录应该包含应用名称
        let path_str = config_dir.to_string_lossy().to_lowercase();
        assert!(path_str.contains("clipvanish") || path_str.contains("ClipVanish"));
    }
}
