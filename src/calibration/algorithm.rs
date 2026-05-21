// 校准算法模块 - V0.3 加权平均 + confidence 升降级
// CR-07: confidence 首次升级庆祝提示
// CR-08: --calibration-stats 叙事性元素
// T11: 加权平均修正算法 + CorrectedParams + CalibrationMeta
// T12: confidence 自动降级（连续3次偏差>30%）

use anyhow::Result;
use serde::Serialize;
use crate::calibration::{CalibrationPoint, CalibrationStore, hash_model_name};
use crate::collector::ResourceSnapshot;

/// 校准引擎 - 管理校准数据、confidence 升降级
pub struct CalibrationEngine<S: CalibrationStore> {
    store: S,
    /// 已触发过庆祝提示的模型（防重复）
    celebrated_models: std::collections::HashSet<String>,
    /// T12: 降级计数器 — 连续偏差 > 30% 的次数
    degradation_count: std::collections::HashMap<String, u32>,
}

#[allow(dead_code)]
impl<S: CalibrationStore> CalibrationEngine<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            celebrated_models: std::collections::HashSet::new(),
            degradation_count: std::collections::HashMap::new(),
        }
    }

    /// 记录一次推理校准数据，同时执行 T12 降级检测
    /// 首次升级到 Calibrated 时输出庆祝提示到 stderr（CR-07）
    /// 
    /// T12 降级逻辑：
    ///   连续 3 次实测偏差 > 30% → 清空该模型数据，重新学习
    ///   （在 record_inference_from_snapshots 中计算偏差后调用此方法时触发）
    pub fn record_inference(
        &mut self,
        point: CalibrationPoint,
        model_name: &str,
    ) -> Result<CalibrationStatus> {
        let bpt_saved = point.bytes_per_token;
        let model_hash = hash_model_name(model_name)?;

        // T12: 先读取插入前的基线（用于偏差对比）
        let baseline_points = self.store.read_by_model(&model_hash)?;
        let baseline_params = if baseline_points.len() >= 10 {
            CalibrationAlgorithm::compute(&baseline_points)
        } else {
            None
        };

        // 追加新数据点
        self.store.append(point, model_name)?;
        let points = self.store.read_by_model(&model_hash)?;

        // T12: 降级检测 — 用插入前的基线对比新数据点
        if let Some(corrected) = baseline_params {
            let deviation = if corrected.avg_bytes_per_token > 0 {
                (bpt_saved as f64 - corrected.avg_bytes_per_token as f64).abs()
                    / corrected.avg_bytes_per_token as f64
            } else {
                0.0
            };
            self.check_degradation(model_name, deviation)?;
        }

        let status = if points.len() >= 10 {
            CalibrationStatus::Calibrated {
                sample_count: points.len(),
            }
        } else {
            CalibrationStatus::Learning {
                sample_count: points.len(),
                needed: 10,
            }
        };

        // CR-07: 首次升级庆祝提示
        if matches!(status, CalibrationStatus::Calibrated { .. }) {
            if !self.celebrated_models.contains(&model_hash) {
                eprintln!(
                    "🎉 [hawk-eye-mem] {} 校准完成！你的 Agent 现在心里有数了 ({} 次采样)",
                    model_name,
                    points.len()
                );
                self.celebrated_models.insert(model_hash);
            }
        }

        Ok(status)
    }

    /// T12: 偏差检测 — 连续 3 次偏差 > 30% 则清空数据并降级
    fn check_degradation(&mut self, model_name: &str, deviation: f64) -> Result<()> {
        let model_hash = hash_model_name(model_name)?;
        let threshold = 0.30; // 30% 偏差阈值

        if deviation > threshold {
            let count = self
                .degradation_count
                .entry(model_hash.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);

            eprintln!(
                "[hawk-eye-mem] Warning: deviation {:.1}% exceeds 30% threshold ({} {}/3)",
                deviation * 100.0,
                if *count >= 3 { "DEGRADED" } else { "monitoring" },
                if *count >= 3 { 3 } else { *count },
            );

            if *count >= 3 {
                // 连续 3 次偏差过大 → 清空数据并降级
                eprintln!(
                    "[hawk-eye-mem] {} calibration degraded. Clearing data and restarting learning.",
                    model_name
                );
                self.store.clear_model(&model_hash)?;
                self.degradation_count.remove(&model_hash);
                self.celebrated_models.remove(&model_hash);
                return Ok(());
            }
        } else {
            // 偏差在阈值内，重置计数器
            self.degradation_count
                .entry(model_hash.clone())
                .and_modify(|c| *c = 0);
        }

        Ok(())
    }

    /// T11: 获取修正后的参数（加权平均 + confidence 判定）
    /// 需要在引擎外部使用以修改 EstimationEngine 的估算
    pub fn get_corrected_params(&self, model_name: &str) -> Result<Option<CorrectedParams>> {
        let model_hash = hash_model_name(model_name)?;
        let points = self.store.read_by_model(&model_hash)?;
        if points.is_empty() {
            return Ok(None);
        }
        Ok(CalibrationAlgorithm::compute(&points))
    }

    /// 获取已校准的 BPT 值（供外部使用）
    pub fn calibrated_bpt(&self, model_name: &str) -> Result<Option<u64>> {
        let params = self.get_corrected_params(model_name)?;
        Ok(params.map(|p| p.avg_bytes_per_token))
    }

    /// 从推理前后的资源快照计算并记录校准数据点（T5 校准采集集成）
    /// 返回记录的数据点，若跳过（tokens=0/噪声/缺内存数据/无模型名）则返回 None
    pub fn record_inference_from_snapshots(
        &mut self,
        before: &ResourceSnapshot,
        after: &ResourceSnapshot,
        tokens_processed: u64,
        model_name: &str,
    ) -> Result<Option<CalibrationPoint>> {
        if tokens_processed == 0 {
            return Ok(None);
        }
        if model_name.is_empty() {
            return Ok(None);
        }

        let before_mem = match before.memory {
            Some(ref m) => m.available_mb,
            None => return Ok(None),
        };
        let after_mem = match after.memory {
            Some(ref m) => m.available_mb,
            None => return Ok(None),
        };

        // delta_mb = before - after（内存增加时跳过）
        let delta_mb = before_mem.saturating_sub(after_mem);
        if delta_mb < 10 {
            return Ok(None); // 噪声过滤：<10MB 不记录
        }

        let bytes_per_token = (delta_mb as u64) * 1024 * 1024 / tokens_processed;
        let point = CalibrationPoint {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            bytes_per_token,
            tokens_processed,
        };

        self.record_inference(point.clone(), model_name)?;
        Ok(Some(point))
    }

    /// 获取校准统计信息（含叙事性进度条 - CR-08）
    pub fn stats(&self, model_name: &str) -> Result<CalibrationStats> {
        let model_hash = hash_model_name(model_name)?;
        let points = self.store.read_by_model(&model_hash)?;

        let avg_bpt = if points.is_empty() {
            0.0
        } else {
            points.iter().map(|p| p.bytes_per_token as f64).sum::<f64>() / points.len() as f64
        };

        let stage = if points.is_empty() {
            CalibrationStage::NotStarted
        } else if points.len() < 3 {
            CalibrationStage::JustStarted {
                count: points.len(),
            }
        } else if points.len() < 6 {
            CalibrationStage::Learning {
                count: points.len(),
                progress: points.len() as f64 / 10.0,
            }
        } else if points.len() < 10 {
            CalibrationStage::AlmostThere {
                count: points.len(),
                progress: points.len() as f64 / 10.0,
            }
        } else {
            CalibrationStage::Calibrated {
                count: points.len(),
            }
        };

        Ok(CalibrationStats {
            sample_count: points.len(),
            avg_bytes_per_token: avg_bpt,
            stage,
        })
    }

    /// T12: 重置指定模型的校准数据
    pub fn reset(&self, model_name: &str) -> Result<()> {
        let model_hash = hash_model_name(model_name)?;
        self.store.clear_model(&model_hash)?;
        eprintln!("Calibration data for \"{}\" has been reset.", model_name);
        Ok(())
    }
}

