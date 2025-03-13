//! AI 模块
//!
//! 该模块实现基于 GNN 的网络流量分析和策略生成功能。
//! 使用 tract-onnx 库进行模型推理，生成最小化策略规则。

mod gnn_model;
mod data_preprocessor;

pub use gnn_model::{GnnModel, ModelConfig, create_default_model};
pub use data_preprocessor::{DataPreprocessor, create_default_preprocessor};

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

/// 初始化 AI 系统
pub async fn init_ai_system() -> Result<Arc<GnnModel>> {
    info!("初始化 AI 系统");
    
    // 创建数据预处理器
    let preprocessor = create_default_preprocessor();
    
    // 创建 GNN 模型
    let model = Arc::new(create_default_model(preprocessor));
    
    // 预热模型
    model.warmup().await?;
    
    info!("AI 系统初始化完成");
    
    Ok(model)
}