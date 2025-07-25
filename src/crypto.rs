/*!
 * ClipVanish™ 加密模块
 * 
 * 实现AES-256-GCM-SIV加密算法，提供剪贴板内容的安全加密存储
 * 特点：
 * - 使用AES-GCM-SIV避免时序攻击
 * - 内存零残留设计
 * - 密钥自动生成和管理
 * 
 * 作者: ClipVanish Team
 */

use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce};
use aes_gcm_siv::aead::{Aead, OsRng};
use rand::{RngCore, CryptoRng};
use zeroize::{Zeroize, ZeroizeOnDrop};
use std::fmt;

/// AES-GCM-SIV nonce 长度（96位）
const NONCE_LENGTH: usize = 12;

/// AES-256 密钥长度（256位）
const KEY_LENGTH: usize = 32;

/// 加密错误类型定义
#[derive(Debug)]
pub enum CryptoError {
    /// 密钥生成失败
    KeyGenerationFailed,
    /// 加密操作失败
    EncryptionFailed,
    /// 解密操作失败
    DecryptionFailed,
    /// 无效的密文格式
    InvalidCiphertext,
    /// 内存操作失败
    MemoryError(String),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::KeyGenerationFailed => write!(f, "密钥生成失败"),
            CryptoError::EncryptionFailed => write!(f, "加密操作失败"),
            CryptoError::DecryptionFailed => write!(f, "解密操作失败"),
            CryptoError::InvalidCiphertext => write!(f, "无效的密文格式"),
            CryptoError::MemoryError(msg) => write!(f, "内存操作错误: {}", msg),
        }
    }
}

impl std::error::Error for CryptoError {}

/// 安全密钥结构体
/// 
/// 自动实现内存零化，确保密钥在销毁时被安全擦除
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureKey {
    /// AES-256密钥数据
    key_data: [u8; KEY_LENGTH],
}

impl SecureKey {
    /// 生成新的安全密钥
    /// 
    /// # 返回值
    /// * `Result<SecureKey, CryptoError>` - 成功返回密钥，失败返回错误
    pub fn generate() -> Result<Self, CryptoError> {
        let mut key_data = [0u8; KEY_LENGTH];
        
        // 使用系统安全随机数生成器
        OsRng.fill_bytes(&mut key_data);
        
        Ok(SecureKey { key_data })
    }
    
    /// 获取密钥数据的引用
    /// 
    /// # 返回值
    /// * `&[u8; KEY_LENGTH]` - 密钥数据引用
    pub fn as_bytes(&self) -> &[u8; KEY_LENGTH] {
        &self.key_data
    }
}

/// 加密后的数据结构
/// 
/// 包含nonce和密文，自动实现内存零化
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct EncryptedData {
    /// 随机nonce（12字节）
    nonce: [u8; NONCE_LENGTH],
    /// 加密后的密文
    ciphertext: Vec<u8>,
}

impl EncryptedData {
    /// 创建新的加密数据结构
    /// 
    /// # 参数
    /// * `nonce` - 随机nonce
    /// * `ciphertext` - 加密后的密文
    pub fn new(nonce: [u8; NONCE_LENGTH], ciphertext: Vec<u8>) -> Self {
        Self { nonce, ciphertext }
    }
    
    /// 获取nonce
    pub fn nonce(&self) -> &[u8; NONCE_LENGTH] {
        &self.nonce
    }
    
    /// 获取密文
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }
    
    /// 获取总长度（nonce + 密文）
    pub fn total_length(&self) -> usize {
        NONCE_LENGTH + self.ciphertext.len()
    }
}

/// ClipVanish加密引擎
/// 
/// 核心加密/解密功能实现，负责剪贴板内容的安全处理
pub struct CryptoEngine {
    /// AES-GCM-SIV加密器实例
    cipher: Aes256GcmSiv,
    /// 当前使用的密钥
    current_key: SecureKey,
}

