use chrono::{DateTime, Duration, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;

// ============================================================================
// 数据结构
// ============================================================================

/// 单次历史数据点
/// CR-10: 不输出原始数据点（仅内部存储使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct HistoryPoint {
    pub timestamp: String, // RFC3339
    pub memory_available_mb: u64,
    pub memory_pressure: String,
    pub cpu_load: f64,
    pub disk_available_mb: u64,
    /// 本次推理实际处理的 token 数（可选，配合校准数据使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_processed: Option<u64>,
}

/// 趋势分析报告
/// CR-10: 只包含聚合结果，不包含原始数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendReport {
    pub direction: String, // increasing / stable / decreasing
    pub slope_mb_per_minute: f64,
    pub r_squared: f64,
    pub days_until_critical: Option<f64>,
    pub confidence: String, // low / medium / high
    pub urgency: String,    // low / medium / high / critical
    pub data_points: usize,
}

// ============================================================================
// HistoryStore: JSONL 存储 + flock 保护
// ============================================================================

#[allow(dead_code)]
pub struct HistoryStore {
    path: PathBuf,
    retention_days: u64,
}

#[allow(dead_code)]
impl HistoryStore {
    /// 创建默认路径的存储（~/.config/hawk-eye-mem/history.jsonl）
    pub fn new(retention_days: u64) -> Self {
        let home = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let dir = home.join(".config/hawk-eye-mem");
        let _ = fs::create_dir_all(&dir);
        Self {
            path: dir.join("history.jsonl"),
            retention_days,
        }
    }

    /// 指定路径的存储（用于测试）
    pub fn with_path(path: PathBuf, retention_days: u64) -> Self {
        Self { path, retention_days }
    }

    /// 追加写入一个数据点（flock 保护）
    pub fn record(&self, point: &HistoryPoint) -> Result<(), String> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("Failed to open history file: {}", e))?;

        file.lock_exclusive()
            .map_err(|e| format!("Failed to lock history file: {}", e))?;

        let line = serde_json::to_string(point)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        writeln!(&file, "{}", line)
            .map_err(|e| format!("Failed to write: {}", e))?;

        file.unlock()
            .map_err(|e| format!("Failed to unlock: {}", e))?;

        Ok(())
    }

    /// 读取所有历史数据点
    pub fn read_all(&self) -> Result<Vec<HistoryPoint>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)
            .map_err(|e| format!("Failed to open history file: {}", e))?;

        file.lock_shared()
            .map_err(|e| format!("Failed to lock history file: {}", e))?;

        let reader = BufReader::new(&file);
        let mut points = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<HistoryPoint>(&line) {
                Ok(p) => points.push(p),
                Err(e) => {
                    eprintln!(
                        "[hawk-eye-mem] Warning: skipping malformed history line: {}",
                        e
                    );
                }
            }
        }

        file.unlock()
            .map_err(|e| format!("Failed to unlock: {}", e))?;

        Ok(points)
    }

    /// 清理超期数据（500ms 超时保护）
    pub fn cleanup(&self) -> Result<usize, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|e| format!("Failed to open history file: {}", e))?;

        // 500ms 超时重试
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            match file.try_lock_exclusive() {
                Ok(_) => break,
                Err(_) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(
                            "Lock timeout: could not acquire exclusive lock within 500ms"
                                .to_string(),
                        );
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }

        let cutoff = Utc::now() - Duration::days(self.retention_days as i64);
        let reader = BufReader::new(&file);
        let mut kept_lines: Vec<String> = Vec::new();
        let mut removed = 0usize;

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(point) = serde_json::from_str::<HistoryPoint>(&line) {
                if let Ok(ts) = point.timestamp.parse::<DateTime<Utc>>() {
                    if ts < cutoff {
                        removed += 1;
                        continue;
                    }
                }
            }
            kept_lines.push(line);
        }

        // 截断并重写
        file.set_len(0)
            .map_err(|e| format!("Failed to truncate: {}", e))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Failed to seek: {}", e))?;
        for line in &kept_lines {
            writeln!(&file, "{}", line)
                .map_err(|e| format!("Failed to write: {}", e))?;
        }

        file.unlock()
            .map_err(|e| format!("Failed to unlock: {}", e))?;

        Ok(removed)
    }

    /// 清空所有历史数据
    pub fn clear(&self) -> Result<(), String> {
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.path)
            .map_err(|e| format!("Failed to open history file: {}", e))?;

        file.lock_exclusive()
            .map_err(|e| format!("Failed to lock history file: {}", e))?;

        file.set_len(0)
            .map_err(|e| format!("Failed to truncate: {}", e))?;

        file.unlock()
            .map_err(|e| format!("Failed to unlock: {}", e))?;

        Ok(())
    }

    /// 获取数据点数量
    pub fn count(&self) -> Result<usize, String> {
        Ok(self.read_all()?.len())
    }
}

// ============================================================================
// TrendAnalyzer: 线性回归趋势分析
// ============================================================================

pub struct TrendAnalyzer;

