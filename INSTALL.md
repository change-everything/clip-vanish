# ClipVanish™ 安装指南

## 🚀 快速开始

### 系统要求

- **Windows**: Windows 10+ (x64)
- **macOS**: macOS 10.15+ (Intel/Apple Silicon)  
- **Linux**: Ubuntu 18.04+ / 其他主流发行版

### 前置依赖

#### Windows
1. **Rust工具链**
   ```powershell
   # 方法1：使用winget（推荐）
   winget install Rustlang.Rust.MSVC
   
   # 方法2：手动安装
   # 访问 https://rustup.rs/ 下载安装
   ```

2. **Microsoft C++ Build Tools**
   ```powershell
   # 方法1：使用winget
   winget install Microsoft.VisualStudio.2022.BuildTools
   
   # 方法2：手动下载
   # 访问 https://visualstudio.microsoft.com/visual-cpp-build-tools/
   ```

#### macOS
```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装Xcode命令行工具
xcode-select --install
```

#### Linux (Ubuntu/Debian)
```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装依赖
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libxcb1-dev
```

## 📦 编译安装

### 1. 克隆项目
```bash
git clone https://github.com/clipvanish/clipvanish.git
cd clipvanish
```

### 2. 验证环境
```bash
# 检查Rust版本
rustc --version
cargo --version

# Windows用户检查链接器
where link.exe
```

### 3. 编译项目
```bash
# 开发版本（包含调试信息）
cargo build

# 发布版本（优化编译）
cargo build --release
```

### 4. 运行测试
```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test crypto
cargo test clipboard
```

## 🎯 使用方法

### 基本命令

```bash
# 查看帮助
./target/release/clipvanish --help

# 启动监听服务（默认30秒自毁）
./target/release/clipvanish start

# 自定义倒计时
./target/release/clipvanish start --timer 60

# 后台运行
./target/release/clipvanish start --daemon

# 查看运行状态
./target/release/clipvanish status

# 紧急销毁所有数据
./target/release/clipvanish nuke

# 强制销毁（跳过确认）
./target/release/clipvanish nuke --force

# 停止服务
./target/release/clipvanish stop

# 查看配置
./target/release/clipvanish config

# 重置配置
./target/release/clipvanish config --reset
```

### 全局热键

- **Ctrl+Alt+V**: 紧急销毁所有剪贴板数据
- **Ctrl+Alt+S**: 显示当前状态
- **Ctrl+Alt+M**: 暂停/恢复监听

## 🔧 配置文件

配置文件位置：
- **Windows**: `%APPDATA%\ClipVanish\config.json`
- **macOS**: `~/Library/Application Support/ClipVanish/config.json`
- **Linux**: `~/.config/clipvanish/config.json`

### 配置示例
```json
{
  "version": "0.1.0",
  "timer": {
    "default_countdown": 30,
    "min_countdown": 5,
    "max_countdown": 3600,
    "enable_warnings": true,
    "warning_threshold": 10
  },
  "security": {
    "enable_memory_locking": true,
    "memory_erase_rounds": 3,
    "auto_clear_on_exit": true,
    "enable_key_rotation": false,
    "key_rotation_interval": 60
  },
  "ui": {
    "verbose_output": false,
    "show_progress": true,
    "enable_colors": true,
    "log_level": "info",
    "enable_tray_icon": true
  },
  "hotkeys": {
    "enable_global_hotkeys": true,
    "emergency_nuke_key": "Ctrl+Alt+V",
    "show_status_key": "Ctrl+Alt+S",
    "toggle_monitoring_key": "Ctrl+Alt+M"
  },
  "clipboard": {
    "poll_interval_ms": 100,
    "supported_types": ["text"],
    "max_content_length": 1048576,
    "enable_length_limit": true
  }
}
```

## 🛠️ 开发

### 开发环境设置
```bash
# 安装开发依赖
cargo install cargo-watch cargo-tarpaulin

# 实时编译检查
cargo watch -x check

# 实时测试
cargo watch -x test

# 代码覆盖率
cargo tarpaulin --out Html
```

### 代码结构
```
src/
├── main.rs          # 程序入口
├── cli.rs           # 命令行界面
├── config.rs        # 配置管理
├── crypto.rs        # 加密引擎
├── clipboard.rs     # 剪贴板监听
├── timer.rs         # 定时器模块
└── memory.rs        # 内存管理
```

## 🐛 故障排除

### 常见问题

1. **链接器错误 (Windows)**
   ```
   error: linker `link.exe` not found
   ```
   **解决方案**: 安装Microsoft C++ Build Tools

2. **权限错误**
   ```
   Permission denied
   ```
   **解决方案**: 以管理员权限运行，或检查防病毒软件设置

3. **剪贴板访问失败**
   ```
   clipboard access failed
   ```
   **解决方案**: 确保没有其他程序独占剪贴板

4. **内存锁定失败**
   ```
   memory lock failed
   ```
   **解决方案**: 在配置中禁用内存锁定，或以管理员权限运行

### 日志调试

```bash
# 启用详细日志
RUST_LOG=debug ./target/release/clipvanish start --verbose

# 查看特定模块日志
RUST_LOG=clipvanish::crypto=debug ./target/release/clipvanish start
```

## 📞 获取帮助

- **GitHub Issues**: https://github.com/clipvanish/clipvanish/issues
- **文档**: https://clipvanish.github.io/docs
- **社区**: https://discord.gg/clipvanish

## ⚠️ 安全提醒

1. ClipVanish会监听所有剪贴板活动
2. 敏感数据会被加密存储在内存中
3. 程序退出时会自动清理所有数据
4. 建议定期更新到最新版本

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件
