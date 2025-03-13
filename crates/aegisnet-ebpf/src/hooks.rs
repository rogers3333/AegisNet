//! eBPF 程序挂载点与钩子逻辑
//!
//! 该模块定义了 eBPF 程序的挂载点和钩子函数，用于拦截网络流量和执行安全策略。
//! 包括 XDP、TC、Socket 等多种挂载点的实现。

use aya::programs::{Xdp, XdpFlags, SchedClassifier, SchedClassifierLink, LinkType};
use aya::Bpf;
use anyhow::Result;
use std::path::Path;

/// 挂载点类型
pub enum HookPoint {
    /// XDP 挂载点（网络接口入口）
    Xdp,
    /// TC 挂载点（流量控制）
    Tc,
    /// Socket 挂载点（套接字操作）
    Socket,
    /// Kprobe 挂载点（内核函数钩子）
    Kprobe,
}

/// 钩子管理器
pub struct HookManager {
    /// eBPF 程序实例
    bpf: Bpf,
    /// 已加载的 XDP 程序
    xdp_programs: Vec<Xdp>,
    /// 已加载的 TC 程序
    tc_programs: Vec<SchedClassifier>,
}

impl HookManager {
    /// 创建新的钩子管理器
    pub fn new(bpf_path: &Path) -> Result<Self> {
        // 加载 eBPF 程序
        let bpf = Bpf::load_file(bpf_path)?;
        
        Ok(Self {
            bpf,
            xdp_programs: Vec::new(),
            tc_programs: Vec::new(),
        })
    }
    
    /// 在指定网络接口上挂载 XDP 程序
    pub fn attach_xdp(&mut self, interface: &str, program_name: &str) -> Result<()> {
        // 获取 XDP 程序
        let program = self.bpf.program_mut(program_name)?
            .try_into::<Xdp>()?;
        
        // 挂载到网络接口
        program.attach(interface, XdpFlags::default())?;
        
        // 保存程序引用
        self.xdp_programs.push(program);
        
        Ok(())
    }
    
    /// 在指定网络接口上挂载 TC 程序
    pub fn attach_tc(&mut self, interface: &str, program_name: &str) -> Result<()> {
        // 获取 TC 程序
        let program = self.bpf.program_mut(program_name)?
            .try_into::<SchedClassifier>()?;
        
        // 创建 TC 链接
        let link = program.attach(interface, LinkType::Egress)?;
        
        // 保存程序引用
        self.tc_programs.push(program);
        
        Ok(())
    }
    
    /// 卸载所有钩子
    pub fn detach_all(&mut self) -> Result<()> {
        // XDP 程序会在 Drop 时自动卸载
        self.xdp_programs.clear();
        self.tc_programs.clear();
        
        Ok(())
    }
    
    /// 获取 eBPF 程序实例
    pub fn get_bpf(&self) -> &Bpf {
        &self.bpf
    }
    
    /// 获取可变 eBPF 程序实例
    pub fn get_bpf_mut(&mut self) -> &mut Bpf {
        &mut self.bpf
    }
}

/// 创建默认的钩子管理器
pub fn create_default_hook_manager(bpf_path: &Path) -> Result<HookManager> {
    HookManager::new(bpf_path)
}