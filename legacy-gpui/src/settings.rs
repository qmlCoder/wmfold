//! 用户设置持久化（每种文件类型显示哪些化学列）。
//!
//! 存储位置：`%APPDATA%\wmfold\settings.json`（非 Windows 用 `$HOME`）。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::chem_columns::{chem_by_kind, FILTER_KINDS};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub column_settings: HashMap<String, Vec<String>>,
}

impl Settings {
    /// 默认设置：每种类型显示全部可用列
    pub fn defaults() -> Self {
        let mut column_settings = HashMap::new();
        for kind in FILTER_KINDS {
            column_settings.insert(
                (*kind).to_string(),
                chem_by_kind(kind).iter().map(|s| s.to_string()).collect(),
            );
        }
        Self { column_settings }
    }
}

/// 配置目录
pub fn config_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOME").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| ".".to_string());
    PathBuf::from(base).join("wmfold")
}

/// 配置文件路径
pub fn config_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// 读取设置，失败则返回默认值。
pub fn load() -> Settings {
    let mut settings = Settings::defaults();
    let path = config_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(saved) = serde_json::from_str::<Settings>(&text) {
            for (kind, cols) in saved.column_settings {
                // 只接受已知类型，且过滤掉未知列，避免坏配置导致界面异常。
                if FILTER_KINDS.contains(&kind.as_str()) {
                    let valid: Vec<String> = cols
                        .into_iter()
                        .filter(|c| chem_by_kind(&kind).contains(&c.as_str()))
                        .collect();
                    settings.column_settings.insert(kind, valid);
                }
            }
        }
    }
    settings
}

/// 保存设置（失败静默忽略，不影响使用）。
pub fn save(settings: &Settings) {
    let dir = config_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(text) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(config_path(), text);
    }
}
