# 秋毫mem V0.4 · 测试用例文档

**文档编号**：HM-TC-004
**版本**：V1.0
**对应版本**：秋毫mem V0.4.0
**编写日期**：2026-05-21
**编写人**：测试经理
**测试范围**：环境指纹引擎、远程采集、历史趋势、容器适配、多Agent全局视图、V0.3回归
**测试环境**：
- 标准环境：Linux (Ubuntu 22.04 x86_64, 16GB RAM)、macOS (Apple Silicon 14.x, 16GB RAM)
- 容器环境：Docker (cgroup v2)、K8s (minikube)
- 多机环境：至少3台机器（可Docker模拟）
- 低配环境：2GB VM、5GB MacBook Air
- GPU环境：NVIDIA RTX 3060、Apple M1


## 一、测试策略

| 层级 | 自动化 | 覆盖目标 | 通过标准 |
|------|--------|----------|----------|
| 单元测试 | ✅ | 环境指纹、远程采集、趋势分析、容器适配、多Agent视图 | 100%通过 |
| 集成测试 | ✅ | CLI参数、多机协同、配置文件 | 100%通过 |
| 回归测试 | ✅ | V0.3全部功能 | 100%通过 |
| 安全测试 | ⚠️ 部分自动化 | API认证、速率限制、公网拦截、数据脱敏 | 全部通过 |
| 手动验收 | ❌ | 真实多机、K8s集群、环境迁移场景 | 全部通过 |


## 二、V0.3 回归测试

确保新模块不影响已有功能。

| 用例ID | 测试目标 | 预期结果 |
|--------|----------|----------|
| REG-020 | `--can-run` 仍可用 | 输出部署评估 |
| REG-021 | `--json` 基础结构不变 | 含system.memory、agent_guidance |
| REG-022 | `--calibration-stats` 仍可用 | 输出校准统计 |
| REG-023 | `--reset-calibration` 仍可用 | 清空校准数据 |
| REG-024 | `--interval --count` 仍可用 | JSON Lines输出 |
| REG-025 | `--gpu-list` 仍可用 | 列出GPU |
| REG-026 | 动态校准引擎仍可用 | confidence从conservative→calibrated |
| REG-027 | 连续监控状态机仍可用 | 状态正常转换 |
| REG-028 | V0.3配置文件兼容 | 正常加载，新段使用默认值 |


## 三、环境指纹引擎测试

### 3.1 指纹生成

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-ENV-001 | 首次运行生成指纹 | 无environment.json | 生成文件，包含id/platform/hostname_hash/cores/memory/gpu/disk |
| UT-ENV-002 | 主机名脱敏 | — | hostname字段为16位哈希，非明文 |
| UT-ENV-003 | 指纹ID稳定 | 同一机器多次生成 | id不变（基于machine-id） |
| UT-ENV-004 | 容器运行时检测 | Docker内运行 | container_runtime="docker" |
| UT-ENV-005 | 非容器环境 | 物理机/VM | container_runtime=null |
| UT-ENV-006 | GPU信息采集 | 有GPU | gpu_names数组包含GPU名称 |
| UT-ENV-007 | 无GPU环境 | 无GPU | gpu_names为空数组 |

### 3.2 变更检测

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-ENV-010 | 内存大幅升级触发 | 旧16GB→新64GB（差48GB>4GB） | 触发变更，direction=upgrade |
| UT-ENV-011 | 内存小幅变化不触发 | 旧16GB→新18GB（差2GB<4GB且<20%） | 不触发变更 |
| UT-ENV-012 | 内存变化超过20%触发 | 旧16GB→新20GB（差4GB=25%>20%） | 触发变更 |
| UT-ENV-013 | CPU核心数变化≥2触发 | 旧4核→新8核 | 触发变更 |
| UT-ENV-014 | CPU核心数变化<2不触发 | 旧4核→新5核 | 不触发变更 |
| UT-ENV-015 | GPU增加触发 | 旧无GPU→新有GPU | 触发变更 |
| UT-ENV-016 | GPU减少触发 | 旧有GPU→新无GPU | 触发变更 |
| UT-ENV-017 | 磁盘大幅变化触发 | 旧100GB→新500GB | 触发变更 |
| UT-ENV-018 | 容器环境切换 | 物理机→Docker | 触发变更 |
| UT-ENV-019A | 多维度同时变更（内存+GPU+CPU） | 旧4核16GB无GPU→新8核32GB有GPU | 3条change记录，逐维度 |

