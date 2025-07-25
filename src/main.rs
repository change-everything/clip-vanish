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
use std::io::{self, Write};

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
    command: Option<Commands>,
    
    /// 启用详细日志输出
    #[arg(short, long)]
    verbose: bool,
    
    /// 静默模式（最小输出）
    #[arg(short, long)]
    silent: bool,
    
    /// 交互模式
    #[arg(short, long)]
    interactive: bool,
}

/// 支持的命令列表
#[derive(Subcommand, Clone)]
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
    
    /// 退出程序
    Exit,
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

    if args.interactive {
        // 交互模式
        println!("欢迎使用 ClipVanish™ 交互式命令行！");
        println!("输入 'help' 查看可用命令，输入 'exit' 退出程序。\n");

        loop {
            print!("clipvanish> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                error!("读取输入失败");
                continue;
            }

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            // 处理帮助命令
            if input == "help" {
                print_help();
                continue;
            }

            // 解析命令
            let args = match parse_interactive_command(input) {
                Ok(cmd) => Args {
                    command: Some(cmd),
                    verbose: args.verbose,
                    silent: args.silent,
                    interactive: true,
                },
                Err(e) => {
                    println!("❌ 命令解析错误: {}", e);
                    continue;
                }
            };

            // 执行命令
            if let Some(cmd) = args.command {
                match &cmd {
                    Commands::Exit => {
                        println!("👋 感谢使用 ClipVanish™，再见！");
                        break;
                    }
                    _ => {
                        if let Err(e) = execute_command(&mut cli_handler, cmd).await {
                            error!("命令执行失败: {}", e);
                        }
                    }
                }
            }
        }
    } else {
        // 非交互模式，执行单个命令
        if let Some(cmd) = args.command {
            if let Err(e) = execute_command(&mut cli_handler, cmd).await {
                error!("命令执行失败: {}", e);
                process::exit(1);
            }
        } else {
            println!("请使用 --help 查看使用说明");
        }
    }
}

/// 打印帮助信息
fn print_help() {
    println!("可用命令：");
    println!("  start [--timer <seconds>] [--daemon]  启动剪贴板监听服务");
    println!("  nuke [--force]                       紧急销毁所有数据");
    println!("  status                               显示当前状态");
    println!("  stop                                 停止服务");
    println!("  config [--reset]                     查看/重置配置");
    println!("  help                                 显示此帮助信息");
    println!("  exit                                 退出程序\n");
}

/// 解析交互式命令
fn parse_interactive_command(input: &str) -> Result<Commands, String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Err("命令不能为空".to_string());
    }

    match parts[0] {
        "start" => {
            let mut timer = 30u64;
            let mut daemon = false;

            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "--timer" | "-t" => {
                        if i + 1 >= parts.len() {
                            return Err("--timer 需要一个参数".to_string());
                        }
                        timer = parts[i + 1].parse()
                            .map_err(|_| "timer 参数必须是一个数字".to_string())?;
                        i += 2;
                    }
                    "--daemon" | "-d" => {
                        daemon = true;
                        i += 1;
                    }
                    _ => {
                        return Err(format!("未知参数: {}", parts[i]));
                    }
                }
            }

            Ok(Commands::Start { timer, daemon })
        }
        "nuke" => {
            let force = parts.get(1).map_or(false, |&arg| arg == "--force" || arg == "-f");
            Ok(Commands::Nuke { force })
        }
        "status" => Ok(Commands::Status),
        "stop" => Ok(Commands::Stop),
        "config" => {
            let reset = parts.get(1).map_or(false, |&arg| arg == "--reset");
            Ok(Commands::Config { reset })
        }
        "exit" => Ok(Commands::Exit),
        _ => Err(format!("未知命令: {}", parts[0])),
    }
}

/// 执行命令
async fn execute_command(cli_handler: &mut CliHandler, command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Start { timer, daemon } => {
            // 使用克隆的引用来启动服务
            cli_handler.start_monitoring(timer, false).await?;
            Ok(())
        },
        Commands::Nuke { force } => {
            cli_handler.emergency_nuke(force).await?;
            Ok(())
        },
        Commands::Status => {
            cli_handler.show_status().await?;
            Ok(())
        },
        Commands::Stop => {
            cli_handler.stop_service().await?;
            Ok(())
        },
        Commands::Config { reset } => {
            cli_handler.manage_config(reset).await?;
            Ok(())
        },
        Commands::Exit => {
            // 交互模式下的退出命令，在主循环中处理
            Ok(())
        },
    }
}

/// 初始化日志系统
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
