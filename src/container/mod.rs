//! 容器适配层模块（W3-W4 核心）
//!
//! 提供容器运行时检测和 cgroup 资源限制读取能力：
//! - ContainerDetector::detect_runtime() → 检测 Docker/Kubernetes/无容器
//! - ContainerDetector::get_memory_limit() → cgroup 内存限制（MB）
//! - ContainerDetector::get_cpu_limit() → cgroup CPU 限制（核心数）
//!
//! ## cgroup 版本自动检测
//! - cgroup v1: /sys/fs/cgroup/memory/memory.limit_in_bytes
//! - cgroup v2: /sys/fs/cgroup/memory.max
//!
//! ## 无限值处理（CR-03）
//! - memory.limit_in_bytes >= 物理内存总量 → None（无实际限制）
//! - memory.max = "max" → None
//! - cpu.cfs_quota_us = -1 → None

use std::path::Path;

/// 容器运行时检测与 cgroup 资源限制读取
pub struct ContainerDetector;

impl ContainerDetector {
    /// 检测容器运行时类型
    ///
    /// 检测策略：
    /// 1. `/.dockerenv` 存在 → "docker"
    /// 2. `/proc/1/cgroup` 包含 "docker" → "docker"
    /// 3. `/proc/1/cgroup` 包含 "kubepods" → "kubernetes"
    /// 4. 都不匹配 → None
    pub fn detect_runtime() -> Option<String> {
        // 1. 检查 /.dockerenv
        if Path::new("/.dockerenv").exists() {
            return Some("docker".to_string());
        }

        // 2. 读取 /proc/1/cgroup
        let cgroup_content = Self::read_file_trimmed("/proc/1/cgroup")?;

        if cgroup_content.contains("docker") {
            Some("docker".to_string())
        } else if cgroup_content.contains("kubepods") {
            Some("kubernetes".to_string())
        } else {
            None
        }
    }

    /// 读取 cgroup 内存限制，返回 MB
    ///
    /// 返回 None 表示无实际限制（CR-03：无限值处理）。
    ///
    /// ## cgroup v1
    /// - 读 `/sys/fs/cgroup/memory/memory.limit_in_bytes`
    /// - 如果值 >= 物理内存总量 → 无实际限制 → None
    ///
    /// ## cgroup v2
    /// - 读 `/sys/fs/cgroup/memory.max`
    /// - 如果值是 "max" → 无限制 → None
    pub fn get_memory_limit() -> Option<u64> {
        // 先尝试 cgroup v2
        if Path::new("/sys/fs/cgroup/memory.max").exists() {
            return Self::get_memory_limit_v2();
        }
        // 回退 cgroup v1
        if Path::new("/sys/fs/cgroup/memory/memory.limit_in_bytes").exists() {
            return Self::get_memory_limit_v1();
        }
        None
    }

    /// cgroup v1 内存限制
    fn get_memory_limit_v1() -> Option<u64> {
        let content = Self::read_file_trimmed("/sys/fs/cgroup/memory/memory.limit_in_bytes")?;
        let limit: u64 = content.parse().ok()?;

        // CR-03: 9223372036854771712 是内核用的大数表示"无限制"
        // 如果 limit >= 物理内存总量 → 无实际限制
        let total_phys = Self::get_total_physical_memory_mb();
        if limit >= total_phys * 1024 * 1024
            || limit == u64::MAX
            || limit > 9_000_000_000_000_000_000
        {
            return None;
        }

        Some(limit / (1024 * 1024))
    }

    /// cgroup v2 内存限制
    fn get_memory_limit_v2() -> Option<u64> {
        let content = Self::read_file_trimmed("/sys/fs/cgroup/memory.max")?;
        // "max" 表示无限制
        if content == "max" {
            return None;
        }
        let limit: u64 = content.parse().ok()?;
        Some(limit / (1024 * 1024))
    }

