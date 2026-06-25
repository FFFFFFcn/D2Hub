use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub last_server: String,
    pub cleaned: bool,
    pub extra_args: Vec<String>,
    pub autostart: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_server: "cn".to_string(),
            cleaned: false,
            extra_args: Vec::new(),
            autostart: false,
        }
    }
}

fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
        .join("dota2switch");
    fs::create_dir_all(&dir).context("创建配置目录失败")?;
    Ok(dir.join("config.json"))
}

pub fn load() -> Result<AppConfig> {
    let path = config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path).context("读取配置失败")?;
        serde_json::from_str(&content).context("解析配置失败")
    } else {
        Ok(AppConfig::default())
    }
}

pub fn save(config: &AppConfig) -> Result<()> {
    let path = config_path()?;
    let content = serde_json::to_string_pretty(config).context("序列化配置失败")?;
    fs::write(&path, content).context("写入配置失败")
}
