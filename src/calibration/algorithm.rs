// 校准算法模块 - V0.3 加权平均 + confidence 升降级
// CR-07: confidence 首次升级庆祝提示
// CR-08: --calibration-stats 叙事性元素

use anyhow::Result;
use crate::calibration::{CalibrationPoint, CalibrationStore, hash_model_name};
use crate::collector::ResourceSnapshot;

/// 校准引擎 - 管理校准数据、confidence 升降级
pub struct CalibrationEngine<S: CalibrationStore> {
    store: S,
    /// 已触发过庆祝提示的模型（防重复）
    celebrated_models: std::collections::HashSet<String>,
}

#[allow(dead_code)]
impl<S: CalibrationStore> CalibrationEngine<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            celebrated_models: std::collections::HashSet::new(),
        }
    }

    /// 记录一次推理校准数据
    /// 首次升级到 Calibrated 时输出庆祝提示到 stderr（CR-07）
    pub fn record_inference(
        &mut self,
        point: CalibrationPoint,
        model_name: &str,
    ) -> Result<CalibrationStatus> {
        self.store.append(point, model_name)?;
        let model_hash = hash_model_name(model_name)?;
        let points = self.store.read_by_model(&model_hash)?;

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
}