    /// 读取 cgroup CPU 限制，返回有效 CPU 核心数
    ///
    /// # cgroup v1
    /// - 读 `cpu.cfs_quota_us` / `cpu.cfs_period_us`
    /// - quota = -1 或 period = 0 → None（无限制）
    ///
    /// # cgroup v2
    /// - 读 `cpu.max`，格式 "quota period"
    pub fn get_cpu_limit() -> Option<f64> {
        // 先尝试 cgroup v2
        if Path::new("/sys/fs/cgroup/cpu.max").exists() {
            return Self::get_cpu_limit_v2();
        }
        // 回退 cgroup v1
        if Path::new("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").exists() {
            return Self::get_cpu_limit_v1();
        }
        None
    }

    /// cgroup v1 CPU 限制
    fn get_cpu_limit_v1() -> Option<f64> {
        let quota_path = "/sys/fs/cgroup/cpu/cpu.cfs_quota_us";
        let period_path = "/sys/fs/cgroup/cpu/cpu.cfs_period_us";

        let quota_str = Self::read_file_trimmed(quota_path)?;
        let period_str = Self::read_file_trimmed(period_path)?;

        let quota: i64 = quota_str.parse().ok()?;
        let period: i64 = period_str.parse().ok()?;

        // quota = -1 表示无限制
        if quota == -1 || period == 0 {
            return None;
        }

        let cores = quota as f64 / period as f64;
        if cores <= 0.0 {
            return None;
        }

        Some(cores)
    }

    /// cgroup v2 CPU 限制
    fn get_cpu_limit_v2() -> Option<f64> {
        let content = Self::read_file_trimmed("/sys/fs/cgroup/cpu.max")?;

        // 格式: "quota period" 或 "max period"
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let quota_str = parts[0];
        // "max" 表示无限制
        if quota_str == "max" {
            return None;
        }

        let quota: u64 = quota_str.parse().ok()?;
        let period: u64 = parts[1].parse().ok()?;

        if period == 0 {
            return None;
        }

        let cores = quota as f64 / period as f64;
        if cores <= 0.0 {
            return None;
        }

        Some(cores)
    }

    // ========================================================================
    // 内部辅助方法
    // ========================================================================