// ============================================================================
// T11: 加权平均修正算法
// ============================================================================

/// 加权平均修正算法 — 独立于特定存储实现
/// 最近 10 次线性衰减权重，最新 1.0，最旧 0.1
pub struct CalibrationAlgorithm;

impl CalibrationAlgorithm {
    /// 对校准数据进行加权平均计算
    /// 返回 None 当样本 < 3（不足以计算有效平均值）
    pub fn compute(points: &[CalibrationPoint]) -> Option<CorrectedParams> {
        if points.len() < 3 {
            return None;
        }

        // 取最近 10 条（按时间降序，即 points 末尾为最新）
        let take_n = points.len().min(10);
        let recent: Vec<&CalibrationPoint> = points.iter().rev().take(take_n).collect();
        let weights: Vec<f64> = (0..recent.len())
            .map(|i| 1.0 - i as f64 * 0.1) // 1.0, 0.9, 0.8, ..., 0.1
            .collect();

        let total_weight: f64 = weights.iter().sum();
        let avg: f64 = recent.iter().zip(&weights)
            .map(|(p, w)| p.bytes_per_token as f64 * w)
            .sum::<f64>() / total_weight;

        let variance: f64 = recent.iter().zip(&weights)
            .map(|(p, w)| w * (p.bytes_per_token as f64 - avg).powi(2))
            .sum::<f64>() / total_weight;
        let std_dev = variance.sqrt();
        let cv = if avg > 0.0 { std_dev / avg } else { 999.0 }; // 变异系数

        // T11: 样本数 ≥ 10 且 CV ≤ 20% → Calibrated
        let confidence = if points.len() >= 10 && cv <= 0.20 {
            Confidence::Calibrated
        } else {
            Confidence::Conservative
        };

        let safety_margin = if confidence == Confidence::Calibrated {
            10.0 // 校准后降至 10%
        } else {
            30.0 // 保守回退
        };

        // T11: 趋势判定（取最近 4 条：前2 vs 后2 均值比较）
        let trend = if recent.len() >= 4 {
            let half = recent.len() / 2;
            let older_half: f64 = recent[half..].iter()
                .map(|p| p.bytes_per_token as f64).sum::<f64>() / half as f64;
            let newer_half: f64 = recent[..half].iter()
                .map(|p| p.bytes_per_token as f64).sum::<f64>() / half as f64;
            let diff = (newer_half - older_half).abs();
            let mean = (older_half + newer_half) / 2.0;
            if diff < mean * 0.05 { "stable".to_string() }
            else if newer_half < older_half { "improving".to_string() }
            else { "degrading".to_string() }
        } else {
            "unknown".to_string()
        };

        Some(CorrectedParams {
            avg_bytes_per_token: avg.round() as u64,
            safety_margin,
            confidence,
            calibration: CalibrationMeta {
                samples: points.len(),
                avg_bytes_per_token: avg.round() as u64,
                std_dev: (std_dev * 10.0).round() / 10.0,
                trend,
            },
        })
    }
}

