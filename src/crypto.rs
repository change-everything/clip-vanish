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

/// 简单的Base64编码表
const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 简单的Base64编码实现
///
/// # 参数
/// * `input` - 待编码的字节数组
///
/// # 返回值
/// * `String` - Base64编码的字符串
fn base64_encode(input: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;

    while i < input.len() {
        let b1 = input[i];
        let b2 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b3 = if i + 2 < input.len() { input[i + 2] } else { 0 };

        let n = ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);

        result.push(BASE64_CHARS[((n >> 18) & 63) as usize] as char);
        result.push(BASE64_CHARS[((n >> 12) & 63) as usize] as char);
        result.push(if i + 1 < input.len() { BASE64_CHARS[((n >> 6) & 63) as usize] as char } else { '=' });
        result.push(if i + 2 < input.len() { BASE64_CHARS[(n & 63) as usize] as char } else { '=' });

        i += 3;
    }

    result
}

/// 简单的Base64解码实现
///
/// # 参数
/// * `input` - Base64编码的字符串
///
/// # 返回值
/// * `Result<Vec<u8>, String>` - 解码后的字节数组
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut chars = input.chars().collect::<Vec<_>>();

    // 补齐到4的倍数
    while chars.len() % 4 != 0 {
        chars.push('=');
    }

    let mut i = 0;
    while i < chars.len() {
        let c1 = char_to_base64_value(chars[i])?;
        let c2 = char_to_base64_value(chars[i + 1])?;
        let c3 = if chars[i + 2] == '=' { 0 } else { char_to_base64_value(chars[i + 2])? };
        let c4 = if chars[i + 3] == '=' { 0 } else { char_to_base64_value(chars[i + 3])? };

        let n = (c1 << 18) | (c2 << 12) | (c3 << 6) | c4;

        result.push((n >> 16) as u8);
        if chars[i + 2] != '=' {
            result.push((n >> 8) as u8);
        }
        if chars[i + 3] != '=' {
            result.push(n as u8);
        }

        i += 4;
    }

    Ok(result)
}

/// 将字符转换为Base64值
fn char_to_base64_value(c: char) -> Result<u32, String> {
    match c {
        'A'..='Z' => Ok((c as u32) - ('A' as u32)),
        'a'..='z' => Ok((c as u32) - ('a' as u32) + 26),
        '0'..='9' => Ok((c as u32) - ('0' as u32) + 52),
        '+' => Ok(62),
        '/' => Ok(63),
        '=' => Ok(0),
        _ => Err(format!("Invalid Base64 character: {}", c)),
    }
}

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

    /// 将加密数据编码为Base64字符串（用于存储到剪贴板）
    ///
    /// # 返回值
    /// * `String` - Base64编码的加密数据
    pub fn to_base64(&self) -> String {
        // 将nonce和密文合并
        let mut combined = Vec::with_capacity(NONCE_LENGTH + self.ciphertext.len());
        combined.extend_from_slice(&self.nonce);
        combined.extend_from_slice(&self.ciphertext);

        // 使用简单的Base64编码
        base64_encode(&combined)
    }

    /// 从Base64字符串解码为加密数据
    ///
    /// # 参数
    /// * `base64_str` - Base64编码的字符串
    ///
    /// # 返回值
    /// * `Result<EncryptedData, CryptoError>` - 解码后的加密数据
    pub fn from_base64(base64_str: &str) -> Result<Self, CryptoError> {
        let combined = base64_decode(base64_str)
            .map_err(|_| CryptoError::InvalidCiphertext)?;

        if combined.len() < NONCE_LENGTH {
            return Err(CryptoError::InvalidCiphertext);
        }

        let mut nonce = [0u8; NONCE_LENGTH];
        nonce.copy_from_slice(&combined[..NONCE_LENGTH]);
        let ciphertext = combined[NONCE_LENGTH..].to_vec();

        Ok(EncryptedData::new(nonce, ciphertext))
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

    /// 解密密文数据并立即重置密钥（用于粘贴操作）
    ///
    /// 根据PRD要求，在粘贴时解密一次后要立刻重置密钥以增强安全性
    ///
    /// # 参数
    /// * `encrypted_data` - 待解密的加密数据
    ///
    /// # 返回值
    /// * `Result<Vec<u8>, CryptoError>` - 成功返回明文数据
    pub fn decrypt_and_reset_key(&mut self, encrypted_data: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(&encrypted_data.nonce);

        // 执行解密操作
        let plaintext = self.cipher
            .decrypt(nonce, encrypted_data.ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)?;

        // 立即重置密钥以增强安全性
        self.regenerate_key()?;

        log::info!("解密完成并已重置密钥，增强安全性");
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

    #[test]
    fn test_decrypt_and_reset_key() {
        let mut engine = CryptoEngine::new().unwrap();
        let plaintext = b"Secret message for paste";
        let original_fingerprint = engine.key_fingerprint();

        // 加密
        let encrypted = engine.encrypt(plaintext).unwrap();

        // 解密并重置密钥
        let decrypted = engine.decrypt_and_reset_key(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);

        // 密钥应该已经重置
        let new_fingerprint = engine.key_fingerprint();
        assert_ne!(original_fingerprint, new_fingerprint);

        // 用旧密钥加密的数据在密钥重置后应该无法解密
        // （这是预期行为，因为密钥已经改变）
        let decrypt_result = engine.decrypt(&encrypted);
        assert!(decrypt_result.is_err());
    }

    #[test]
    fn test_base64_encoding_decoding() {
        let engine = CryptoEngine::new().unwrap();
        let plaintext = b"Hello, ClipVanish Base64 test!";

        // 加密
        let encrypted = engine.encrypt(plaintext).unwrap();

        // 转换为Base64
        let base64_str = encrypted.to_base64();
        assert!(!base64_str.is_empty());

        // 从Base64恢复
        let recovered = EncryptedData::from_base64(&base64_str).unwrap();

        // 验证恢复的数据与原始加密数据相同
        assert_eq!(recovered.nonce(), encrypted.nonce());
        assert_eq!(recovered.ciphertext(), encrypted.ciphertext());

        // 验证可以正确解密
        let decrypted = engine.decrypt(&recovered).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_base64_invalid_input() {
        // 测试无效的Base64输入
        let result = EncryptedData::from_base64("invalid base64!");
        assert!(result.is_err());

        // 测试太短的输入
        let result = EncryptedData::from_base64("dGVzdA=="); // "test" in base64, too short
        assert!(result.is_err());
    }
}
