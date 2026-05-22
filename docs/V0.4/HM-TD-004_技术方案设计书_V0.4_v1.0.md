# 秋毫mem V0.4 · 技术方案设计书

**文档编号**：HM-TD-004
**版本**：V1.0
**日期**：2026-05-21
**编写人**：技术负责人

## 一、架构变更概述

V0.3 的核心架构：CLI 入口 → 部署评估/实时监控 → 动态校准 + 状态机 + ResourceCollector。

V0.4 新增四个核心模块和一个重大增强：

```
┌──────────────────────────────────────────────────────────────┐
│                        CLI 入口 (main.rs)                     │
│  + --serve / --remote / --trend / --alert / --clear-history  │
│  + --env-fingerprint / --reset-environment                   │
└──────┬────────┬──────────┬──────────┬──────────┬─────────────┘
       │        │          │          │          │
┌──────▼──┐ ┌──▼───┐ ┌───▼───┐ ┌───▼────┐ ┌───▼──────┐
│ 环境指纹 │ │ 远程 │ │ 趋势  │ │ 容器   │ │ 多Agent  │
│ 引擎    │ │ 采集 │ │ 分析  │ │ 适配层 │ │ 全局视图 │
└────┬────┘ └──┬───┘ └───┬───┘ └───┬────┘ └────┬─────┘
     │         │         │         │            │
     └─────────┼─────────┼─────────┼────────────┘
               │         │         │
          ┌────▼────┐ ┌─▼──────┐ ┌▼──────────┐
          │ 本地存储 │ │ HTTP   │ │ Resource  │
          │ (JSON/  │ │ Server │ │ Collector │
          │  JSONL) │ │        │ │ (已有)    │
          └─────────┘ └────────┘ └───────────┘
```

**核心变更**：
1. **环境指纹引擎**：首次运行生成指纹，后续检测环境变化，自动触发迁移建议。
2. **远程采集**：轻量 HTTP 服务端 + 客户端聚合，支持多机资源视图。
3. **趋势分析引擎**：本地时序数据存储 + 趋势方向分析。
4. **容器适配层**：cgroup 感知，Docker/K8s 环境检测与资源限制尊重。
5. **多 Agent 全局视图增强**：从进程检测升级为每个 Agent 资源占用详情列表。

## 二、环境指纹引擎

### 2.1 指纹生成

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFingerprint {
    pub id: String,  // SHA256(hostname + machine_id + timestamp)
    pub created_at: String,
    pub hostname: String,  // 脱敏：仅保留前 8 位哈希
    pub platform: String,
    pub cpu_cores: u32,
    pub total_memory_mb: u64,
    pub gpu_names: Vec<String>,
    pub disk_total_mb: u64,
    pub container_runtime: Option<String>,  // "docker" / "kubernetes" / null
}

impl EnvironmentFingerprint {
    pub fn generate(snapshot: &ResourceSnapshot) -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_default();
        let hostname_hash = sha256_hex(&hostname, 16);  // CR-06：主机名脱敏
        
