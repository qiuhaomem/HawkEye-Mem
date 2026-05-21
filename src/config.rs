use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub model: Option<ModelConfigSection>,
    pub directories: Option<DirectoriesConfig>,
    #[allow(dead_code)]
    pub gpu: Option<GpuConfig>,
    #[allow(dead_code)]
    pub calibration: Option<CalibrationConfig>,
    #[allow(dead_code)]
    pub state_machine: Option<StateMachineConfig>,
    #[allow(dead_code)]
    pub multi_agent: Option<MultiAgentConfig>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GpuConfig {
    pub rocm_smi_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CalibrationConfig {
    pub enabled: Option<bool>,
    pub max_samples: Option<usize>,
    pub min_samples_for_calibrated: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct StateMachineConfig {
    pub warning_seconds: Option<u64>,
    pub critical_seconds: Option<u64>,
    pub recovery_seconds: Option<u64>,
    pub min_samples_warning: Option<usize>,
    pub min_samples_critical: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct MultiAgentConfig {
    pub enabled: Option<bool>,
    pub extra_process_names: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DirectoriesConfig {
    pub model_cache: Option<String>,
    pub agent_process_names: Option<Vec<String>>,
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
        std::fs::write(&path, b"k v").unwrap();
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
        let tmp = tempfile::TempDir::new().unwrap();
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("HAWKEYE_MEM_CONFIG");
        let result = AppConfig::load(None);
        if let Some(ref h) = old_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        assert!(result.is_ok(), "默认路径无文件应返回Ok(None)");
        assert!(result.unwrap().is_none(), "应为None");
    }

    // UT-CF-005: 新配置段解析
    #[test]
    fn test_ut_cf_005_new_sections() {
        let dir = std::env::temp_dir().join("hawkeye_test_cf005");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        let toml_content = r#"
[calibration]
enabled = true
max_samples = 100
min_samples_for_calibrated = 10

[state_machine]
warning_seconds = 30
critical_seconds = 60
recovery_seconds = 120
min_samples_warning = 3
min_samples_critical = 5

[multi_agent]
enabled = true
extra_process_names = ["my-agent", "test-agent"]

[gpu]
rocm_smi_path = "/opt/rocm/bin/rocm-smi"
"#;
        std::fs::write(&path, toml_content).unwrap();
        let result = AppConfig::load(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "新配置段应正常解析: {:?}", result.err());
        let config = result.unwrap().unwrap();
        assert!(config.calibration.is_some(), "calibration 段应存在");
        assert!(config.state_machine.is_some(), "state_machine 段应存在");
        assert!(config.multi_agent.is_some(), "multi_agent 段应存在");
        assert!(config.gpu.is_some(), "gpu 段应存在");
        let ma = config.multi_agent.unwrap();
        assert_eq!(ma.extra_process_names.unwrap().len(), 2);
    }
}