### 3.3 迁移建议

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-ENV-020 | 升级后自动重新评估 | 内存16GB→64GB | 输出new_recommendation，含"70B"或"larger models" |
| UT-ENV-021 | 降级后警告 | 内存64GB→8GB | 输出new_recommendation，含"reduce context to" |
| UT-ENV-022 | GPU新增建议 | GPU从无到有 | 文案含"switch to GPU inference" |
| UT-ENV-023 | GPU移除建议 | GPU从有到无 | 文案含"fall back to CPU" |
| UT-ENV-024 | 多维度变化 | 内存↑+GPU↑ | 逐项列出每个变化和建议 |

### 3.4 指纹存储与CLI

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-ENV-030 | 指纹文件轮转 | 已有3个指纹文件 | environment.json→environment.1.json→environment.2.json |
| UT-ENV-031 | HMAC签名验证 | 手动篡改指纹文件 | 读取时验签失败，提示"环境指纹可能被篡改" |
| UT-ENV-032 | 签名密钥基于machine-id | 复制指纹文件到另一台机器 | 验签失败 |
| UT-ENV-033 | `--env-fingerprint` 输出 | CLI运行 | JSON格式，与存储文件一致 |
| UT-ENV-034 | `--reset-environment` | CLI运行+确认 | 清空所有指纹文件 |
| UT-ENV-035 | `--reset-environment --force` | CLI运行 | 无需确认，直接清空 |


## 四、远程采集测试

### 4.1 HTTP 服务端

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-REM-001 | `--serve` 启动 | 端口9240可用 | HTTP服务监听127.0.0.1:9240 |
| UT-REM-002 | `--serve --port 9999` | 端口9999可用 | 监听127.0.0.1:9999 |
| UT-REM-003 | 绑定0.0.0.0强制退出 | 尝试绑定0.0.0.0 | 输出ERROR，退出码1 |
| UT-REM-004 | `/metrics` 端点返回 | 服务运行 | 返回JSON，仅含system层指标 |
| UT-REM-005 | `/metrics` 不含agent_guidance | 服务运行 | 返回JSON中无agent_guidance |
| UT-REM-006 | `/metrics` 不含deployment_assessment | 服务运行 | 无deployment_assessment |
| UT-REM-007 | `/metrics` 最小权限 | 服务运行 | 仅返回memory_available_mb/pressure/cpu_load/disk_available_mb/gpu_vram |
| UT-REM-008 | `/full` 端点返回完整快照 | 服务运行 | 完整ResourceSnapshot |
| UT-REM-009 | 未配置API Key时无认证 | 无api_key | `/metrics` 可直接访问 |
| UT-REM-010 | 配置API Key时拦截无认证 | 有api_key | 无Authorization头→401 |
| UT-REM-011 | 正确API Key通过认证 | 有api_key | Authorization: Bearer <key> →200 |
| UT-REM-012 | 错误API Key被拒绝 | 有api_key | 错误key→401 |
| UT-REM-013 | API Key恒定时间比较 | 模糊测试 | 正确/错误key响应时间差异<1ms |
| UT-REM-014 | 速率限制 | 同一IP 1秒内11次请求 | 第11次返回429 |
| UT-REM-015 | 速率限制恢复 | 超过限制后等待1秒 | 下一窗口正常返回 |
| UT-REM-016 | 无效路径返回404 | 服务运行 | `/invalid` →404 |
| UT-REM-032 | HTTP服务panic隔离 | 注入panic collector | 仅该请求出错，其他请求正常 |

### 4.2 远程客户端

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-REM-020 | `--remote` 单机拉取 | 远程服务运行 | 返回远程机器的system指标 |
| UT-REM-021 | `--remote` 多机聚合 | 3台远程服务运行 | remote_nodes数组3个元素 |
| UT-REM-022 | 远程超时处理 | 远程服务不可达 | 5秒超时，该节点reachable=false |
| UT-REM-023 | `--remote-key` 认证 | 远程服务有api_key | 正确key拉取成功 |
| UT-REM-024 | 远程key错误 | 远程服务有api_key | 返回认证失败 |
| UT-REM-025 | HTTP明文传输警告 | URL为http:// | stderr输出Warning，但继续执行 |
| UT-REM-026 | HTTPS传输无警告 | URL为https:// | 无警告 |
| UT-REM-027 | 聚合全局摘要 | 3台机器，1台critical | global_summary.nodes_critical=1, global_action=reduce_context |
| UT-REM-028 | 全部正常时全局action | 3台机器全部ok | global_action=ok |
| UT-REM-029 | 配置文件中nodes自动连接 | `[remote] nodes = [...]` | 自动拉取配置中所有节点 |