// ============================================================================
// T11: 修正参数结构体
// ============================================================================

/// 校准修正后的估算参数（T11）
#[derive(Debug, Clone, Serialize)]
pub struct CorrectedParams {
    pub avg_bytes_per_token: u64,
    pub safety_margin: f64,
    pub confidence: Confidence,
    pub calibration: CalibrationMeta,
}

/// 校准元数据（T11，嵌入 JSON 输出）
#[derive(Debug, Clone, Serialize)]
pub struct CalibrationMeta {
    pub samples: usize,
    pub avg_bytes_per_token: u64,
    pub std_dev: f64,
    pub trend: String,
}

/// T11/T12: Confidence 等级（重新导出，与 engine::Confidence 对齐）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Confidence {
    Conservative,
    Calibrated,
}

/// 校准状态
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationStatus {
    /// 学习中（样本 < 10）
    Learning {
        sample_count: usize,
        needed: usize,
    },
    /// 已校准（样本 ≥ 10）
    Calibrated {
        sample_count: usize,
    },
}

/// 校准阶段（CR-08 叙事性五阶段）
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CalibrationStage {
    NotStarted,
    JustStarted { count: usize },
    Learning { count: usize, progress: f64 },
    AlmostThere { count: usize, progress: f64 },
    Calibrated { count: usize },
}