        Self {
            id: generate_fingerprint_id(&hostname),
            created_at: Utc::now().to_rfc3339(),
            hostname: hostname_hash,
            platform: std::env::consts::OS.to_string(),
            cpu_cores: snapshot.cpu.as_ref().map(|c| c.cores).unwrap_or(0),
            total_memory_mb: snapshot.memory.as_ref().map(|m| m.total_mb).unwrap_or(0),
            gpu_names: snapshot.gpu.as_ref()
                .map(|g| g.iter().map(|g| g.name.clone()).collect())
                .unwrap_or_default(),
            disk_total_mb: snapshot.disk.as_ref().map(|d| d.total_mb).unwrap_or(0),
            container_runtime: detect_container_runtime(),
        }
    }
}
```

**存储位置**：`~/.config/hawk-eye-mem/environment.json`

**历史指纹**：保留最近 3 次，文件名为 `environment.json`、`environment.1.json`、`environment.2.json`。新指纹写入前将旧文件轮转。

### 2.2 变更检测

```rust
impl EnvironmentFingerprint {
    pub fn detect_changes(&self, previous: &Self) -> Vec<EnvironmentChange> {
        let mut changes = Vec::new();
        
        // 内存变化超过模型所需的 50%（CR-02）
        // 模型所需默认按 8GB 估算 → 50% = 4GB
        let mem_diff = (self.total_memory_mb as i64 - previous.total_memory_mb as i64).abs();
        if mem_diff > 4096 {  // 4GB 阈值
            changes.push(EnvironmentChange {
                resource: "memory".to_string(),
                previous_mb: previous.total_memory_mb,
                current_mb: self.total_memory_mb,
                direction: if self.total_memory_mb > previous.total_memory_mb 
                    { "upgrade" } else { "degrade" },
            });
        }
        
        // CPU 核心数变化超过 2 核
        let cpu_diff = (self.cpu_cores as i32 - previous.cpu_cores as i32).abs();
        if cpu_diff >= 2 {
            changes.push(EnvironmentChange { /* ... */ });
        }
        
        // GPU 增减
        if self.gpu_names != previous.gpu_names {
            changes.push(EnvironmentChange { /* ... */ });
        }
        
        changes
    }
}
```

**迁移建议**：当检测到 `upgrade` 或 `degrade` 时，自动运行部署评估引擎，基于新环境重新计算 `deployment_assessment`，并输出 `new_recommendation`。

### 2.3 JSON 输出扩展

```json
{
  "environment_change": {
    "detected": true,
    "previous_fingerprint_id": "a1b2c3d4",
    "changes": [
      {
        "resource": "memory",
        "previous_mb": 16384,
        "current_mb": 65536,
        "direction": "upgrade"
      }
    ],
    "new_recommendation": "Memory has increased to 64GB. You can now safely run larger models."
  }
}
```

## 三、远程采集

### 3.1 HTTP 服务端

```rust
pub struct RemoteServer {
    port: u16,
    api_key: Option<String>,
}

impl RemoteServer {
    pub fn start(&self) -> Result<()> {
        // CR-05：绑定地址检查
        let bind_addr = format!("127.0.0.1:{}", self.port);  // 硬编码 localhost
        // 如果用户传了 --bind 0.0.0.0，强制退出
        if bind_addr.starts_with("0.0.0.0") {
            eprintln!("ERROR: Binding to 0.0.0.0 is forbidden for security reasons.");
            std::process::exit(1);
        }
        
        let server = tiny_http::Server::http(&bind_addr)?;
        
        for request in server.incoming_requests() {
            let url = request.url();
            
            match url {
                "/metrics" => {
                    // CR-01：只返回 system 快照
                    // CR-07：最小权限，仅返回聚合所需指标
                    let snapshot = CollectorRegistry::new().collect_all()?;
                    let metrics = MetricsResponse {
                        memory_available_mb: snapshot.memory.as_ref().map(|m| m.available_mb),
                        memory_pressure: snapshot.memory.as_ref().map(|m| m.pressure.clone()),
                        cpu_load_1m: snapshot.cpu.as_ref().map(|c| c.load_avg_1m),
                        disk_available_mb: snapshot.disk.as_ref().map(|d| d.available_mb),
                        gpu_vram_available_mb: snapshot.gpu.as_ref()
                            .and_then(|g| g.first())
                            .map(|g| g.vram_total_mb - g.vram_used_mb),
                    };
                    respond_json(&request, &metrics)?;
                }
                "/full" => {
                    // 可选：返回完整快照（含 agent_guidance）
                    let output = collect_full_output()?;
                    respond_json(&request, &output)?;
                }
                _ => {
                    request.respond(Response::from_string("404")
                        .with_status_code(404))?;
                }
            }
        }
    }
}
```

**认证中间件**：
```rust
fn check_auth(request: &Request, api_key: &Option<String>) -> bool {
    match api_key {
        None => true,
        Some(key) => {
            request.headers().iter()
                .find(|h| h.field.equiv("Authorization"))
                .map(|h| h.value.as_str() == format!("Bearer {}", key))
                .unwrap_or(false)
        }
    }
}
```

### 3.2 远程客户端

```rust
pub struct RemoteClient {
    urls: Vec<String>,
    api_key: Option<String>,
    timeout: Duration,
}