### 4.3 多线程HTTP

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-REM-030 | 并发请求不阻塞 | 3个客户端同时请求 | 3个请求同时处理，非串行等待 |
| UT-REM-031 | 慢客户端不拖累其他 | 1个慢客户端+2个正常 | 正常客户端立即返回 |


## 五、历史趋势测试

### 5.1 数据存储

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-TRD-001 | `--interval` 模式自动记录 | `--interval 1`运行 | history.jsonl新增记录 |
| UT-TRD-002 | 记录包含核心字段 | 自动记录 | timestamp/memory_available_mb/pressure/cpu_load/disk_available_mb |
| UT-TRD-003 | 数据保留7天 | 7天前数据+今天数据 | 7天前数据被清理 |
| UT-TRD-003A | 清理超时保护 | history.jsonl 10000行+IO慢 | 500ms超时放弃清理，不阻塞采集 |
| UT-TRD-004 | 保留天数可配置 | `history.retention_days=3` | 3天前数据被清理 |
| UT-TRD-005 | 旧数据压缩为均值 | 超过24小时数据 | 5分钟窗口聚合 |
| UT-TRD-006 | `--clear-history` | 有历史数据 | 清空history.jsonl |
| UT-TRD-007 | `--clear-history` 需确认 | 交互式 | 提示确认 |
| UT-TRD-009 | 并发写入保护 | 两个进程同时记录 | 一个成功，一个放弃不阻塞 |

### 5.2 趋势分析

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-TRD-010 | `--trend` 输出趋势报告 | 100个采样点 | direction/slope/r_squared/days_until_critical |
| UT-TRD-011 | 稳定趋势 | 内存使用平稳 | direction=stable |
| UT-TRD-012 | 下降趋势 | 可用内存逐渐减少 | direction=decreasing |
| UT-TRD-013 | 上升趋势 | 可用内存逐渐增加 | direction=increasing |
| UT-TRD-014 | 数据不足 | <10个采样点 | direction=insufficient_data |
| UT-TRD-015 | 预计到达临界(下降趋势) | 可用内存线性下降 | days_until_critical为正数 |
| UT-TRD-016 | 无临界风险 | 可用内存充足且稳定 | days_until_critical=null |
| UT-TRD-017 | urgency=low | days_until_critical>30 | urgency=low |
| UT-TRD-018 | urgency=medium | days_until_critical 7-30 | urgency=medium |
| UT-TRD-019 | urgency=high | days_until_critical<7 | urgency=high |
| UT-TRD-020 | urgency=critical | 已到达临界 | urgency=critical |
| UT-TRD-021 | 不输出原始数据点 | `--trend` | 只含聚合指标 |
| UT-TRD-021A | 近临界+稳定趋势→urgency≥medium | used=88%, direction=stable | urgency=medium或higher |
| UT-TRD-022 | `--trend --json` | JSON输出 | trend字段完整 |

### 5.3 预言机

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-TRD-025 | 预测触发预警 | 趋势显示14天后到临界 | prediction_warning含"预计14天后" |
| UT-TRD-026 | 样本少置信度低 | <50个采样点 | prediction_confidence=low |
| UT-TRD-027 | 样本多置信度高 | >100个采样点 | prediction_confidence=high |


## 六、容器适配测试

