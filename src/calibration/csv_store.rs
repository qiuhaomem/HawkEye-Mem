// Copyright 2026 秋毫mem Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// CsvStore - CalibrationStore的CSV文件实现
// 文件路径：~/.config/hawk-eye-mem/calibration/<model_hash>.csv
// 格式：timestamp,bytes_per_token,tokens_processed
// 并发保护：flock独占锁（锁失败放弃写入不阻塞）
// 数据上限：max_samples条，超出删除最旧的half条

use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use super::{hash_model_name, CalibrationPoint, CalibrationStore};

pub struct CsvStore {
    base_path: PathBuf,
    max_samples: usize,
}

impl CsvStore {
    /// base_path: 校准数据目录，如 ~/.config/hawk-eye-mem/calibration/
    pub fn new(base_path: PathBuf, max_samples: usize) -> Self {
        Self {
            base_path,
            max_samples,
        }
    }

    /// 获取模型对应的CSV文件路径
    fn csv_path(&self, model_hash: &str) -> PathBuf {
        self.base_path.join(format!("{}.csv", model_hash))
    }

    /// 确保目录存在
    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_path)
            .context("Failed to create calibration directory")?;
        Ok(())
    }

    /// 尝试获取文件独占锁（立即返回，不阻塞），成功true/失败false
    fn try_lock(file: &File) -> bool {
        file.try_lock_exclusive().is_ok()
    }

    /// 读取CSV所有行并解析
    fn read_all(&self, path: &PathBuf) -> Result<Vec<CalibrationPoint>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut points = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 3 {
                continue;
            }

            if let (Ok(ts), Ok(bpt), Ok(tok)) = (
                parts[0].trim().parse::<u64>(),
                parts[1].trim().parse::<u64>(),
                parts[2].trim().parse::<u64>(),
            ) {
                points.push(CalibrationPoint {
                    timestamp: ts,
                    bytes_per_token: bpt,
                    tokens_processed: tok,
                });
            }
        }

        Ok(points)
    }

    /// 如果超出max_samples，删除最旧的一半
    fn trim(&self, path: &PathBuf, points: &mut Vec<CalibrationPoint>) -> Result<()> {
        if points.len() <= self.max_samples {
            return Ok(());
        }
        // 按时间排序（应该已经有序，但确保一下）
        points.sort_by_key(|p| p.timestamp);
        let keep = points.split_off(points.len() - self.max_samples / 2);
        *points = keep;

        // 重写文件
        self.write_all(path, points)?;
        Ok(())
    }

    /// 覆盖写入所有点
    fn write_all(&self, path: &PathBuf, points: &[CalibrationPoint]) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        for p in points {
            writeln!(
                &file,
                "{},{},{}",
                p.timestamp, p.bytes_per_token, p.tokens_processed
            )?;
        }
        Ok(())
    }
}

impl CalibrationStore for CsvStore {
    fn append(&self, point: CalibrationPoint, model_name: &str) -> Result<()> {
        self.ensure_dir()?;
        let model_hash = hash_model_name(model_name)?;
        let path = self.csv_path(&model_hash);

        // flock独占锁，失败则放弃写入
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .context("Failed to open calibration CSV")?;

        if !Self::try_lock(&file) {
            eprintln!("[hawk-eye-mem] Warning: calibration file locked, skipping append");
            return Ok(());
        }

        writeln!(
            &file,
            "{},{},{}",
            point.timestamp, point.bytes_per_token, point.tokens_processed
        )
        .context("Failed to write calibration data")?;

        // 解锁
        let _ = file.unlock();

        // 检查是否需要裁剪
        let mut points = self.read_all(&path)?;
        if points.len() > self.max_samples {
            self.trim(&path, &mut points)?;
        }

        Ok(())
    }

    fn read_by_model(&self, model_hash: &str) -> Result<Vec<CalibrationPoint>> {
        let path = self.csv_path(model_hash);
        self.read_all(&path)
    }

    fn clear_model(&self, model_hash: &str) -> Result<()> {
        let path = self.csv_path(model_hash);
        if path.exists() {
            std::fs::remove_file(&path).context("Failed to remove calibration CSV")?;
        }
        Ok(())
    }

    fn model_hashes(&self) -> Result<Vec<String>> {
        self.ensure_dir()?;
        let mut hashes = Vec::new();
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".csv") {
                    hashes.push(name.trim_end_matches(".csv").to_string());
                }
            }
        }
        Ok(hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_append_and_read() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);

        let point = CalibrationPoint {
            timestamp: 1000,
            bytes_per_token: 2048,
            tokens_processed: 4096,
        };
        store.append(point.clone(), "test-model").unwrap();

        let hash = hash_model_name("test-model").unwrap();
        let points = store.read_by_model(&hash).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tokens_processed, 4096);
    }

    #[test]
    fn test_clear_model() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 100);

        store
            .append(
                CalibrationPoint {
                    timestamp: 1,
                    bytes_per_token: 2048,
                    tokens_processed: 100,
                },
                "m1",
            )
            .unwrap();
        store
            .append(
                CalibrationPoint {
                    timestamp: 2,
                    bytes_per_token: 2048,
                    tokens_processed: 200,
                },
                "m2",
            )
            .unwrap();

        let hash = hash_model_name("m1").unwrap();
        store.clear_model(&hash).unwrap();

        let hashes = store.model_hashes().unwrap();
        assert_eq!(hashes.len(), 1);
    }

    #[test]
    fn test_trim_excess() {
        let tmp = TempDir::new().unwrap();
        let store = CsvStore::new(tmp.path().to_path_buf(), 10);

        for i in 0..20 {
            store
                .append(
                    CalibrationPoint {
                        timestamp: i,
                        bytes_per_token: 2048,
                        tokens_processed: 100 + i,
                    },
                    "trim-test",
                )
                .unwrap();
        }

        let hash = hash_model_name("trim-test").unwrap();
        let points = store.read_by_model(&hash).unwrap();
        assert!(points.len() <= 10, "Should be trimmed to max_samples");
    }
}
