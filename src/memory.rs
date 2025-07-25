/*!
 * ClipVanish™ 内存管理模块
 * 
 * 实现安全的内存管理功能，确保敏感数据的物理级清除
 * 特点：
 * - 内存锁定防止swap泄露
 * - 安全的内存零化
 * - 多重覆盖擦除
 * - 跨平台内存保护
 * 
 * 作者: ClipVanish Team
 */

use std::ptr;
use std::slice;
use log::{info, warn, debug, error};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(windows)]
use winapi::um::{
    memoryapi::{VirtualLock, VirtualUnlock},
    processthreadsapi::GetCurrentProcess,
    winnt::{HANDLE, PAGE_READWRITE},
};

#[cfg(unix)]
use libc::{mlock, munlock, getpagesize};

/// 内存管理错误类型
#[derive(Debug)]
pub enum MemoryError {
    /// 内存锁定失败
    LockFailed(String),
    /// 内存解锁失败
    UnlockFailed(String),
    /// 内存分配失败
    AllocationFailed,
    /// 无效的内存地址
    InvalidAddress,
    /// 系统不支持该操作
    UnsupportedOperation,
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::LockFailed(msg) => write!(f, "内存锁定失败: {}", msg),
            MemoryError::UnlockFailed(msg) => write!(f, "内存解锁失败: {}", msg),
            MemoryError::AllocationFailed => write!(f, "内存分配失败"),
            MemoryError::InvalidAddress => write!(f, "无效的内存地址"),
            MemoryError::UnsupportedOperation => write!(f, "系统不支持该操作"),
        }
    }
}

impl std::error::Error for MemoryError {}

/// 安全内存块
/// 
/// 自动管理的安全内存区域，支持锁定和安全擦除
#[derive(Debug)]
pub struct SecureMemoryBlock {
    /// 内存指针
    ptr: *mut u8,
    /// 内存大小
    size: usize,
    /// 是否已锁定
    is_locked: bool,
    /// 是否已分配
    is_allocated: bool,
}

impl SecureMemoryBlock {
    /// 分配新的安全内存块
    /// 
    /// # 参数
    /// * `size` - 内存块大小（字节）
    /// 
    /// # 返回值
    /// * `Result<SecureMemoryBlock, MemoryError>` - 成功返回内存块
    pub fn allocate(size: usize) -> Result<Self, MemoryError> {
        if size == 0 {
            return Err(MemoryError::AllocationFailed);
        }
        
        // 分配内存
        let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>())
            .map_err(|_| MemoryError::AllocationFailed)?;
        
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        
        if ptr.is_null() {
            return Err(MemoryError::AllocationFailed);
        }
        
        debug!("分配安全内存块，大小: {} 字节", size);
        
