//! AegisNet Common - 跨模块共享工具与数据结构
//!
//! 该模块提供 AegisNet 项目中所有组件共享的数据结构、错误处理和工具函数。
//! 包括 SPIFFE ID、策略规则等数据模型以及统一的错误处理机制。

pub mod models;
pub mod error;

/// 重新导出常用类型，方便使用
pub use error::Error;
pub use error::Result;
pub use models::policy::*;
pub use models::spiffe::*;