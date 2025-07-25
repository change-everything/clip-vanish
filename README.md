# ClipVanish™

> 全球首款「物理级自毁」剪贴板工具，实现隐私数据的秒级自动销毁

## 🚀 核心特性

- **🔒 AES-256加密**: 剪贴板内容自动加密，明文仅存于内存
- **⏰ 倒计时自毁**: 粘贴后自动倒计时销毁（默认30秒）
- **🔥 物理级清除**: 覆盖内存+剪贴板缓存，非简单清空
- **⚡ 一键紧急销毁**: Ctrl+Alt+V立即擦除所有剪贴数据
- **🛡️ 内存防护**: mlock()锁定敏感内存，禁止换出到磁盘

## 📦 安装

```bash
# 从源码构建
git clone https://github.com/clipvanish/clipvanish.git
cd clipvanish
cargo build --release

# 运行
./target/release/clipvanish --help
```

## 🎯 使用方法

### 启动监听模式
```bash
# 默认30秒自毁倒计时
clipvanish start

# 自定义倒计时（5秒）
clipvanish start --timer 5

# 静默模式（无输出）
clipvanish start --silent
```

### 紧急销毁
```bash
# 立即销毁所有剪贴板数据
clipvanish nuke

# 或使用全局热键: Ctrl+Alt+V
```

### 查看状态
```bash
# 显示当前状态
clipvanish status
```

## 🔧 技术架构

- **加密引擎**: Rust + AES-GCM-SIV算法，避免时序攻击
- **剪贴板监听**: 跨平台原生API (Win32/NSPasteboard/X11)
- **内存管理**: 自定义内存池 + mlock()防止swap泄露
- **安全设计**: 零残留内存管理，多重覆盖销毁

## 🛡️ 安全保证

1. **加密存储**: 所有剪贴板内容使用AES-256-GCM-SIV加密
2. **内存锁定**: 敏感数据使用mlock()防止换出
3. **安全擦除**: 销毁时使用0x00+随机噪声多重覆盖
4. **时序安全**: 使用常数时间算法避免侧信道攻击

## 📋 系统要求

- **Windows**: Windows 10+ (x64)
- **macOS**: macOS 10.15+ (Intel/Apple Silicon)
- **Linux**: Ubuntu 18.04+ / 其他主流发行版

## 🤝 贡献

欢迎提交Issue和Pull Request！

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件

## ⚠️ 免责声明

本工具仅用于合法的隐私保护目的。用户需自行承担使用责任。