impl std::fmt::Display for CalibrationStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalibrationStage::NotStarted => {
                write!(f, "还未开始 — 运行一次推理试试吧")
            }
            CalibrationStage::JustStarted { count } => {
                write!(f, "刚起步 ░░░░░░░░░░ {} 次 — 多跑几次推理就好了", count)
            }
            CalibrationStage::Learning { count, .. } => {
                write!(f, "学习中 ████░░░░░░ {} 次 — 越来越了解了", count)
            }
            CalibrationStage::AlmostThere { count, .. } => {
                write!(f, "快好了 ███████░░░ {} 次 — 还差最后几次", count)
            }
            CalibrationStage::Calibrated { count } => {
                write!(
                    f,
                    "已校准 ✅ {} 次 — 你的 Agent 现在心里有数了",
                    count
                )
            }
        }
    }
}

/// 校准统计（供 --calibration-stats 使用）
#[allow(dead_code)]
pub struct CalibrationStats {
    pub sample_count: usize,
    pub avg_bytes_per_token: f64,
    pub stage: CalibrationStage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::csv_store::CsvStore;
    use tempfile::TempDir;

    #[test]
    fn test_learning_status() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        let status = engine
            .record_inference(
                CalibrationPoint {
                    timestamp: 1,
                    bytes_per_token: 2048,
                    tokens_processed: 100,
                },
                "test-model",
            )
            .unwrap();