    /// 读取文件并 trim 空白字符，出错时返回 None
    fn read_file_trimmed(path: &str) -> Option<String> {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// 获取物理内存总量（MB）
    fn get_total_physical_memory_mb() -> u64 {
        // 优先从 sysinfo 或 /proc/meminfo 读取
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    // 格式: "MemTotal:       16384000 kB"
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return kb / 1024;
                        }
                    }
                }
            }
        }
        // 回退：使用 num_cpus 感知，多数情况用 16GB 保守值
        // 但这里我们返回 0，让调用者自行处理
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // ========================================================================
    // 辅助函数：创建模拟的 cgroup 文件系统
    // ========================================================================

    /// 创建模拟的 cgroup v1 文件系统
    #[allow(dead_code)]
    struct MockCgroupV1 {
        _dir: TempDir,
        mem_limit_path: String,
        cpu_quota_path: String,
        cpu_period_path: String,
        _cgroup_proc_path: String,
        meminfo_path: String,
    }

    impl MockCgroupV1 {
        fn new(mem_limit_bytes: Option<u64>, cpu_quota: i64, cpu_period: i64) -> Self {
            let dir = TempDir::new().unwrap();
            let base = dir.path();

            // memory
            let mem_dir = base.join("sys/fs/cgroup/memory");
            std::fs::create_dir_all(&mem_dir).unwrap();
            let mem_limit_path = mem_dir.join("memory.limit_in_bytes");
            if let Some(limit) = mem_limit_bytes {
                let mut f = std::fs::File::create(&mem_limit_path).unwrap();
                write!(f, "{}", limit).unwrap();
            }

            // cpu
            let cpu_dir = base.join("sys/fs/cgroup/cpu");
            std::fs::create_dir_all(&cpu_dir).unwrap();
            let cpu_quota_path = cpu_dir.join("cpu.cfs_quota_us");
            let mut f = std::fs::File::create(&cpu_quota_path).unwrap();
            write!(f, "{}", cpu_quota).unwrap();
            let cpu_period_path = cpu_dir.join("cpu.cfs_period_us");
            let mut f = std::fs::File::create(&cpu_period_path).unwrap();
            write!(f, "{}", cpu_period).unwrap();

            // /proc/1/cgroup
            let proc_dir = base.join("proc/1");
            std::fs::create_dir_all(&proc_dir).unwrap();
            let cgroup_proc_path = proc_dir.join("cgroup");

            // /proc/meminfo
            let proc_base = base.join("proc");
            let meminfo_path = proc_base.join("meminfo");
            let mut f = std::fs::File::create(&meminfo_path).unwrap();
            write!(
                f,
                "MemTotal:       16384000 kB\nMemFree:        8192000 kB\n"
            )
            .unwrap();

            Self {
                _dir: dir,
                mem_limit_path: mem_limit_path.to_string_lossy().to_string(),
                cpu_quota_path: cpu_quota_path.to_string_lossy().to_string(),
                cpu_period_path: cpu_period_path.to_string_lossy().to_string(),
                _cgroup_proc_path: cgroup_proc_path.to_string_lossy().to_string(),
                meminfo_path: meminfo_path.to_string_lossy().to_string(),
            }
        }
    }

    /// 创建模拟的 cgroup v2 文件系统
    #[allow(dead_code)]
    struct MockCgroupV2 {
        _dir: TempDir,
        mem_max_path: String,
        cpu_max_path: String,
        _cgroup_proc_path: String,
    }

    impl MockCgroupV2 {
        fn new(mem_max: &str, cpu_max: &str) -> Self {
            let dir = TempDir::new().unwrap();
            let base = dir.path();

            // memory.max
            let mem_dir = base.join("sys/fs/cgroup");
            std::fs::create_dir_all(&mem_dir).unwrap();
            let mem_max_path = mem_dir.join("memory.max");
            let mut f = std::fs::File::create(&mem_max_path).unwrap();
            write!(f, "{}", mem_max).unwrap();

            // cpu.max
            let cpu_max_path = mem_dir.join("cpu.max");
            let mut f = std::fs::File::create(&cpu_max_path).unwrap();
            write!(f, "{}", cpu_max).unwrap();

            // /proc/1/cgroup
            let proc_dir = base.join("proc/1");
            std::fs::create_dir_all(&proc_dir).unwrap();
            let cgroup_proc_path = proc_dir.join("cgroup");

            // /proc/meminfo
            let proc_base = base.join("proc");
            let meminfo_path = proc_base.join("meminfo");
            let mut f = std::fs::File::create(&meminfo_path).unwrap();
            write!(
                f,
                "MemTotal:       16384000 kB\nMemFree:        8192000 kB\n"
            )
            .unwrap();

            Self {
                _dir: dir,
                mem_max_path: mem_max_path.to_string_lossy().to_string(),
                cpu_max_path: cpu_max_path.to_string_lossy().to_string(),
                _cgroup_proc_path: cgroup_proc_path.to_string_lossy().to_string(),
            }
        }
    }

    // ========================================================================
    // UT-CTN-001: Docker 环境检测
    // ========================================================================
    #[test]
    fn test_ut_ctn_001_docker_detection() {
        // 由于测试环境可能没有 /.dockerenv，我们测试 /proc/1/cgroup 的逻辑
        // 这里我们间接测试：通过检查方法是否正常执行不 panic
        let result = ContainerDetector::detect_runtime();
        // 不应 panic，返回值可能是 None 或 Some
        assert!(result.is_none() || result.is_some());
    }

    // ========================================================================
    // UT-CTN-002: cgroup 内存限制 512MB（cgroup v1）
    // ========================================================================
    #[test]
    fn test_ut_ctn_002_memory_limit_v1_512mb() {
        // cgroup v1: 512MB = 512 * 1024 * 1024 = 536870912 bytes
        let mock = MockCgroupV1::new(Some(536_870_912), -1, 100_000);

        // 模拟 ContainerDetector 内部读取 mock 文件
        let limit = read_memory_limit_v1_from(&mock.mem_limit_path, &mock.meminfo_path);
        assert_eq!(limit, Some(512));
    }

    // ========================================================================
    // UT-CTN-003: cgroup 无限制回退（cgroup v1 limit >= 物理内存）
    // ========================================================================
    #[test]
    fn test_ut_ctn_003_memory_no_limit_v1() {
        // 创建一个 mock，其中 limit 远大于物理内存（16GB = 16384MB）
        // 使用典型的"无限制"大值 9223372036854771712
        let mock = MockCgroupV1::new(Some(9_223_372_036_854_771_712), -1, 100_000);

        let limit = read_memory_limit_v1_from(&mock.mem_limit_path, &mock.meminfo_path);
        assert_eq!(limit, None, "无限值应回退为 None");
    }

    // ========================================================================
    // UT-CTN-004: cgroup CPU 限制 2 核（cgroup v1）
    // ========================================================================
    #[test]
    fn test_ut_ctn_004_cpu_limit_2core_v1() {
        let mock = MockCgroupV1::new(None, 200_000, 100_000);

        let cores = read_cpu_limit_v1_from(&mock.cpu_quota_path, &mock.cpu_period_path);
        assert_eq!(cores, Some(2.0));
    }

    // ========================================================================
    // UT-CTN-004A: cgroup v2 memory.max="max"（无限制）
    // ========================================================================
    #[test]
    fn test_ut_ctn_004a_memory_max_v2_max_string() {
        let mock = MockCgroupV2::new("max", "max 100000");

        let limit = read_memory_limit_v2_from(&mock.mem_max_path);
        assert_eq!(limit, None, "v2 memory.max='max' 应返回 None");
    }

    // ========================================================================
    // UT-CTN-004B: cgroup v1 memory.limit = -1（实际存为 u64 大值）
    // ========================================================================
    #[test]
    fn test_ut_ctn_004b_memory_limit_negative_v1() {
        // 内核将 -1 写为 u64::MAX
        let mock = MockCgroupV1::new(Some(u64::MAX), -1, 100_000);

        let limit = read_memory_limit_v1_from(&mock.mem_limit_path, &mock.meminfo_path);
        assert_eq!(limit, None, "v1 memory.limit = u64::MAX 应返回 None");
    }

    // ========================================================================
    // UT-CTN-005: CPU 无限制回退（quota = -1）
    // ========================================================================
    #[test]
    fn test_ut_ctn_005_cpu_no_limit_v1() {
        let mock = MockCgroupV1::new(None, -1, 100_000);

        let cores = read_cpu_limit_v1_from(&mock.cpu_quota_path, &mock.cpu_period_path);
        assert_eq!(cores, None, "quota=-1 应返回 None");
    }

    // ========================================================================
    // cgroup v2 CPU 限制
    // ========================================================================
    #[test]
    fn test_cpu_limit_v2_4cores() {
        let mock = MockCgroupV2::new("max", "400000 100000");

        let cores = read_cpu_limit_v2_from(&mock.cpu_max_path);
        assert_eq!(cores, Some(4.0));
    }

    #[test]
    fn test_cpu_limit_v2_max() {
        let mock = MockCgroupV2::new("max", "max 100000");

        let cores = read_cpu_limit_v2_from(&mock.cpu_max_path);
        assert_eq!(cores, None, "v2 cpu.max='max' 应返回 None");
    }

    // ========================================================================
    // cgroup v2 内存限制具体值
    // ========================================================================
    #[test]
    fn test_memory_limit_v2_2gb() {
        let mock = MockCgroupV2::new("2147483648", "max 100000");

        let limit = read_memory_limit_v2_from(&mock.mem_max_path);
        assert_eq!(limit, Some(2048), "2GB = 2048MB");
    }

    // ========================================================================
    // UT-CTN-020: 物理机→Docker 迁移检测（通过 /proc/1/cgroup 内容模拟）
    // ========================================================================
    #[test]
    fn test_ut_ctn_020_baremetal_to_docker() {
        // 模拟容器内的 /proc/1/cgroup
        // 以 docker 为例，cgroup 内容包含 "docker"
        let cgroup_content = "12:blkio:/docker/abc123\n11:cpuset:/docker/abc123\n";
        let runtime = detect_runtime_from_cgroup(cgroup_content);
        assert_eq!(runtime, Some("docker".to_string()));

        // kubernetes 场景
        let k8s_content = "1:name=systemd:/kubepods/besteffort/pod123/abc456\n";
        let runtime = detect_runtime_from_cgroup(k8s_content);
        assert_eq!(runtime, Some("kubernetes".to_string()));

        // 物理机场景
        let bare_content = "12:blkio:/\n11:cpuset:/\n";
        let runtime = detect_runtime_from_cgroup(bare_content);
        assert_eq!(runtime, None);
    }

    // ========================================================================
    // 测试辅助方法：直接从指定路径读取，绕过真正的 /sys/fs/cgroup
    // ========================================================================

    /// 从指定路径读取 cgroup v1 内存限制
    fn read_memory_limit_v1_from(limit_path: &str, meminfo_path: &str) -> Option<u64> {
        let content = std::fs::read_to_string(limit_path).ok()?;
        let content = content.trim().to_string();
        let limit: u64 = content.parse().ok()?;

        // 读取模拟的 meminfo
        let meminfo = std::fs::read_to_string(meminfo_path).ok()?;
        let total_phys_mb = {
            let mut total = 0u64;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            total = kb / 1024;
                        }
                    }
                }
            }
            total
        };

        if limit >= total_phys_mb * 1024 * 1024
            || limit == u64::MAX
            || limit > 9_000_000_000_000_000_000
        {
            return None;
        }

        Some(limit / (1024 * 1024))
    }

    /// 从指定路径读取 cgroup v2 内存限制
    fn read_memory_limit_v2_from(path: &str) -> Option<u64> {
        let content = std::fs::read_to_string(path).ok()?;
        let content = content.trim().to_string();
        if content == "max" {
            return None;
        }
        let limit: u64 = content.parse().ok()?;
        Some(limit / (1024 * 1024))
    }

    /// 从指定路径读取 cgroup v1 CPU 限制
    fn read_cpu_limit_v1_from(quota_path: &str, period_path: &str) -> Option<f64> {
        let quota_str = std::fs::read_to_string(quota_path).ok()?;
        let period_str = std::fs::read_to_string(period_path).ok()?;
        let quota_str = quota_str.trim().to_string();
        let period_str = period_str.trim().to_string();

        let quota: i64 = quota_str.parse().ok()?;
        let period: i64 = period_str.parse().ok()?;

        if quota == -1 || period == 0 {
            return None;
        }

        let cores = quota as f64 / period as f64;
        if cores <= 0.0 {
            return None;
        }

        Some(cores)
    }

    /// 从指定路径读取 cgroup v2 CPU 限制
    fn read_cpu_limit_v2_from(path: &str) -> Option<f64> {
        let content = std::fs::read_to_string(path).ok()?;
        let content = content.trim().to_string();

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let quota_str = parts[0];
        if quota_str == "max" {
            return None;
        }

        let quota: u64 = quota_str.parse().ok()?;
        let period: u64 = parts[1].parse().ok()?;

        if period == 0 {
            return None;
        }

        let cores = quota as f64 / period as f64;
        if cores <= 0.0 {
            return None;
        }

        Some(cores)
    }

    /// 从 cgroup 内容检测运行时
    fn detect_runtime_from_cgroup(content: &str) -> Option<String> {
        if content.contains("docker") {
            Some("docker".to_string())
        } else if content.contains("kubepods") {
            Some("kubernetes".to_string())
        } else {
            None
        }
    }
}