impl RemoteClient {
    pub fn fetch_all(&self) -> Result<Vec<RemoteNode>> {
        let mut nodes = Vec::new();
        
        for url in &self.urls {
            // CR-06：传输安全提示
            if url.starts_with("http://") {
                eprintln!("Warning: Using HTTP for remote metrics. Consider HTTPS for sensitive environments.");
            }
            
            let response = ureq::get(url)
                .set("Authorization", &format!("Bearer {}", self.api_key.as_deref().unwrap_or("")))
                .timeout(self.timeout)
                .call()?;
            
            let metrics: MetricsResponse = serde_json::from_reader(response.into_reader())?;
            nodes.push(RemoteNode {
                url: url.clone(),
                metrics,
                reachable: true,
            });
        }
        
        Ok(nodes)
    }
}
```

**聚合输出**：
```json
{
  "remote_nodes": [
    {
      "url": "http://192.168.1.10:9240",
      "reachable": true,
      "memory_available_mb": 5800,
      "memory_pressure": "high"
    }
  ],
  "global_summary": {
    "total_nodes": 3,
    "nodes_critical": 1,
    "global_action": "reduce_context"
  }
}
```

## 四、趋势分析引擎

### 4.1 数据存储

```rust
// 复用 CalibrationStore trait（CR-04）
pub struct HistoryStore {
    path: PathBuf,
}

impl CalibrationStore for HistoryStore {
    // 历史数据点
    fn append(&self, point: CalibrationPoint, _model_hash: &str) -> Result<()> {
        let file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        flock_write(&file, || {
            writeln!(file, "{}", serde_json::to_string(&HistoryPoint {
                timestamp: point.timestamp,
                memory_available_mb: point.bytes_per_token,  // 复用字段
                memory_pressure: point.tokens_processed as u32,  // 复用
                cpu_load: 0.0,  // HistoryPoint 扩展字段
                disk_available_mb: 0,
            })?)?;
            Ok(())
        })?;
        Ok(())
    }
    
    fn read_by_model(&self, _model_hash: &str) -> Result<Vec<CalibrationPoint>> {
        // 按时间范围读取
    }
    
    fn clear_model(&self, _model_hash: &str) -> Result<()> {
        std::fs::remove_file(&self.path)?;
        Ok(())
    }
}
```

**存储文件**：`~/.config/hawk-eye-mem/history.jsonl`

**数据保留**：
```rust
impl HistoryStore {
    pub fn cleanup(&self, retention_days: u64) -> Result<()> {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);
        let lines = std::fs::read_to_string(&self.path)?;
        let filtered: Vec<String> = lines.lines()
            .filter(|line| {
                let point: HistoryPoint = serde_json::from_str(line).unwrap_or_default();
                point.timestamp > cutoff
            })
            .map(|s| s.to_string())
            .collect();
        std::fs::write(&self.path, filtered.join("\n"))?;
        Ok(())
    }
}
```

### 4.2 趋势计算

```rust
impl TrendAnalyzer {
    pub fn analyze(&self, store: &HistoryStore) -> Result<TrendReport> {
        let points = store.read_recent(100)?;  // 最近 100 个采样点
        
        if points.len() < 10 {
            return Ok(TrendReport {
                direction: "insufficient_data",
                confidence: "low",
                ..Default::default()
            });
        }
        
        // 简单线性回归
        let (slope, r_squared) = linear_regression(&points);
        
        let direction = if slope.abs() < 0.01 { "stable" }
                        else if slope > 0.0 { "increasing" }
                        else { "decreasing" };
        
        // 预计到达临界时间
        let days_until_critical = if slope < 0.0 {
            let latest_avail = points.last().unwrap().memory_available_mb;
            let critical_threshold = 2048;  // 2GB
            if latest_avail > critical_threshold as f64 {
                Some(((latest_avail - critical_threshold as f64) / slope.abs()) / 1440.0)
            } else {
                Some(0.0)
            }
        } else {
            None
        };
        
        Ok(TrendReport {
            direction: direction.to_string(),
            slope_mb_per_minute: slope,
            r_squared,
            days_until_critical,
            confidence: if points.len() > 50 { "high" } else { "medium" }.to_string(),
        })
    }
}
```

## 五、容器与云环境适配

```rust
pub struct ContainerDetector;