        Ok(SecureMemoryBlock {
            ptr,
            size,
            is_locked: false,
            is_allocated: true,
        })
    }
    
    /// 锁定内存防止swap
    /// 
    /// # 返回值
    /// * `Result<(), MemoryError>` - 操作结果
    pub fn lock(&mut self) -> Result<(), MemoryError> {
        if !self.is_allocated {
            return Err(MemoryError::InvalidAddress);
        }
        
        if self.is_locked {
            debug!("内存块已经锁定");
            return Ok(());
        }
        
        let result = self.platform_lock();
        
        match result {
            Ok(_) => {
                self.is_locked = true;
                debug!("内存块锁定成功，大小: {} 字节", self.size);
                Ok(())
            },
            Err(e) => {
                warn!("内存块锁定失败: {}", e);
                Err(e)
            }
        }
    }
    
    /// 解锁内存
    /// 
    /// # 返回值
    /// * `Result<(), MemoryError>` - 操作结果
    pub fn unlock(&mut self) -> Result<(), MemoryError> {
        if !self.is_locked {
            debug!("内存块未锁定");
            return Ok(());
        }
        
        let result = self.platform_unlock();
        
        match result {
            Ok(_) => {
                self.is_locked = false;
                debug!("内存块解锁成功");
                Ok(())
            },
            Err(e) => {
                warn!("内存块解锁失败: {}", e);
                Err(e)
            }
        }
    }
    
    /// 获取内存块的可变切片
    /// 
    /// # 返回值
    /// * `&mut [u8]` - 内存块切片
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if !self.is_allocated {
            panic!("尝试访问未分配的内存块");
        }
        
        unsafe { slice::from_raw_parts_mut(self.ptr, self.size) }
    }
    
    /// 获取内存块的不可变切片
    /// 
    /// # 返回值
    /// * `&[u8]` - 内存块切片
    pub fn as_slice(&self) -> &[u8] {
        if !self.is_allocated {
            panic!("尝试访问未分配的内存块");
        }
        
        unsafe { slice::from_raw_parts(self.ptr, self.size) }
    }
    
    /// 安全擦除内存内容
    /// 
    /// 使用多种模式覆盖内存确保数据无法恢复
    pub fn secure_erase(&mut self) {
        if !self.is_allocated {
            return;
        }
        
        debug!("开始安全擦除内存块，大小: {} 字节", self.size);
        
        let slice = self.as_mut_slice();
        
        // 第一轮：全零覆盖
        slice.zeroize();
        
        // 第二轮：全1覆盖
        unsafe {
            ptr::write_bytes(self.ptr, 0xFF, self.size);
        }
        
        // 第三轮：随机数据覆盖
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        rng.fill_bytes(slice);
        
        // 第四轮：再次零覆盖
        slice.zeroize();
        
        // 确保编译器不会优化掉这些操作
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        
        debug!("内存块安全擦除完成");
    }
    
    /// 获取内存块大小
    /// 
    /// # 返回值
    /// * `usize` - 内存块大小
    pub fn size(&self) -> usize {
        self.size
    }
    
    /// 检查内存块是否已锁定
    /// 
    /// # 返回值
    /// * `bool` - 是否已锁定
    pub fn is_locked(&self) -> bool {
        self.is_locked
    }
    
    /// 平台特定的内存锁定实现
    #[cfg(windows)]
    fn platform_lock(&self) -> Result<(), MemoryError> {
        let result = unsafe {
            VirtualLock(self.ptr as *mut _, self.size)
        };
        
        if result == 0 {
            let error_code = unsafe { winapi::um::errhandlingapi::GetLastError() };
            Err(MemoryError::LockFailed(format!("Windows错误码: {}", error_code)))
        } else {
            Ok(())
        }
    }
    
    /// 平台特定的内存解锁实现
    #[cfg(windows)]
    fn platform_unlock(&self) -> Result<(), MemoryError> {
        let result = unsafe {
            VirtualUnlock(self.ptr as *mut _, self.size)
        };
        
        if result == 0 {
            let error_code = unsafe { winapi::um::errhandlingapi::GetLastError() };
            Err(MemoryError::UnlockFailed(format!("Windows错误码: {}", error_code)))
        } else {
            Ok(())
        }
    }
    
    /// Unix/Linux平台的内存锁定实现
    #[cfg(unix)]
    fn platform_lock(&self) -> Result<(), MemoryError> {
        let result = unsafe {
            mlock(self.ptr as *const _, self.size)
        };
        
        if result != 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(MemoryError::LockFailed(format!("errno: {}", errno)))
        } else {
            Ok(())
        }
    }
    
    /// Unix/Linux平台的内存解锁实现
    #[cfg(unix)]
    fn platform_unlock(&self) -> Result<(), MemoryError> {
        let result = unsafe {
            munlock(self.ptr as *const _, self.size)
        };
        
        if result != 0 {
            let errno = unsafe { *libc::__errno_location() };
            Err(MemoryError::UnlockFailed(format!("errno: {}", errno)))
        } else {
            Ok(())
        }
    }
}

/// 实现Drop trait确保内存安全释放
impl Drop for SecureMemoryBlock {
    fn drop(&mut self) {
        if self.is_allocated {
            debug!("释放安全内存块");
            
            // 安全擦除内存
            self.secure_erase();
            
            // 解锁内存
            if self.is_locked {
                if let Err(e) = self.unlock() {
                    error!("释放时解锁内存失败: {}", e);
                }
            }
            
            // 释放内存
            let layout = std::alloc::Layout::from_size_align(self.size, std::mem::align_of::<u8>())
                .expect("无效的内存布局");
            
            unsafe {
                std::alloc::dealloc(self.ptr, layout);
            }
            
            self.is_allocated = false;
        }
    }
}

/// 安全内存工具类
/// 
/// 提供全局的内存安全操作功能
pub struct SecureMemory;

impl SecureMemory {
    /// 执行全局安全内存清理
    /// 
    /// 尝试清理可能残留的敏感数据
    pub fn secure_zero_memory() {
        debug!("执行全局安全内存清理");
        
        // 强制垃圾回收（如果可用）
        // 注意：Rust没有显式的GC，但我们可以尝试一些清理操作
        
        // 清理栈上的敏感数据
        Self::clear_stack_memory();
        
        // 内存屏障确保操作不被优化
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        
        info!("全局安全内存清理完成");
    }
    
    /// 清理栈内存
    fn clear_stack_memory() {
        // 在栈上分配一块内存并清零，尝试覆盖可能的敏感数据残留
        const STACK_CLEAR_SIZE: usize = 4096; // 4KB
        let mut stack_buffer = [0u8; STACK_CLEAR_SIZE];
        
        // 使用随机数据填充
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut stack_buffer);
        
        // 再次清零
        stack_buffer.zeroize();
        
