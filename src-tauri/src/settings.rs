//! 用户设置持久化（每种文件类型显示哪些化学列）。
//!
//! 存储位置：`%APPDATA%\wmfold\settings.json`（非 Windows 用 `$HOME`）。
//! JSON 结构与 0.3.4 的 GPUI 版保持兼容：
//! `{ "column_settings": { "gjf": ["formula", ...], ... } }`

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 参与筛选 / 设置的类型代码（与前端 `src/utils/chemColumns.ts` 的 FILTER_KINDS 一致）。
const FILTER_KINDS: &[&str] = &[
    "gjf", "log", "out", "fch", "xyz", "mol", "sdf", "molden", "dmo", "inp", "cub",
];

/// 每种类型可提供的化学列（与前端 `CHEM_BY_KIND` 一致）。
fn available_columns(kind: &str) -> &'static [&'static str] {
    match kind {
        "gjf" => &["formula", "nAtoms", "charge", "multip", "molW"],
        "log" | "out" => &["formula", "nAtoms", "charge", "multip", "energy", "molW"],
        "fch" => &["formula", "nAtoms", "charge", "multip", "nBasis", "molW"],
        "inp" => &["formula", "nAtoms", "charge", "multip", "molW"],
        "mol" | "sdf" | "xyz" | "dmo" => &["formula", "nAtoms", "molW"],
        "molden" => &["formula", "nAtoms", "multip", "molW"],
        "cub" => &["nAtoms"],
        _ => &[],
    }
}

/// 用户设置：key 为文件类型代码，value 为该类型要显示的化学列 key 列表。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// 注意：保持 snake_case 键名，与 0.3.4 GPUI 版写出的 settings.json 兼容。
    #[serde(default)]
    pub column_settings: HashMap<String, Vec<String>>,
}

impl Settings {
    /// 默认设置：每种类型显示全部可用列。
    pub fn defaults() -> Self {
        let mut column_settings = HashMap::new();
        for kind in FILTER_KINDS {
            column_settings.insert(
                (*kind).to_string(),
                available_columns(kind)
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            );
        }
        Self { column_settings }
    }
}

/// 校验并补全读到的设置：只保留已知类型与已知列，缺失的类型用默认列补齐。
fn sanitize(saved: Settings) -> Settings {
    let mut settings = Settings::defaults();
    for (kind, cols) in saved.column_settings {
        if !FILTER_KINDS.contains(&kind.as_str()) {
            continue;
        }
        let allowed = available_columns(&kind);
        let valid: Vec<String> = cols
            .into_iter()
            .filter(|c| allowed.contains(&c.as_str()))
            .collect();
        settings.column_settings.insert(kind, valid);
    }
    settings
}

/// 配置目录。
pub fn config_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOME").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| ".".to_string());
    PathBuf::from(base).join("wmfold")
}

/// 配置文件路径。
pub fn config_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// 从磁盘读取设置；文件不存在或损坏时返回默认值。
pub fn load_settings_inner() -> Settings {
    match std::fs::read_to_string(config_path()) {
        Ok(text) => serde_json::from_str::<Settings>(&text)
            .map(sanitize)
            .unwrap_or_else(|_| Settings::defaults()),
        Err(_) => Settings::defaults(),
    }
}

/// 保存设置到磁盘。
pub fn save_settings_inner(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(config_path(), text).map_err(|e| e.to_string())
}

// ---- Tauri 命令 ----

/// 读取用户设置（每种类型显示的化学列）。
#[tauri::command]
pub fn load_settings() -> Settings {
    load_settings_inner()
}

/// 保存用户设置。
#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    save_settings_inner(&settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_kind() {
        let s = Settings::defaults();
        for kind in FILTER_KINDS {
            assert!(
                s.column_settings.contains_key(*kind),
                "缺少类型 {kind} 的默认列"
            );
        }
        assert_eq!(s.column_settings["cub"], vec!["nAtoms".to_string()]);
        assert_eq!(
            s.column_settings["fch"],
            vec![
                "formula".to_string(),
                "nAtoms".to_string(),
                "charge".to_string(),
                "multip".to_string(),
                "nBasis".to_string(),
                "molW".to_string()
            ]
        );
    }

    #[test]
    fn sanitize_filters_unknown_kinds_and_columns() {
        let mut saved = Settings::default();
        saved.column_settings.insert(
            "xyz".to_string(),
            vec!["formula".to_string(), "bogus".to_string()],
        );
        saved
            .column_settings
            .insert("not-a-kind".to_string(), vec!["formula".to_string()]);

        let out = sanitize(saved);
        assert_eq!(out.column_settings["xyz"], vec!["formula".to_string()]);
        assert!(!out.column_settings.contains_key("not-a-kind"));
        // 没有在文件里出现的类型仍然有默认列
        assert_eq!(out.column_settings["cub"], vec!["nAtoms".to_string()]);
    }

    #[test]
    fn serializer_uses_snake_case_key() {
        let s = Settings::defaults();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"column_settings\""), "JSON 键名应保持兼容");
    }
}
