//! AegisNet eBPF 模块
//!
//! 该模块包含 AegisNet 的内核态 eBPF 程序，负责网络流量的拦截、身份认证和策略执行。
//! 使用 Aya 框架进行 eBPF 程序的加载和管理。

mod auth;
mod encryption;
mod policy;
mod maps;
mod logging;
mod hooks;

pub use auth::*;
pub use encryption::*;
pub use policy::*;
pub use maps::*;
pub use logging::*;
pub use hooks::*;

/// eBPF 程序初始化函数
pub fn init() -> anyhow::Result<()> {
    // 初始化 eBPF 程序
    Ok(())
}