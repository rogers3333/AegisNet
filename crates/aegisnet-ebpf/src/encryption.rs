//! Noise 协议加密模块
//!
//! 该模块实现基于 Noise 协议的网络通信加密，用于保护服务间通信安全。
//! 支持多种加密算法和密钥交换方式，适用于不同安全级别需求。

use anyhow::Result;
use snow::{Builder, HandshakeState, TransportState};
use std::sync::{Arc, Mutex};

/// 加密会话状态
#[derive(Debug)]
pub enum SessionState {
    /// 握手阶段
    Handshake(HandshakeState),
    /// 传输阶段
    Transport(TransportState),
    /// 会话关闭
    Closed,
}

/// Noise 协议加密器
pub struct NoiseEncryptor {
    /// 当前会话状态
    state: Arc<Mutex<SessionState>>,
    /// 协议参数
    pattern: String,
}

impl NoiseEncryptor {
    /// 创建新的加密器实例
    pub fn new(pattern: &str) -> Result<Self> {
        // 初始化为握手状态
        let builder = Builder::new(pattern.parse()?)
            .local_private_key(&[0u8; 32]) // 示例密钥，实际应从安全存储获取
            .build_initiator()?;

        Ok(Self {
            state: Arc::new(Mutex::new(SessionState::Handshake(builder))),
            pattern: pattern.to_string(),
        })
    }

    /// 加密数据
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut state = self.state.lock().unwrap();
        let mut output = vec![0u8; plaintext.len() + 16]; // 预留认证标签空间

        match &mut *state {
            SessionState::Transport(transport) => {
                let n = transport.write_message(plaintext, &mut output)?;
                output.truncate(n);
                Ok(output)
            }
            _ => Err(anyhow::anyhow!("加密器不在传输状态"))
        }
    }

    /// 解密数据
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut state = self.state.lock().unwrap();
        let mut output = vec![0u8; ciphertext.len()];

        match &mut *state {
            SessionState::Transport(transport) => {
                let n = transport.read_message(ciphertext, &mut output)?;
                output.truncate(n);
                Ok(output)
            }
            _ => Err(anyhow::anyhow!("加密器不在传输状态"))
        }
    }

    /// 执行握手过程
    pub fn perform_handshake(&self, message: &[u8]) -> Result<Vec<u8>> {
        let mut state = self.state.lock().unwrap();
        let mut output = vec![0u8; message.len() + 16];

        match &mut *state {
            SessionState::Handshake(handshake) => {
                let n = handshake.write_message(message, &mut output)?;
                output.truncate(n);

                // 如果握手完成，转换为传输状态
                if handshake.is_handshake_finished() {
                    let transport = handshake.into_transport_mode()?;
                    *state = SessionState::Transport(transport);
                }

                Ok(output)
            }
            _ => Err(anyhow::anyhow!("加密器不在握手状态"))
        }
    }
}

/// 创建默认的加密器
pub fn create_default_encryptor() -> Result<NoiseEncryptor> {
    // 使用 Noise_XX_25519_ChaChaPoly_BLAKE2s 模式
    NoiseEncryptor::new("Noise_XX_25519_ChaChaPoly_BLAKE2s")
}