### 6.1 Docker 环境

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-CTN-001 | Docker环境检测 | Docker容器内 | container_runtime="docker" |
| UT-CTN-002 | cgroup内存限制读取 | 容器限制512MB | total_mb=512 |
| UT-CTN-003 | cgroup无限制回退 | 容器无内存限制 | total_mb=宿主机物理内存 |
| UT-CTN-004 | cgroup CPU限制读取 | 容器限制2核 | cpu.cores=2 |
| UT-CTN-004A | cgroup v2 memory.max="max" | memory.max="max" | 回退物理内存 |
| UT-CTN-004B | cgroup memory.limit=-1 | limit_in_bytes=-1 | 回退物理内存 |
| UT-CTN-005 | CPU无限制回退 | 容器无CPU限制 | cpu.cores=宿主机核心数 |
| UT-CTN-006 | 低内存模式自动启用 | 容器512MB | 压力判定使用百分比阈值 |
| UT-CTN-007 | 512MB下正常运行 | 512MB容器 | 不OOM，采集正常 |
| UT-CTN-008 | 容器内`--json`完整输出 | Docker容器 | 所有字段存在 |
| UT-CTN-009 | 容器内`--can-run` | 512MB容器 | 评估基于512MB限额 |

### 6.2 K8s 环境

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-CTN-010 | K8s环境检测 | K8s Pod内 | container_runtime="kubernetes" |
| UT-CTN-011 | Pod资源限制 | resources.limits.memory=1Gi | total_mb=1024 |
| UT-CTN-012 | Pod元数据 | downward API或环境变量 | pod_name/namespace可选 |
| UT-CTN-013 | K8s无限制 | 无resources.limits | 回退物理内存 |

### 6.3 容器迁移

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-CTN-020 | 物理机→Docker迁移检测 | 旧指纹物理机→新环境Docker | 触发环境变更 |
| UT-CTN-021 | Docker→Docker升级 | 旧512MB→新1GB | 触发内存升级 |


## 七、多Agent全局视图测试

| 用例ID | 测试目标 | 前置条件 | 预期结果 |
|--------|----------|----------|----------|
| UT-MAV-001 | 检测已知Agent进程 | hermes运行中 | agents数组含hermes |
| UT-MAV-002 | 检测多个Agent | hermes+claude-code | agents数组2个元素 |
| UT-MAV-003 | 每个Agent含资源详情 | hermes运行中 | name/pid/memory_rss_mb/cpu_percent |
| UT-MAV-004 | 输出总Agent内存 | 2个Agent | total_agent_memory_mb为两者之和 |
| UT-MAV-007 | 自定义Agent名称 | 配置[agents] names | 检测到自定义名称 |
| UT-MAV-008 | 无Agent运行 | — | agents数组为空 |
| UT-MAV-009 | 进程已退出 | 扫描时进程刚退出 | 跳过不报错 |

**V0.4不输出**（对应CR-08）：global_strategy、resource_allocation


## 八、集成测试

| 用例ID | 测试目标 | 输入 | 预期结果 |
|--------|----------|------|----------|
| IT-CLI-040 | `--env-fingerprint` | CLI运行 | JSON输出，含fingerprint_id |
| IT-CLI-041 | `--reset-environment --force` | CLI运行 | 清空指纹文件 |
| IT-CLI-042 | `--serve --port 9999` | 后台启动 | 服务监听，可curl |
| IT-CLI-043 | `--remote http://localhost:9240` | 本地服务运行 | 拉取远程指标 |
| IT-CLI-044 | `--trend` | 有历史数据 | 趋势报告 |
| IT-CLI-045 | `--clear-history` | 有历史数据 | 清空确认 |
| IT-CLI-046 | `--alert` 正常时不输出 | pressure=low | 无输出 |
| IT-CLI-047 | `--alert` critical时输出 | pressure=critical | 最小化JSON单行：pressure/available_mb/action |
| IT-CLI-047A | `--alert` 管道消费 | `--alert \| while read line` | 每行合法JSON，下游可解析 |
| IT-CLI-048 | 新增参数`--help`可见 | `--help` | 列出所有V0.4新参数 |
| IT-CLI-049 | 配置文件新段兼容 | V0.3配置运行V0.4 | 使用默认值，不报错 |
| IT-JSON-040 | JSON含environment_change | 环境变更后 | environment_change.detected=true |
| IT-JSON-041 | JSON含remote_nodes | --remote多机 | remote_nodes数组 |
| IT-JSON-042 | JSON含trends | --trend --json | trends字段完整 |
| IT-JSON-043 | JSON含multi_agent | Agent运行 | multi_agent.agents数组 |
| IT-JSON-044 | JSON含container_runtime | Docker内 | container_runtime="docker" |


## 九、多机协同专项测试

