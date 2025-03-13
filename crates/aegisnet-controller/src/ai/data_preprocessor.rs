//! 数据预处理模块
//!
//! 该模块负责对网络流量数据进行预处理，为 GNN 模型提供标准化的输入。
//! 包括特征提取、归一化和编码等功能。

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// 预处理配置
#[derive(Debug, Clone)]
pub struct PreprocessorConfig {
    /// 是否启用特征归一化
    pub normalize: bool,
    /// 是否启用 PCA 降维
    pub use_pca: bool,
    /// PCA 降维后的维度
    pub pca_dimensions: usize,
    /// 是否启用特征选择
    pub feature_selection: bool,
}

impl Default for PreprocessorConfig {
    fn default() -> Self {
        Self {
            normalize: true,
            use_pca: false,
            pca_dimensions: 32,
            feature_selection: true,
        }
    }
}

/// 数据预处理器
pub struct DataPreprocessor {
    /// 预处理配置
    config: PreprocessorConfig,
    /// 特征均值（用于归一化）
    feature_means: RwLock<Option<Vec<f32>>>,
    /// 特征标准差（用于归一化）
    feature_stds: RwLock<Option<Vec<f32>>>,
    /// PCA 变换矩阵
    pca_matrix: RwLock<Option<Vec<Vec<f32>>>>,
    /// 特征选择掩码
    feature_mask: RwLock<Option<Vec<bool>>>,
}

impl DataPreprocessor {
    /// 创建新的数据预处理器
    pub fn new(config: PreprocessorConfig) -> Self {
        Self {
            config,
            feature_means: RwLock::new(None),
            feature_stds: RwLock::new(None),
            pca_matrix: RwLock::new(None),
            feature_mask: RwLock::new(None),
        }
    }
    
    /// 初始化预处理器
    pub async fn init(&self) -> Result<()> {
        info!("初始化数据预处理器");
        
        // 初始化特征均值和标准差（用于归一化）
        if self.config.normalize {
            let mut means = self.feature_means.write().await;
            let mut stds = self.feature_stds.write().await;
            
            // 这里应该从训练数据中计算均值和标准差
            // 为了示例，使用默认值
            *means = Some(vec![0.0; 64]);
            *stds = Some(vec![1.0; 64]);
            
            debug!("特征归一化参数已初始化");
        }
        
        // 初始化 PCA 矩阵
        if self.config.use_pca {
            let mut pca = self.pca_matrix.write().await;
            
            // 这里应该从训练数据中计算 PCA 变换矩阵
            // 为了示例，使用单位矩阵
            let mut matrix = Vec::new();
            for i in 0..self.config.pca_dimensions {
                let mut row = vec![0.0; 64];
                if i < 64 {
                    row[i] = 1.0;
                }
                matrix.push(row);
            }
            
            *pca = Some(matrix);
            debug!("PCA 变换矩阵已初始化");
        }
        
        // 初始化特征选择掩码
        if self.config.feature_selection {
            let mut mask = self.feature_mask.write().await;
            
            // 这里应该从训练数据中计算特征重要性并选择重要特征
            // 为了示例，选择所有特征
            *mask = Some(vec![true; 64]);
            
            debug!("特征选择掩码已初始化");
        }
        
        info!("数据预处理器初始化完成");
        Ok(())
    }
    
    /// 预处理输入特征
    pub async fn preprocess(&self, features: &[f32]) -> Result<Vec<f32>> {
        debug!("预处理输入特征，原始维度: {}", features.len());
        
        // 确保预处理器已初始化
        if self.config.normalize && self.feature_means.read().await.is_none() {
            self.init().await?;
        }
        
        let mut processed = features.to_vec();
        
        // 应用特征选择
        if self.config.feature_selection {
            if let Some(mask) = self.feature_mask.read().await.as_ref() {
                processed = processed.iter()
                    .zip(mask.iter())
                    .filter_map(|(v, &selected)| if selected { Some(*v) } else { None })
                    .collect();
            }
        }
        
        // 应用归一化
        if self.config.normalize {
            if let (Some(means), Some(stds)) = (
                self.feature_means.read().await.as_ref(),
                self.feature_stds.read().await.as_ref()
            ) {
                for i in 0..processed.len().min(means.len()) {
                    if stds[i] > 1e-10 {
                        processed[i] = (processed[i] - means[i]) / stds[i];
                    }
                }
            }
        }
        
        // 应用 PCA 降维
        if self.config.use_pca {
            if let Some(pca) = self.pca_matrix.read().await.as_ref() {
                let mut pca_result = vec![0.0; pca.len()];
                
                for (i, row) in pca.iter().enumerate() {
                    for j in 0..processed.len().min(row.len()) {
                        pca_result[i] += processed[j] * row[j];
                    }
                }
                
                processed = pca_result;
            }
        }
        
        debug!("预处理完成，处理后维度: {}", processed.len());
        Ok(processed)
    }
    
    /// 从训练数据中学习预处理参数
    pub async fn fit(&self, training_data: &[Vec<f32>]) -> Result<()> {
        info!("从训练数据学习预处理参数，样本数: {}", training_data.len());
        
        if training_data.is_empty() {
            return Err(anyhow::anyhow!("训练数据为空"));
        }
        
        let feature_dim = training_data[0].len();
        
        // 计算特征均值和标准差
        if self.config.normalize {
            let mut means = vec![0.0; feature_dim];
            let mut stds = vec![0.0; feature_dim];
            
            // 计算均值
            for sample in training_data {
                for (i, &value) in sample.iter().enumerate() {
                    if i < feature_dim {
                        means[i] += value;
                    }
                }
            }
            
            for mean in &mut means {
                *mean /= training_data.len() as f32;
            }
            
            // 计算标准差
            for sample in training_data {
                for (i, &value) in sample.iter().enumerate() {
                    if i < feature_dim {
                        stds[i] += (value - means[i]).powi(2);
                    }
                }
            }
            
            for std in &mut stds {
                *std = (*std / training_data.len() as f32).sqrt();
                if *std < 1e-10 {
                    *std = 1.0; // 避免除以零
                }
            }
            
            // 更新预处理器参数
            *self.feature_means.write().await = Some(means);
            *self.feature_stds.write().await = Some(stds);
            
            debug!("特征归一化参数已更新");
        }
        
        // 这里应该实现 PCA 和特征选择的学习逻辑
        // 为了示例，使用默认值
        
        info!("预处理参数学习完成");
        Ok(())
    }
}

/// 创建默认的数据预处理器
pub fn create_default_preprocessor() -> Arc<DataPreprocessor> {
    Arc::new(DataPreprocessor::new(PreprocessorConfig::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_preprocessing() {
        let preprocessor = create_default_preprocessor();
        
        // 创建测试输入
        let features = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        // 初始化预处理器
        preprocessor.init().await.unwrap();
        
        // 执行预处理
        let processed = preprocessor.preprocess(&features).await.unwrap();
        
        // 验证输出
        assert!(!processed.is_empty());
    }
    
    #[tokio::test]
    async fn test_fit() {
        let preprocessor = create_default_preprocessor();
        
        // 创建训练数据
        let training_data = vec![
            vec![1.0, 2.0, 3.0],
            vec![2.0, 3.0, 4.0],
            vec![3.0, 4.0, 5.0],
        ];
        
        // 学习预处理参数
        preprocessor.fit(&training_data).await.unwrap();
        
        // 验证参数已更新
        if preprocessor.config.normalize {
            let means = preprocessor.feature_means.read().await;
            assert!(means.is_some());
        }
    }
}