        // 防止编译器优化
        std::hint::black_box(&stack_buffer);
    }
    
    /// 获取系统页面大小
    /// 
    /// # 返回值
    /// * `usize` - 系统页面大小
    #[cfg(unix)]
    pub fn get_page_size() -> usize {
        unsafe { getpagesize() as usize }
    }
    
    /// 获取系统页面大小（Windows版本）
    /// 
    /// # 返回值
    /// * `usize` - 系统页面大小
    #[cfg(windows)]
    pub fn get_page_size() -> usize {
        use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};
        
        let mut sys_info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
        unsafe {
            GetSystemInfo(&mut sys_info);
        }
        
        sys_info.dwPageSize as usize
    }
    
    /// 检查系统是否支持内存锁定
    /// 
    /// # 返回值
    /// * `bool` - 是否支持内存锁定
    pub fn supports_memory_locking() -> bool {
        // 尝试分配并锁定一小块内存来测试支持情况
        match SecureMemoryBlock::allocate(4096) {
            Ok(mut block) => {
                match block.lock() {
                    Ok(_) => {
                        debug!("系统支持内存锁定");
                        true
                    },
                    Err(e) => {
                        warn!("系统不支持内存锁定: {}", e);
                        false
                    }
                }
            },
            Err(_) => {
                warn!("无法分配测试内存块");
                false
            }
        }
    }
    
    /// 获取内存使用统计信息
    /// 
    /// # 返回值
    /// * `MemoryStats` - 内存统计信息
    pub fn get_memory_stats() -> MemoryStats {
        MemoryStats {
            page_size: Self::get_page_size(),
            supports_locking: Self::supports_memory_locking(),
        }
    }
}

/// 内存统计信息
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// 系统页面大小
    pub page_size: usize,
    /// 是否支持内存锁定
    pub supports_locking: bool,
}

/// 安全字符串类型
/// 
/// 自动实现内存零化的字符串类型，用于存储敏感信息
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureString {
    /// 内部数据
    data: String,
}

impl SecureString {
    /// 创建新的安全字符串
    /// 
    /// # 参数
    /// * `s` - 字符串内容
    /// 
    /// # 返回值
    /// * `SecureString` - 安全字符串实例
    pub fn new(s: String) -> Self {
        SecureString { data: s }
    }
    
    /// 从字符串切片创建安全字符串
    /// 
    /// # 参数
    /// * `s` - 字符串切片
    /// 
    /// # 返回值
    /// * `SecureString` - 安全字符串实例
    pub fn from_str(s: &str) -> Self {
        SecureString { data: s.to_string() }
    }
    
    /// 获取字符串内容的引用
    /// 
    /// # 返回值
    /// * `&str` - 字符串内容引用
    pub fn as_str(&self) -> &str {
        &self.data
    }
    
    /// 获取字符串长度
    /// 
    /// # 返回值
    /// * `usize` - 字符串长度
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// 检查字符串是否为空
    /// 
    /// # 返回值
    /// * `bool` - 是否为空
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl std::fmt::Display for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[SECURE_STRING:{}bytes]", self.len())
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureString {{ len: {} }}", self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_secure_memory_allocation() {
        let mut block = SecureMemoryBlock::allocate(1024).unwrap();
        assert_eq!(block.size(), 1024);
        assert!(!block.is_locked());
        
        // 测试写入和读取
        let slice = block.as_mut_slice();
        slice[0] = 42;
        assert_eq!(slice[0], 42);
    }
    
    #[test]
    fn test_memory_locking() {
        let mut block = SecureMemoryBlock::allocate(4096).unwrap();
        
        // 尝试锁定内存
        let lock_result = block.lock();
        
        // 在某些环境下可能不支持内存锁定，这是正常的
        match lock_result {
            Ok(_) => {
                assert!(block.is_locked());
                
                // 测试解锁
                block.unlock().unwrap();
                assert!(!block.is_locked());
            },
            Err(e) => {
                println!("内存锁定不支持（这在某些环境下是正常的）: {}", e);
            }
        }
    }
    
    #[test]
    fn test_secure_erase() {
        let mut block = SecureMemoryBlock::allocate(100).unwrap();
        
        // 写入一些数据
        let slice = block.as_mut_slice();
        for i in 0..100 {
            slice[i] = (i % 256) as u8;
        }
        
        // 验证数据写入成功
        assert_eq!(slice[0], 0);
        assert_eq!(slice[50], 50);
        
        // 执行安全擦除
        block.secure_erase();
        
        // 验证数据被清零
        let slice = block.as_slice();
        for &byte in slice {
            assert_eq!(byte, 0);
        }
    }
    
    #[test]
    fn test_secure_string() {
        let secure_str = SecureString::from_str("sensitive data");
        assert_eq!(secure_str.as_str(), "sensitive data");
        assert_eq!(secure_str.len(), 14);
        assert!(!secure_str.is_empty());
        
        // 测试Display trait不会泄露内容
        let display_str = format!("{}", secure_str);
        assert!(!display_str.contains("sensitive"));
        assert!(display_str.contains("SECURE_STRING"));
    }
    
    #[test]
    fn test_memory_stats() {
        let stats = SecureMemory::get_memory_stats();
        assert!(stats.page_size > 0);
        println!("页面大小: {} 字节", stats.page_size);
        println!("支持内存锁定: {}", stats.supports_locking);
    }
    
    #[test]
    fn test_secure_zero_memory() {
        // 这个测试主要确保函数不会崩溃
        SecureMemory::secure_zero_memory();
    }
}
