//! 身份管理模块
//!
//! 该模块负责管理和验证服务身份，集成 SPIRE 作为身份提供方。
//! 包含 SPIRE 客户端和身份缓存实现。

mod spire_client;
mod identity_cache;

pub use spire_client::{SpireClient, SpireClientConfig, SpiffeIdentity, create_default_client};
pub use identity_cache::{IdentityCache, create_default_cache};

use anyhow::Result;
use std::sync::Arc;

/// 初始化身份管理系统
pub async fn init_identity_system() -> Result<Arc<SpireClient>> {
    // 创建 SPIRE 客户端
    let client = Arc::new(create_default_client());
    
    // 启动身份刷新任务
    client.start_refresh_task().await?;
    
    // 创建并初始化身份缓存
    let cache = create_default_cache(client.clone());
    cache.start_cleanup_task().await?;
    
    Ok(client)
}