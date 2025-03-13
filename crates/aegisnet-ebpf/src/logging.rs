//! 内核态日志记录模块
//!
//! 该模块实现内核态 eBPF 程序的日志记录功能，用于调试和监控。
//! 使用 perf event 将日志从内核态传输到用户态。

use aya::maps::perf::PerfEventArray;
use aya::util::online_cpus;
use std::sync::Arc;
use std::thread;
use anyhow::Result;

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// 调试信息
    Debug,
    /// 普通信息
    Info,
    /// 警告信息
    Warning,
    /// 错误信息
    Error,
}

/// 日志记录器
pub struct Logger {
    /// Perf event 数组，用于传输日志
    perf_array: Arc<PerfEventArray<u32>>,
    /// 是否启用调试日志
    debug_enabled: bool,
}

impl Logger {
    /// 创建新的日志记录器
    pub fn new(perf_array: PerfEventArray<u32>, debug_enabled: bool) -> Self {
        Self {
            perf_array: Arc::new(perf_array),
            debug_enabled,
        }
    }

    /// 启动日志监听线程
    pub fn start_log_listener(&self) -> Result<()> {
        let cpus = online_cpus()?;
        let perf_array = self.perf_array.clone();

        for cpu in cpus {
            let perf_array = perf_array.clone();
            thread::spawn(move || {
                let mut buffers = vec![0u8; 1024];
                loop {
                    match perf_array.read_events(cpu, &mut buffers) {
                        Ok((events, _)) => {
                            for event in events {
                                // 处理日志事件
                                if let Ok(log_str) = std::str::from_utf8(&event) {
                                    println!("[CPU {}] {}", cpu, log_str);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading perf events: {}", e);
                            break;
                        }
                    }
                }
            });
        }

        Ok(())
    }

    /// 记录日志（用户态）
    pub fn log(&self, level: LogLevel, message: &str) {
        if level == LogLevel::Debug && !self.debug_enabled {
            return;
        }

        let level_str = match level {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        };

        println!("[{}] {}", level_str, message);
    }
}

/// 创建默认的日志记录器
pub fn create_default_logger(perf_array: PerfEventArray<u32>) -> Logger {
    Logger::new(perf_array, cfg!(debug_assertions))
}