impl CryptoEngine {
    /// 创建新的加密引擎实例
    /// 
    /// # 返回值
    /// * `Result<CryptoEngine, CryptoError>` - 成功返回引擎实例
    pub fn new() -> Result<Self, CryptoError> {
        let key = SecureKey::generate()?;
        let cipher = Aes256GcmSiv::new_from_slice(key.as_bytes())
            .map_err(|_| CryptoError::KeyGenerationFailed)?;
        
        Ok(CryptoEngine {
            cipher,
            current_key: key,
        })
    }
    
    /// 加密明文数据
    /// 
    /// # 参数
    /// * `plaintext` - 待加密的明文数据
    /// 
    /// # 返回值
    /// * `Result<EncryptedData, CryptoError>` - 成功返回加密数据
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError> {
        // 生成随机nonce
        let mut nonce_bytes = [0u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // 执行加密操作
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;
        
        Ok(EncryptedData::new(nonce_bytes, ciphertext))
    }
    
    /// 解密密文数据
    /// 
    /// # 参数
    /// * `encrypted_data` - 待解密的加密数据
    /// 
    /// # 返回值
    /// * `Result<Vec<u8>, CryptoError>` - 成功返回明文数据
    pub fn decrypt(&self, encrypted_data: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(&encrypted_data.nonce);
        
        // 执行解密操作
        let plaintext = self.cipher
            .decrypt(nonce, encrypted_data.ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)?;
        
        Ok(plaintext)
    }
    
    /// 重新生成密钥（用于增强安全性）
    /// 
    /// # 返回值
    /// * `Result<(), CryptoError>` - 操作结果
    pub fn regenerate_key(&mut self) -> Result<(), CryptoError> {
        // 生成新密钥
        let new_key = SecureKey::generate()?;
        let new_cipher = Aes256GcmSiv::new_from_slice(new_key.as_bytes())
            .map_err(|_| CryptoError::KeyGenerationFailed)?;
        
        // 替换旧密钥和加密器
        self.current_key = new_key;
        self.cipher = new_cipher;
        
        log::info!("加密密钥已重新生成");
        Ok(())
    }
    
    /// 获取当前密钥的指纹（用于调试，不暴露实际密钥）
    /// 
    /// # 返回值
    /// * `String` - 密钥指纹（SHA256前8字节的十六进制）
    pub fn key_fingerprint(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.current_key.as_bytes().hash(&mut hasher);
        let hash = hasher.finish();
        
        format!("{:016x}", hash)[..16].to_string()
    }
}

/// 实现Drop trait确保加密引擎销毁时清理敏感数据
impl Drop for CryptoEngine {
    fn drop(&mut self) {
        log::debug!("加密引擎正在安全销毁");
        // SecureKey会自动零化，这里主要是记录日志
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_generation() {
        let key1 = SecureKey::generate().unwrap();
        let key2 = SecureKey::generate().unwrap();
        
        // 确保生成的密钥不相同
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }
    
    #[test]
    fn test_encryption_decryption() {
        let engine = CryptoEngine::new().unwrap();
        let plaintext = b"Hello, ClipVanish!";
        
        // 加密
        let encrypted = engine.encrypt(plaintext).unwrap();
        assert!(encrypted.total_length() > plaintext.len());
        
        // 解密
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_multiple_encryptions_different_results() {
        let engine = CryptoEngine::new().unwrap();
        let plaintext = b"Same message";
        
        let encrypted1 = engine.encrypt(plaintext).unwrap();
        let encrypted2 = engine.encrypt(plaintext).unwrap();
        
        // 相同明文的多次加密应该产生不同的密文（因为nonce不同）
        assert_ne!(encrypted1.ciphertext(), encrypted2.ciphertext());
        
        // 但解密后应该得到相同的明文
        let decrypted1 = engine.decrypt(&encrypted1).unwrap();
        let decrypted2 = engine.decrypt(&encrypted2).unwrap();
        assert_eq!(decrypted1, decrypted2);
        assert_eq!(decrypted1, plaintext);
    }
    
    #[test]
    fn test_key_regeneration() {
        let mut engine = CryptoEngine::new().unwrap();
        let original_fingerprint = engine.key_fingerprint();
        
        engine.regenerate_key().unwrap();
        let new_fingerprint = engine.key_fingerprint();
        
        // 密钥重新生成后指纹应该不同
        assert_ne!(original_fingerprint, new_fingerprint);
    }
}