| 用例ID | 场景 | 环境 | 步骤 | 验收标准 |
|--------|------|------|------|----------|
| MA-MULTI-001 | 3机部署远程采集 | 3个Docker容器 | 每台启动--serve，主控端--remote三个地址 | 3台指标均正确显示 |
| MA-MULTI-002 | 单机宕机影响 | 3台中1台宕机 | 停止1台服务 | 该节点reachable=false，其余正常 |
| MA-MULTI-003 | 全局告警 | 3台中1台critical | 主控端查询 | global_action=reduce_context |
| MA-MULTI-004 | 恢复检测 | 宕机节点恢复 | 重启服务 | reachable恢复为true |

## 十、环境迁移手动验收

| 用例ID | 场景 | 步骤 | 验收标准 |
|--------|------|------|----------|
| MA-MIG-001 | M1 Mac→Linux服务器 | 先在Mac跑V0.3，部署到Linux | 检测环境变化，输出迁移建议 |
| MA-MIG-002 | 物理机→Docker | 在物理机跑过，部署到容器 | 检测到容器运行时变化 |
| MA-MIG-003 | 服务器内存升级 | 16GB→64GB | 触发upgrade，建议可跑更大模型 |
| MA-MIG-004 | GPU新增 | 无GPU→加装GPU | 触发GPU新增建议 |

## 十一、网络安全专项测试

| 用例ID | 测试目标 | 步骤 | 预期结果 |
|--------|----------|------|----------|
| SEC-001 | `--serve` 公网绑定拦截 | `--serve --bind 0.0.0.0` | 强制退出，输出ERROR，退出码非0，100ms内端口未被占用 |
| SEC-002 | API Key防暴力破解 | 大量错误key尝试 | 速率限制生效，恒定时间比较 |
| SEC-003 | `/metrics` 最小权限验证 | 检查返回字段 | 不含主机名/路径/进程详情 |
| SEC-003A | `/metrics` 禁止字段清单 | 显式检查 | 不含hostname/process_list/env_vars/cmdline/file_paths |
| SEC-004 | `--trend` 不泄露原始数据 | 检查输出 | 不含单点时间戳和原始值 |
| SEC-005 | 远程采集HTTPS警告 | `--remote http://...` | stderr有Warning |
| SEC-006 | history.jsonl脱敏 | 检查每行 | 不含可关联到具体机器的标识符 |

## 十二、测试覆盖率目标

| 模块 | 行覆盖率 | 分支覆盖率 |
|------|----------|------------|
| `environment/fingerprint.rs` | >95% | >90% |
| `environment/detector.rs` | >90% | >85% |
| `remote/server.rs` | >85% | >80% |
| `remote/client.rs` | >85% | >80% |
| `trends/store.rs` | >90% | >85% |
| `trends/analyzer.rs` | >90% | >85% |
| `container/detector.rs` | >85% | >80% |
| `multi_agent/view.rs` | >85% | >80% |

## 十三、执行计划

| 阶段 | 测试类型 | 责任人 | 时间 |
|------|----------|--------|------|
| W1-W2 | 环境指纹单元测试 | 开发 | 随编码 |
| W2-W3 | 远程采集单元测试 + 多线程验证 | 开发 | 随编码 |
| W2 | Docker多机测试环境搭建 | PMO+测试经理 | W2结束前 |
| W3-W4 | 容器适配单元测试 | 开发 | 随编码 |
| W4-W5 | 趋势分析单元测试 | 开发 | 随编码 |
| W5-W6 | 多Agent视图单元测试 | 开发 | 随编码 |
| W6-W7 | 集成测试 + 回归测试 | 测试经理 | W6开始 |
| W7-W8 | 多机协同专项 + 安全专项 + 手动验收 | 测试经理+安全 | W7-W8 |
| W8 | 覆盖率检查 + 补充 | 测试经理 | W8 |
| W9-W10 | Buffer + 发布 | 测试经理 | W9-W10 |

---

**测试经理总结**：V0.4 测试用例总计 **164 个**（单元 98 + 集成 16 + 回归 9 + 多机/迁移手动 8 + 网络安全 6 + 低配/容器 20 + 追加 7）。重点覆盖了四个首次引入的能力：环境指纹的生成/检测/迁移建议闭环、远程采集的服务端安全+客户端聚合、历史趋势的存储/分析/预言、容器适配的cgroup感知和低内存兜底。网络安全专项确保首次引入的网络服务不会成为攻击面。
