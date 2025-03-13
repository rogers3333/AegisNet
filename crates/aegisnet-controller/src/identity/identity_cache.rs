//! 身份信息缓存模块
//!
//! 该模块实现 SPIFFE 身份信息的本地缓存，减少对 SPIRE 服务器的请求频率。
//! 支持定期刷新和过期清理机制。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use anyhow::Result;
use tracing::{info, warn, debug};
use super::spire_client::{SpiffeIdentity, SpireClient};

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 身份信息
    identity: SpiffeIdentity,
    /// 缓存时间
    cached_at: SystemTime,
    /// 过期时间
    expires_at: SystemTime,
}

/// 身份缓存管理器
pub struct IdentityCache {
    /// 缓存数据
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// SPIRE 客户端
    spire_client: Arc<SpireClient>,
    /// 缓存有效期（秒）
    ttl: u64,
}

impl IdentityCache {
    /// 创建新的身份缓存
    pub fn new(spire_client: Arc<SpireClient>, ttl: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            spire_client,
            ttl,
        }
    }

    /// 获取身份信息
    pub async fn get_identity(&self, spiffe_id: &str) -> Result<Option<SpiffeIdentity>> {
        // 先检查缓存
        let cache = self.cache.read().await;
        let now = SystemTime::now();
        
        if let Some(entry) = cache.get(spiffe_id) {
            if now < entry.expires_at {
                debug!("从缓存获取身份信息: {}", spiffe_id);
                return Ok(Some(entry.identity.clone()));
            }
        }
        drop(cache); // 释放读锁
        
        // 缓存未命中或已过期，从 SPIRE 获取
        debug!("缓存未命中，从 SPIRE 获取身份信息: {}", spiffe_id);
        if self.spire_client.validate_id(spiffe_id).await? {
            // 获取所有身份并更新缓存
            let identities = self.spire_client.get_valid_identities().await?;
            let mut cache = self.cache.write().await;
            
            let now = SystemTime::now();
            let expires_at = now + Duration::from_secs(self.ttl);
            
            for identity in &identities {
                cache.insert(identity.id.clone(), CacheEntry {
                    identity: identity.clone(),
                    cached_at: now,
                    expires_at,
                });
            }
            
            // 返回请求的身份
            for identity in identities {
                if identity.id == spiffe_id {
                    return Ok(Some(identity));
                }
            }
        }
        
        Ok(None)
    }

    /// 启动缓存清理任务
    pub async fn start_cleanup_task(&self) -> Result<()> {
        let cache = self.cache.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // 每分钟清理一次
            loop {
                interval.tick().await;
                let now = SystemTime::now();
                let mut cache = cache.write().await;
                
                // 移除过期条目
                let expired: Vec<String> = cache.iter()
                    .filter(|(_, entry)| now > entry.expires_at)
                    .map(|(id, _)| id.clone())
                    .collect();
                
                for id in expired {
                    cache.remove(&id);
                    debug!("移除过期缓存条目: {}", id);
                }
                
                info!("缓存清理完成，当前缓存条目数: {}", cache.len());
            }
        });
        
        Ok(())
    }

    /// 手动刷新缓存
    pub async fn refresh_cache(&self) -> Result<()> {
        let identities = self.spire_client.get_valid_identities().await?;
        let mut cache = self.cache.write().await;
        
        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(self.ttl);
        
        // 更新缓存
        for identity in &identities {
            cache.insert(identity.id.clone(), CacheEntry {
                identity: identity.clone(),
                cached_at: now,
                expires_at,
            });
        }
        
        info!("手动刷新缓存完成，当前缓存条目数: {}", cache.len());
        Ok(())
    }
}

/// 创建默认的身份缓存
pub fn create_default_cache(spire_client: Arc<SpireClient>) -> IdentityCache {
    IdentityCache::new(spire_client, 3600) // 默认缓存 1 小时
}