/*!
 * ClipVanish™ - 物理级自毁剪贴板工具
 * 
 * 主程序入口，负责命令行参数解析和程序启动
 * 
 * 作者: ClipVanish Team
 * 版本: 0.1.0 (MVP)
 */

use clap::{Parser, Subcommand};
use log::{info, error, warn};
use std::process;
use tokio;

mod crypto;
mod clipboard;
mod timer;
mod memory;
mod cli;
mod config;

use crate::cli::CliHandler;
use crate::config::Config;

/// ClipVanish™ 命令行参数定义
#[derive(Parser)]
#[command(
    name = "clipvanish",
    version = "0.1.0",
    about = "ClipVanish™ - 物理级自毁剪贴板工具",
    long_about = "全球首款「物理级自毁」剪贴板工具，实现隐私数据的秒级自动销毁。\n支持AES-256加密、倒计时自毁、一键紧急销毁等功能。"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
    
    /// 启用详细日志输出
    #[arg(short, long)]
    verbose: bool,
    
    /// 静默模式（最小输出）
    #[arg(short, long)]
    silent: bool,
}

/// 支持的命令列表
#[derive(Subcommand)]
enum Commands {
    /// 启动剪贴板监听和自毁服务
    Start {
        /// 自毁倒计时（秒），默认30秒
        #[arg(short, long, default_value = "30")]
        timer: u64,
        
        /// 后台运行模式
        #[arg(short, long)]
        daemon: bool,
    },
    
    /// 立即销毁所有剪贴板数据（紧急模式）
    Nuke {
        /// 强制模式，跳过确认
        #[arg(short, long)]
        force: bool,
    },
    
    /// 显示当前运行状态
    Status,
    
    /// 停止运行中的ClipVanish服务
    Stop,
    
    /// 显示配置信息
    Config {
        /// 重置为默认配置
        #[arg(long)]
        reset: bool,
    },
}

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args = Args::parse();
    
    // 初始化日志系统
    init_logger(args.verbose, args.silent);
    
    // 显示启动信息
    if !args.silent {
        println!("🔒 ClipVanish™ v0.1.0 - 物理级自毁剪贴板工具");
        println!("   作者: ClipVanish Team | MIT License\n");
    }
    
    // 加载配置
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("配置加载失败: {}", e);
            process::exit(1);
        }
    };
    
    // 创建CLI处理器
    let mut cli_handler = CliHandler::new(config);
    
    // 根据命令执行相应操作
    let result = match args.command {
        Commands::Start { timer, daemon } => {
            info!("启动剪贴板监听服务，自毁倒计时: {}秒", timer);
            cli_handler.start_monitoring(timer, daemon).await
        },
        
        Commands::Nuke { force } => {
            warn!("执行紧急销毁操作");
            cli_handler.emergency_nuke(force).await
        },
        
        Commands::Status => {
            cli_handler.show_status().await
        },
        
        Commands::Stop => {
            info!("停止ClipVanish服务");
            cli_handler.stop_service().await
        },
        
        Commands::Config { reset } => {
            if reset {
                info!("重置配置为默认值");
            }
            cli_handler.manage_config(reset).await
        },
    };
    
    // 处理执行结果
    match result {
        Ok(_) => {
            if !args.silent {
                println!("✅ 操作完成");
            }
        },
        Err(e) => {
            error!("操作失败: {}", e);
            process::exit(1);
        }
    }
}

/// 初始化日志系统
/// 
/// # 参数
/// * `verbose` - 是否启用详细日志
/// * `silent` - 是否启用静默模式
fn init_logger(verbose: bool, silent: bool) {
    let log_level = if silent {
        "error"
    } else if verbose {
        "debug"
    } else {
        "info"
    };
    
    std::env::set_var("RUST_LOG", format!("clipvanish={}", log_level));
    env_logger::init();
    
    if verbose {
        info!("日志系统已初始化，级别: {}", log_level);
    }
}