impl TrendAnalyzer {
    /// 分析内存趋势。
    /// 不足 10 个数据点返回 None。
    /// CR-10: 不输出原始数据点。
    pub fn analyze(points: &[HistoryPoint]) -> Option<TrendReport> {
        if points.len() < 10 {
            return None;
        }

        // 按时间戳排序
        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let first_ts = sorted[0].timestamp.parse::<DateTime<Utc>>().ok()?;
        let mut x_vals: Vec<f64> = Vec::with_capacity(sorted.len());
        let mut y_vals: Vec<f64> = Vec::with_capacity(sorted.len());

        for point in &sorted {
            let ts = point.timestamp.parse::<DateTime<Utc>>().ok()?;
            let minutes = (ts - first_ts).num_seconds() as f64 / 60.0;
            x_vals.push(minutes);
            y_vals.push(point.memory_available_mb as f64);
        }

        let n = x_vals.len() as f64;
        let sum_x: f64 = x_vals.iter().sum();
        let sum_y: f64 = y_vals.iter().sum();
        let sum_xy: f64 = x_vals.iter().zip(y_vals.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_vals.iter().map(|x| x * x).sum();

        // 斜率: b = (n*sum_xy - sum_x*sum_y) / (n*sum_x2 - sum_x*sum_x)
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return Some(TrendReport {
                direction: "stable".to_string(),
                slope_mb_per_minute: 0.0,
                r_squared: 0.0,
                days_until_critical: None,
                confidence: "low".to_string(),
                urgency: "low".to_string(),
                data_points: points.len(),
            });
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;
        let intercept = (sum_y - slope * sum_x) / n;

        // R-squared: 1 - SS_res / SS_tot
        let mean_y = sum_y / n;
        let ss_res: f64 = y_vals
            .iter()
            .zip(x_vals.iter())
            .map(|(y, x)| {
                let predicted = slope * x + intercept;
                (y - predicted).powi(2)
            })
            .sum();
        let ss_tot: f64 = y_vals.iter().map(|y| (y - mean_y).powi(2)).sum();

        let r_squared = if ss_tot.abs() < 1e-10 {
            0.0
        } else {
            1.0 - ss_res / ss_tot
        };

        // 方向判定
        let direction = if slope > 0.5 {
            "increasing"
        } else if slope < -0.5 {
            "decreasing"
        } else {
            "stable"
        }
        .to_string();

        // 到达临界的天数（仅下降趋势）
        let days_until_critical = if slope < 0.0 {
            let current_mb = sorted.last().map(|p| p.memory_available_mb as f64).unwrap_or(0.0);
            let abs_slope = slope.abs();
            if abs_slope > 0.0 && current_mb > 0.0 {
                let minutes_until = current_mb / abs_slope;
                let days = minutes_until / (60.0 * 24.0);
                Some(days)
            } else {
                None
            }
        } else {
            None
        };

        // 置信度: 基于 r_squared + 样本量
        let confidence = {
            let base = if r_squared >= 0.8 {
                "high"
            } else if r_squared >= 0.5 {
                "medium"
            } else {
                "low"
            };

            // 样本量修正
            if points.len() >= 30 && base == "medium" {
                "high"
            } else if points.len() < 20 && base == "high" {
                "medium"
            } else {
                base
            }
        }
        .to_string();

        // 紧急程度 (CR-05): 基于斜率 + 当前可用内存
        let current_mb = sorted.last().map(|p| p.memory_available_mb as f64).unwrap_or(0.0);
        let urgency = if slope < -5.0 && current_mb < 500.0 {
            "critical"
        } else if slope < -2.0 && current_mb < 1000.0 {
            "high"
        } else if slope < -0.5 {
            "medium"
        } else {
            "low"
        }
        .to_string();

        Some(TrendReport {
            direction,
            slope_mb_per_minute: slope,
            r_squared,
            days_until_critical,
            confidence,
            urgency,
            data_points: points.len(),
        })
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_point(timestamp: &str, available_mb: u64) -> HistoryPoint {
        HistoryPoint {
            timestamp: timestamp.to_string(),
            memory_available_mb: available_mb,
            memory_pressure: "low".to_string(),
            cpu_load: 0.5,
            disk_available_mb: 50000,
            tokens_processed: None,
        }
    }

    // UT-TR-001: < 10 点 → insufficient data (None)
    #[test]
    fn test_ut_tr_001_insufficient_data() {
        let points: Vec<HistoryPoint> = (0..9)
            .map(|i| {
                make_point(
                    &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                    8000 - i * 100,
                )
            })
            .collect();
        assert_eq!(points.len(), 9);
        assert!(TrendAnalyzer::analyze(&points).is_none());
    }

    // UT-TR-002: 恰好 10 点 → 应输出分析
    #[test]
    fn test_ut_tr_002_exactly_10_points() {
        let points: Vec<HistoryPoint> = (0..10)
            .map(|i| {
                make_point(
                    &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                    8000 - i * 100,
                )
            })
            .collect();
        assert!(TrendAnalyzer::analyze(&points).is_some());
    }

    // UT-TR-003: 下降趋势检测
    #[test]
    fn test_ut_tr_003_decreasing_trend() {
        let mut points = Vec::new();
        for i in 0..20 {
            let mb = 10000 - i * 300;
            points.push(make_point(
                &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                mb.max(0) as u64,
            ));
        }
        let report = TrendAnalyzer::analyze(&points).unwrap();
        assert_eq!(report.direction, "decreasing");
        assert!(report.slope_mb_per_minute < 0.0);
        assert!(report.days_until_critical.is_some());
    }

    // UT-TR-004: 稳定趋势检测
    #[test]
    fn test_ut_tr_004_stable_trend() {
        let mut points = Vec::new();
        for i in 0..20 {
            let mb = 8000i64 + (i as i64 % 3 - 1) * 50;
            points.push(make_point(
                &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                mb.max(0) as u64,
            ));
        }
        let report = TrendAnalyzer::analyze(&points).unwrap();
        assert_eq!(report.direction, "stable");
    }

    // UT-TR-005: 上升趋势
    #[test]
    fn test_ut_tr_005_increasing_trend() {
        let mut points = Vec::new();
        for i in 0..20 {
            let mb = 4000 + i * 200;
            points.push(make_point(
                &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                mb,
            ));
        }
        let report = TrendAnalyzer::analyze(&points).unwrap();
        assert_eq!(report.direction, "increasing");
        assert!(report.slope_mb_per_minute > 0.0);
    }

    // UT-TR-006: 置信度基于 r_squared
    #[test]
    fn test_ut_tr_006_confidence_levels() {
        // 完美线性下降 → 高置信度
        let mut points = Vec::new();
        for i in 0..25 {
            points.push(make_point(
                &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                10000 - i * 200,
            ));
        }
        let report = TrendAnalyzer::analyze(&points).unwrap();
        assert_eq!(
            report.confidence, "high",
            "perfect linear fit should give high confidence"
        );
    }

    // UT-TR-007: 紧急程度 (CR-05)
    #[test]
    fn test_ut_tr_007_urgency_critical() {
        let mut points = Vec::new();
        for i in 0..15 {
            points.push(make_point(
                &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                (500i64 - i as i64 * 30).max(0) as u64,
            ));
        }
        let report = TrendAnalyzer::analyze(&points).unwrap();
        assert_eq!(
            report.urgency, "critical",
            "rapid decrease + low memory should be critical"
        );
    }

    // UT-TR-008: CR-10 — 不输出原始数据点
    #[test]
    fn test_ut_tr_008_no_raw_data() {
        let mut points = Vec::new();
        for i in 0..15 {
            points.push(make_point(
                &format!("2026-05-{:02}T10:{:02}:00+08:00", 1 + i / 60, i % 60),
                8000 - i * 100,
            ));
        }
        let report = TrendAnalyzer::analyze(&points).unwrap();
        let json = serde_json::to_value(&report).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("raw_points"), "should not contain raw data");
        assert!(!obj.contains_key("data"), "should not contain data field");
    }

    // UT-TR-009: HistoryStore record + read 往返
    #[test]
    fn test_ut_tr_009_store_roundtrip() {
        let dir = std::env::temp_dir().join("hawkeye_test_tr009");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("history.jsonl");
        let store = HistoryStore::with_path(path.clone(), 30);

        let point = make_point("2026-05-20T10:00:00+08:00", 8000);
        store.record(&point).unwrap();

        let points = store.read_all().unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].memory_available_mb, 8000);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // UT-TR-010: HistoryStore clear
    #[test]
    fn test_ut_tr_010_store_clear() {
        let dir = std::env::temp_dir().join("hawkeye_test_tr010");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("history.jsonl");
        let store = HistoryStore::with_path(path.clone(), 30);

        let point = make_point("2026-05-20T10:00:00+08:00", 8000);
        store.record(&point).unwrap();
        assert_eq!(store.count().unwrap(), 1);

        store.clear().unwrap();
        assert_eq!(store.count().unwrap(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // UT-TR-011: HistoryStore cleanup 超期数据
    #[test]
    fn test_ut_tr_011_store_cleanup() {
        let dir = std::env::temp_dir().join("hawkeye_test_tr011");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("history.jsonl");
        let store = HistoryStore::with_path(path.clone(), 7); // 7 天保留

        // 超期点（15 天前）
        let old_ts = (Utc::now() - Duration::days(15)).to_rfc3339();
        store.record(&make_point(&old_ts, 8000)).unwrap();

        // 新点（当前）
        let new_ts = Utc::now().to_rfc3339();
        store.record(&make_point(&new_ts, 7500)).unwrap();

        assert_eq!(store.count().unwrap(), 2);

        let removed = store.cleanup().unwrap();
        assert_eq!(removed, 1, "should remove 1 old point");

        let remaining = store.read_all().unwrap();
        assert_eq!(remaining.len(), 1, "should keep 1 recent point");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