impl ContainerDetector {
    /// 检测容器运行时
    pub fn detect_runtime() -> Option<String> {
        // Docker
        if Path::new("/.dockerenv").exists() {
            return Some("docker".to_string());
        }
        
        // cgroup 中包含 docker/kubepods 关键字
        if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
            if cgroup.contains("docker") { return Some("docker".to_string()); }
            if cgroup.contains("kubepods") { return Some("kubernetes".to_string()); }
        }
        
        None
    }
    
    /// 读取 cgroup 内存限制
    pub fn get_memory_limit() -> Option<u64> {
        let limit_str = std::fs::read_to_string(
            "/sys/fs/cgroup/memory/memory.limit_in_bytes"
        ).ok()?;
        
        let limit: u64 = limit_str.trim().parse().ok()?;
        
        // CR-03：无限值处理
        let physical_mem = get_physical_memory();
        if limit >= physical_mem {
            None  // 无实际限制，使用物理内存
        } else {
            Some(limit / (1024 * 1024))  // 转为 MB
        }
    }
    
    /// 读取 CPU 限制
    pub fn get_cpu_limit() -> Option<f64> {
        let quota = std::fs::read_to_string(
            "/sys/fs/cgroup/cpu/cpu.cfs_quota_us"
        ).ok()?.trim().parse::<i64>().ok()?;
        
        let period = std::fs::read_to_string(
            "/sys/fs/cgroup/cpu/cpu.cfs_period_us"
        ).ok()?.trim().parse::<i64>().ok()?;
        
        // -1 表示无限制
        if quota == -1 || period == 0 {
            return None;
        }
        
        Some(quota as f64 / period as f64)
    }
}
```

**在 ResourceSnapshot 中集成**：
```rust
pub fn collect_all(&self) -> Result<ResourceSnapshot> {
    let mut snapshot = ResourceSnapshot::default();
    
    // 检测容器环境
    let container_runtime = ContainerDetector::detect_runtime();
    
    // 内存：优先使用 cgroup 限制
    if let Some(limit_mb) = ContainerDetector::get_memory_limit() {
        let mut mem = MemoryCollector::collect()?;
        mem.total_mb = limit_mb;  // 用 cgroup 限制替代物理内存
        snapshot.memory = Some(mem);
    }
    
    // CPU：优先使用 cgroup 限制
    if let Some(cpu_limit) = ContainerDetector::get_cpu_limit() {
        let mut cpu = CpuCollector::collect()?;
        cpu.cores = cpu_limit.ceil() as u32;
        snapshot.cpu = Some(cpu);
    }
    
    snapshot.container_runtime = container_runtime;
    Ok(snapshot)
}
```

## 六、多 Agent 全局视图增强

```rust
#[derive(Debug, Clone, Serialize)]
pub struct MultiAgentView {
    pub agents: Vec<AgentResourceUsage>,
    pub total_agent_memory_mb: u64,
    pub total_agent_cpu_percent: f64,
    // V0.4 不包含 global_strategy 和 resource_allocation（CR-08）
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentResourceUsage {
    pub name: String,
    pub pid: u32,
    pub memory_rss_mb: u64,
    pub cpu_percent: f64,
    pub runtime: Option<String>,  // "hermes" / "claude-code" / null
}
```

**约束**：V0.4 只输出全局视图，不输出协调策略。`global_pressure` 继续延后（CR-08：多 Agent 对外只说"全局视图"）。

## 七、CLI 与配置文件扩展

### 7.1 新增 CLI 参数

| 参数 | 功能 |
|------|------|
| `--env-fingerprint` | 输出当前环境指纹 JSON |
| `--reset-environment` | 重置环境指纹（需 `--force`） |
| `--serve` | 启动 HTTP 服务（`--port` 指定端口，默认 9240） |
| `--remote <url>` | 从远程秋毫mem 实例拉取指标（可多次指定） |
| `--remote-key <key>` | 远程认证密钥 |
| `--trend` | 输出历史趋势分析报告 |
| `--clear-history` | 清空历史数据 |
| `--alert` | 告警模式（仅 critical 时输出一行，配合管道） |

### 7.2 配置文件扩展

```toml
[remote]
api_key = "your-secret-key"
nodes = ["http://192.168.1.10:9240", "http://192.168.1.11:9240"]

[history]
retention_days = 7
auto_record = true  # --interval 模式下自动记录

[agents]
# 注册本机运行的 Agent 名称，用于多 Agent 检测
names = ["hermes-main", "claude-code"]
```

## 八、关键风险与应对

| 风险 | 等级 | 应对 |
|------|------|------|
| `--serve` 公网暴露 | 高 | CR-05：绑定 0.0.0.0 时强制退出；文档醒目警告 |
| 远程采集明文传输 | 中 | CR-06：HTTP 时输出警告；后续版本优先 HTTPS |
| cgroup 路径因发行版/内核版本不同 | 中 | 优雅回退：cgroup 不可用或格式不识别时使用物理内存 |
| 历史数据与校准数据存储逻辑不一致 | 低 | CR-04：复用 CalibrationStore trait，统一接口 |
| 多 Agent 协调建议过早暴露 | 低 | CR-08：V0.4 只输出全局视图，不提协调 |

## 九、工期预估（10 周）

| 周次 | 任务 | 里程碑 |
|------|------|--------|
| W1-W2 | 环境指纹引擎：生成+检测+存储+JSON输出 | 环境指纹可用 |
| W2-W3 | 远程采集 HTTP 服务端 + 客户端 | 远程采集可用 |
| W3-W4 | 容器适配层：cgroup 感知 + Docker/K8s | 容器环境正确识别 |
| W4-W5 | 趋势分析引擎：HistoryStore + 趋势计算 | 趋势分析可用 |
| W5-W6 | 多 Agent 全局视图增强 | 多 Agent 视图可用 |
| W6-W7 | CLI 新增参数 + 配置文件扩展 | 所有 CLI 参数可用 |
| W7-W8 | 集成测试 + 跨平台测试（含多机） | 全功能集成通过 |
| W8-W9 | 文档 + DISCLAIMER 更新 + 数据说明文档 | 文档齐全 |
| W9-W10 | Buffer：风险应对 + 性能调优 + 发布准备 | v0.4.0 发布 |

## 十、存储文件总览

| 文件 | 格式 | 内容 | V0.4 新增 |
|------|------|------|----------|
| `calibration.csv` | CSV | 校准数据（已哈希） | V0.3 |
| `environment.json` | JSON | 当前环境指纹 | ✅ |
| `environment.1.json` | JSON | 历史指纹 1 | ✅ |
| `environment.2.json` | JSON | 历史指纹 2 | ✅ |
| `history.jsonl` | JSONL | 时序历史快照 | ✅ |
| `config.toml` | TOML | 用户配置 | V0.1 |

---

**技术负责人总结**：V0.4 的技术核心是将秋毫mem 从单机工具升级为集群感知平台。环境指纹让 Agent 搬家后自动适应，远程采集让运维人员在一台机器上看全局，容器适配覆盖最常见的部署场景。存储层复用 V0.3 的 CalibrationStore trait 统一逻辑。远程采集的 HTTP 服务默认绑定 localhost、硬拦截公网 IP，安全基线已建立。工期 10 周，P0 必达，P1 最简，P2 延后。