        assert!(matches!(
            status,
            CalibrationStatus::Learning {
                sample_count: 1,
                ..
            }
        ));
    }

    #[test]
    fn test_calibrated_after_10() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        for i in 0..10 {
            engine
                .record_inference(
                    CalibrationPoint {
                        timestamp: i,
                        bytes_per_token: 2048,
                        tokens_processed: 100,
                    },
                    "upgrade-test",
                )
                .unwrap();
        }

        let status = engine
            .record_inference(
                CalibrationPoint {
                    timestamp: 10,
                    bytes_per_token: 2000,
                    tokens_processed: 100,
                },
                "upgrade-test",
            )
            .unwrap();

        assert!(matches!(status, CalibrationStatus::Calibrated { .. }));
    }

    #[test]
    fn test_stage_display() {
        let stage = CalibrationStage::Learning {
            count: 5,
            progress: 0.5,
        };
        let text = format!("{}", stage);
        assert!(text.contains("学习中"));
        assert!(text.contains("5"));
    }

    #[test]
    fn test_stats_empty() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let engine = CalibrationEngine::new(store);

        let stats = engine.stats("new-model").unwrap();
        assert_eq!(stats.sample_count, 0);
    }

    #[test]
    fn test_stats_with_data() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        engine
            .record_inference(
                CalibrationPoint {
                    timestamp: 1,
                    bytes_per_token: 2000,
                    tokens_processed: 100,
                },
                "stats-test",
            )
            .unwrap();
        engine
            .record_inference(
                CalibrationPoint {
                    timestamp: 2,
                    bytes_per_token: 2100,
                    tokens_processed: 150,
                },
                "stats-test",
            )
            .unwrap();

        let stats = engine.stats("stats-test").unwrap();
        assert_eq!(stats.sample_count, 2);
        assert!((stats.avg_bytes_per_token - 2050.0).abs() < 0.1);
    }

    // ========== T5 校准采集集成测试 ==========

    fn make_mock_snapshot(available_mb: u64) -> crate::collector::ResourceSnapshot {
        use crate::collector::MemoryMetrics;
        crate::collector::ResourceSnapshot {
            memory: Some(MemoryMetrics {
                total_mb: 16000,
                used_mb: 16000 - available_mb,
                available_mb,
                used_percent: (16000 - available_mb) as f64 / 16000.0 * 100.0,
                pressure: crate::collector::PressureLevel::Low,
            }),
            disk: None,
            cpu: None,
            gpu: None,
            timestamp: "test".to_string(),
            collection_duration_ms: 0.0,
        }
    }

    #[test]
    fn test_record_from_snapshots_normal() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        let before = make_mock_snapshot(8000);  // 8000MB available
        let after = make_mock_snapshot(5000);   // 5000MB available → delta=3000MB

        let result = engine
            .record_inference_from_snapshots(&before, &after, 4096, "test-model")
            .unwrap();

        assert!(result.is_some(), "normal case should return Some");
        let point = result.unwrap();
        // bytes_per_token = 3000 * 1024 * 1024 / 4096 ≈ 768,000
        assert_eq!(point.bytes_per_token, 3000 * 1024 * 1024 / 4096);
        assert_eq!(point.tokens_processed, 4096);
    }

    #[test]
    fn test_record_from_snapshots_tokens_zero() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        let before = make_mock_snapshot(8000);
        let after = make_mock_snapshot(5000);

        let result = engine
            .record_inference_from_snapshots(&before, &after, 0, "test-model")
            .unwrap();
        assert!(result.is_none(), "tokens=0 should return None");
    }

    #[test]
    fn test_record_from_snapshots_noise_filter() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        let before = make_mock_snapshot(8000);
        let after = make_mock_snapshot(7995);  // delta=5MB < 10MB

        let result = engine
            .record_inference_from_snapshots(&before, &after, 4096, "test-model")
            .unwrap();
        assert!(result.is_none(), "delta<10 should be filtered as noise");
    }

    #[test]
    fn test_record_from_snapshots_empty_model() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        let before = make_mock_snapshot(8000);
        let after = make_mock_snapshot(5000);

        let result = engine
            .record_inference_from_snapshots(&before, &after, 4096, "")
            .unwrap();
        assert!(result.is_none(), "empty model name should return None");
    }

    #[test]
    fn test_record_from_snapshots_memory_increased() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        let before = make_mock_snapshot(5000);
        let after = make_mock_snapshot(8000);  // memory increased

        let result = engine
            .record_inference_from_snapshots(&before, &after, 4096, "test-model")
            .unwrap();
        assert!(result.is_none(), "memory increase should be skipped");
    }

    // ========== T11: 加权平均算法测试 ==========

    #[test]
    fn test_t11_weighted_average_10_samples() {
        // 10 条数据，全部相同的 bpt=2048，CV=0 → Calibrated
        let points: Vec<CalibrationPoint> = (0..10)
            .map(|i| CalibrationPoint {
                timestamp: i,
                bytes_per_token: 2048,
                tokens_processed: 100,
            })
            .collect();

        let result = CalibrationAlgorithm::compute(&points).unwrap();
        assert_eq!(result.avg_bytes_per_token, 2048);
        assert!((result.safety_margin - 10.0).abs() < 0.001, "margin should be 10%");
        assert_eq!(result.confidence, Confidence::Calibrated);
        assert_eq!(result.calibration.samples, 10);
        assert_eq!(result.calibration.trend, "stable");
        assert!(result.calibration.std_dev < 1.0, "std_dev should be near 0 for uniform data");
    }

    #[test]
    fn test_t11_weighted_average_fewer_than_3() {
        // 样本 < 3 → None
        let points = vec![
            CalibrationPoint { timestamp: 1, bytes_per_token: 2048, tokens_processed: 100 },
            CalibrationPoint { timestamp: 2, bytes_per_token: 2100, tokens_processed: 100 },
        ];
        let result = CalibrationAlgorithm::compute(&points);
        assert!(result.is_none(), "fewer than 3 samples should return None");
    }

    #[test]
    fn test_t11_weighted_average_high_cv() {
        // CV > 20% → Conservative
        let points: Vec<CalibrationPoint> = vec![
            CalibrationPoint { timestamp: 1, bytes_per_token: 4000, tokens_processed: 100 },
            CalibrationPoint { timestamp: 2, bytes_per_token: 1000, tokens_processed: 100 },
            CalibrationPoint { timestamp: 3, bytes_per_token: 3000, tokens_processed: 100 },
            CalibrationPoint { timestamp: 4, bytes_per_token: 500, tokens_processed: 100 },
            CalibrationPoint { timestamp: 5, bytes_per_token: 3500, tokens_processed: 100 },
            CalibrationPoint { timestamp: 6, bytes_per_token: 2000, tokens_processed: 100 },
            CalibrationPoint { timestamp: 7, bytes_per_token: 1500, tokens_processed: 100 },
            CalibrationPoint { timestamp: 8, bytes_per_token: 2500, tokens_processed: 100 },
            CalibrationPoint { timestamp: 9, bytes_per_token: 800, tokens_processed: 100 },
            CalibrationPoint { timestamp: 10, bytes_per_token: 3000, tokens_processed: 100 },
        ];
        let result = CalibrationAlgorithm::compute(&points).unwrap();
        assert_eq!(result.confidence, Confidence::Conservative);
        assert!((result.safety_margin - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_t11_trend_improving() {
        // 旧数据 bpt 高（差），新数据 bpt 低（好）
        let points: Vec<CalibrationPoint> = (0..10)
            .map(|i| CalibrationPoint {
                timestamp: i,
                bytes_per_token: 3000 - i * 100, // 3000, 2900, ..., 2100
                tokens_processed: 100,
            })
            .collect();
        let result = CalibrationAlgorithm::compute(&points).unwrap();
        assert_eq!(result.calibration.trend, "improving");
    }

    #[test]
    fn test_t11_trend_degrading() {
        // 旧数据 bpt 低（好），新数据 bpt 高（差）
        let points: Vec<CalibrationPoint> = (0..10)
            .map(|i| CalibrationPoint {
                timestamp: i,
                bytes_per_token: 2000 + i * 100, // 2000, 2100, ..., 2900
                tokens_processed: 100,
            })
            .collect();
        let result = CalibrationAlgorithm::compute(&points).unwrap();
        assert_eq!(result.calibration.trend, "degrading");
    }

    #[test]
    fn test_t11_weight_decay() {
        // 验证线性权重衰减：最新权重 > 最旧权重
        let points: Vec<CalibrationPoint> = (0..8)
            .map(|i| CalibrationPoint {
                timestamp: i,
                bytes_per_token: if i == 7 { 3000 } else { 2000 },
                tokens_processed: 100,
            })
            .collect();
        let result = CalibrationAlgorithm::compute(&points).unwrap();
        // 最新值 3000（权重 1.0），7 条 2000（权重 0.9-0.3），加权平均应偏向 3000
        assert!(result.avg_bytes_per_token > 2000, "weighted avg should favor recent higher value");
        assert!(result.avg_bytes_per_token < 3000, "but not equal to 3000 due to older values");
    }

    // ========== T12: 降级逻辑测试 ==========

    #[test]
    fn test_t12_degradation_3_consecutive_deviations() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        // 先建立稳定的 10 条基线数据 (bpt=2048)
        for i in 0..10 {
            engine
                .record_inference(
                    CalibrationPoint {
                        timestamp: i,
                        bytes_per_token: 2048,
                        tokens_processed: 100,
                    },
                    "degrade-test",
                )
                .unwrap();
        }

        // 验证现在是 Calibrated
        let params = engine.get_corrected_params("degrade-test").unwrap().unwrap();
        assert_eq!(params.confidence, Confidence::Calibrated);

        // 连续 3 次偏差 > 30%
        // 使用非常大的偏差值（bpt=4000，偏差 95%），确保不会因加权平均偏移而低于 30%
        for i in 0..3 {
            engine
                .record_inference(
                    CalibrationPoint {
                        timestamp: 11 + i,
                        bytes_per_token: 4000,
                        tokens_processed: 100,
                    },
                    "degrade-test",
                )
                .unwrap();
        }

        // 第 3 次后数据应被清空
        let params_after = engine.get_corrected_params("degrade-test").unwrap();
        assert!(params_after.is_none(), "data should be cleared after 3 degradations");
    }

    #[test]
    fn test_t12_no_degradation_for_normal_data() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        // 基线数据
        for i in 0..10 {
            engine
                .record_inference(
                    CalibrationPoint {
                        timestamp: i,
                        bytes_per_token: 2000,
                        tokens_processed: 100,
                    },
                    "normal-test",
                )
                .unwrap();
        }

        // 正常数据点（偏差很小）
        engine
            .record_inference(
                CalibrationPoint {
                    timestamp: 11,
                    bytes_per_token: 2050, // 仅 2.5% 偏差
                    tokens_processed: 100,
                },
                "normal-test",
            )
            .unwrap();

        let params = engine.get_corrected_params("normal-test").unwrap();
        assert!(params.is_some(), "data should not be cleared for normal deviation");
    }

    #[test]
    fn test_t12_reset_calibration() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);
        let mut engine = CalibrationEngine::new(store);

        // 记录一些数据
        for i in 0..5 {
            engine
                .record_inference(
                    CalibrationPoint {
                        timestamp: i,
                        bytes_per_token: 2048,
                        tokens_processed: 100,
                    },
                    "reset-test",
                )
                .unwrap();
        }

        // 重置
        engine.reset("reset-test").unwrap();

        let params = engine.get_corrected_params("reset-test").unwrap();
        assert!(params.is_none(), "data should be cleared after reset");
    }
}