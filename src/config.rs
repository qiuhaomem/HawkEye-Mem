use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub model: Option<ModelConfigSection>,
}

#[derive(Debug, Deserialize)]
pub struct ModelConfigSection {
    pub bytes_per_token: Option<u64>,
    pub margin: Option<f64>,
}

impl AppConfig {
    pub fn load(path: Option<&str>) -> Result<Option<Self>> {
        let config_path = match path {
            Some(p) => PathBuf::from(p),
            None => {
                if let Ok(env_path) = std::env::var("HAWKEYE_MEM_CONFIG") {
                    PathBuf::from(env_path)
                } else {
                    let home = dirs_next::home_dir()
                        .context("Cannot determine home directory")?;
                    home.join(".config/hawk-eye-mem/config.toml")
                }
            }
        };

        if !config_path.exists() {
            if path.is_some() {
                return Err(anyhow::anyhow!(
                    "Failed to read config file: {}",
                    config_path.display()
                ));
            }
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: AppConfig = toml::from_str(&content)
            .with_context(|| "Failed to parse config")?;

        Ok(Some(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // UT-CF-001: 有效TOML加载
    #[test]
    fn test_ut_cf_001_valid_toml() {
        let dir = std::env::temp_dir().join("hawkeye_test_cf001");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        std::fs::write(&path, b"[model]\nbytes_per_token = 4096").unwrap();
        let result = AppConfig::load(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "有效TOML应返回Ok");
        let config = result.unwrap();
        assert!(config.is_some(), "应有配置内容");
        assert_eq!(config.unwrap().model.unwrap().bytes_per_token.unwrap(), 4096);
    }

    // UT-CF-002: 无效TOML格式
    #[test]
    fn test_ut_cf_002_invalid_toml() {
        let dir = std::env::temp_dir().join("hawkeye_test_cf002");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        std::fs::write(&path, b"k v").unwrap();  // TOML需要等号
        let result = AppConfig::load(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err(), "无效TOML应返回Err");
        let err_msg = result.unwrap_err().to_string().to_lowercase();
        assert!(err_msg.contains("parse"), "错误消息应包含'parse': {}", err_msg);
    }

    // UT-CF-003: 文件不存在
    #[test]
    fn test_ut_cf_003_file_not_found() {
        let result = AppConfig::load(Some("/nonexistent/path/config.toml"));
        assert!(result.is_err(), "显式路径不存在应返回Err");
        assert!(result.unwrap_err().to_string().contains("Failed to read config file"));
    }

    // UT-CF-004: 默认路径无文件静默跳过
    #[test]
    fn test_ut_cf_004_default_path_silent() {
        std::env::remove_var("HAWKEYE_MEM_CONFIG");
        let result = AppConfig::load(None);
        if result.is_ok() {
            let config = result.unwrap();
            assert!(config.is_none(), "无配置路径时应为None");
        }
    }

    // UT-CF-005: 环境变量覆盖默认路径
    #[test]
    fn test_ut_cf_005_env_override() {
        let dir = std::env::temp_dir().join("hawkeye_test_cf005");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        std::fs::write(&path, b"[model]\nbytes_per_token = 3000").unwrap();
        std::env::set_var("HAWKEYE_MEM_CONFIG", path.to_str().unwrap());
        let result = AppConfig::load(None);
        std::env::remove_var("HAWKEYE_MEM_CONFIG");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "环境变量指定路径应返回Ok");
        if let Ok(Some(config)) = result {
            assert_eq!(config.model.unwrap().bytes_per_token.unwrap(), 3000);
        }
    }
}
