use super::{CollectorOutput, ResourceCollector, ResourceSnapshot};
use super::cpu::CpuCollector;
use super::disk::DiskCollector;
use super::gpu::GpuCollector;
use crate::multi_agent::MultiAgentDetector;
use crate::thermal::ThermalCollector;

/// 采集器注册中心：管理所有启用的 Collector
/// 每个 Collector 返回独立结果，Registry 负责组装成 ResourceSnapshot（CR-01）
pub struct CollectorRegistry {
    collectors: Vec<Box<dyn ResourceCollector>>,
    /// 模型缓存路径（用于 DiskCollector，支持运行时设置）
    model_cache_path: Option<String>,
    /// 额外 Agent 进程名（从配置读取）
    extra_agent_processes: Option<Vec<String>>,
}

impl CollectorRegistry {
    /// 创建新 Registry，根据平台注册默认 Collector
    pub fn new() -> Self {
        let mut registry = Self {
            collectors: Vec::new(),
            model_cache_path: None,
            extra_agent_processes: None,
        };
        registry.register_defaults();
        registry
    }

    /// 设置模型缓存目录路径
    pub fn set_directories(&mut self, model_cache_path: Option<String>) {
        self.model_cache_path = model_cache_path;
    }

    /// 设置额外 Agent 进程名列表
    pub fn set_extra_agent_processes(&mut self, extra: Option<Vec<String>>) {
        self.extra_agent_processes = extra;
    }

    /// 注册一个 Collector
    pub fn register(&mut self, collector: Box<dyn ResourceCollector>) {
        self.collectors.push(collector);
    }

    /// 注册默认 Collector（根据平台）
    fn register_defaults(&mut self) {
        #[cfg(target_os = "linux")]
        self.register(Box::new(super::linux::LinuxCollector));
        #[cfg(target_os = "macos")]
        self.register(Box::new(super::macos::MacosCollector));
        #[cfg(not(any(target_os = "linux", target_os = "macos")))] {
            self.register(Box::new(UnsupportedCollector));
        }

        self.register(Box::new(CpuCollector));
        self.register(Box::new(GpuCollector));
        self.register(Box::new(ThermalCollector));
        // MultiAgentDetector 在 collect_all 前动态创建（依赖配置中的 extra 列表）
    }

    /// 串行采集所有 Collector，组装成 ResourceSnapshot
    pub fn collect_all(&self) -> ResourceSnapshot {
        let start = std::time::Instant::now();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let mut memory = None;
        let mut disk = None;
        let mut cpu = None;
        let mut gpu = None;
        let mut thermal = None;
        let mut agents = None;

        for collector in &self.collectors {
            match collector.collect() {
                Ok(CollectorOutput::Memory(m)) => memory = Some(m),
                Ok(CollectorOutput::Cpu(c)) => cpu = Some(c),
                Ok(CollectorOutput::Gpu(g)) => gpu = Some(g),
                Ok(CollectorOutput::Disk(d)) => disk = Some(d),
                Ok(CollectorOutput::Thermal(t)) => thermal = Some(t),
                Ok(CollectorOutput::Agent(a)) => agents = Some(a),
                Err(e) => {
                    eprintln!("Warning: collector failed: {}", e);
                }
            }
        }

        // 动态创建 DiskCollector
        let disk_collector = DiskCollector::new(self.model_cache_path.clone());
        match disk_collector.collect() {
            Ok(CollectorOutput::Disk(d)) => disk = Some(d),
            Err(e) => eprintln!("Warning: disk collector failed: {}", e),
            _ => {}
        }

        // 动态创建 MultiAgentDetector（依赖配置）
        let agent_detector = MultiAgentDetector::new(self.extra_agent_processes.clone());
        match agent_detector.collect() {
            Ok(CollectorOutput::Agent(a)) => agents = Some(a),
            Err(e) => eprintln!("Warning: agent detector failed: {}", e),
            _ => {}
        }

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        ResourceSnapshot {
            memory,
            disk,
            cpu,
            gpu,
            thermal,
            agents,
            timestamp,
            collection_duration_ms: (duration_ms * 10.0).round() / 10.0,
        }
    }
}

/// 不支持的平台（兜底）
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub struct UnsupportedCollector;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl ResourceCollector for UnsupportedCollector {
    fn collect(&self) -> Result<super::CollectorOutput, super::CollectError> {
        Err(super::CollectError::UnsupportedPlatform)
    }
}
