//! 错误处理模块
//!
//! 该模块提供 AegisNet 项目的统一错误处理机制，包括自定义错误类型、
//! 错误转换和详细的错误信息，便于调试和日志记录。

use std::fmt;
use std::io;
use thiserror::Error;

/// AegisNet 统一错误类型
#[derive(Error, Debug)]
pub enum Error {
    /// 配置错误
    #[error("配置错误: {0}")]
    Config(String),

    /// 网络错误
    #[error("网络错误: {0}")]
    Network(String),

    /// eBPF 程序错误
    #[error("eBPF 程序错误: {0}")]
    Ebpf(String),

    /// 身份验证错误
    #[error("身份验证错误: {0}")]
    Authentication(String),

    /// 授权错误
    #[error("授权错误: {0}")]
    Authorization(String),

    /// 策略错误
    #[error("策略错误: {0}")]
    Policy(String),

    /// 序列化/反序列化错误
    #[error("序列化/反序列化错误: {0}")]
    Serialization(String),

    /// I/O 错误
    #[error("I/O 错误: {0}")]
    Io(#[from] io::Error),

    /// JSON 错误
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// 未知错误
    #[error("未知错误: {0}")]
    Unknown(String),
}

/// AegisNet 结果类型别名
pub type Result<T> = std::result::Result<T, Error>;

/// 错误上下文信息
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// 错误发生的文件
    pub file: String,
    /// 错误发生的行号
    pub line: u32,
    /// 错误发生的函数
    pub function: String,
    /// 错误详细信息
    pub details: String,
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "位置: {}:{} ({}), 详情: {}",
            self.file, self.line, self.function, self.details
        )
    }
}

/// 创建带有上下文的错误
#[macro_export]
macro_rules! error_with_context {
    ($error:expr, $details:expr) => {
        {
            let context = $crate::error::ErrorContext {
                file: file!().to_string(),
                line: line!(),
                function: stringify!(#[function_name]).to_string(),
                details: $details.to_string(),
            };
            tracing::error!("{}: {}", $error, context);
            $error
        }
    };
}

/// 从字符串创建错误
pub trait IntoError<T> {
    /// 将当前类型转换为错误
    fn into_error(self, kind: fn(String) -> Error) -> Result<T>;
}

impl<T> IntoError<T> for String {
    fn into_error(self, kind: fn(String) -> Error) -> Result<T> {
        Err(kind(self))
    }
}

impl<T> IntoError<T> for &str {
    fn into_error(self, kind: fn(String) -> Error) -> Result<T> {
        Err(kind(self.to_string()))
    }
}