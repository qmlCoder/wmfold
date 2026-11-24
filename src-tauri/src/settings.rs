//! 用户设置持久化（化学列展示 + 程序设置）。
//!
//! 存储位置：`%APPDATA%\wmfold\settings.json`（非 Windows 用 `$HOME`）。
//! JSON 结构与 0.3.4 的 GPUI 版保持兼容：
//! `{ "column_settings": { "gjf": ["formula", ...], ... }, "show_hidden": false }`

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use wmfold_core::columns::{available_columns, FILTER_KINDS};

/// 用户设置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// 注意：保持 snake_case 键名，与 0.3.4 GPUI 版写出的 settings.json 兼容。
    #[serde(default)]
    pub column_settings: HashMap<String, Vec<String>>,
    /// 程序设置：是否显示隐藏文件（Windows 隐藏属性 / Linux 点文件）。
    #[serde(default)]
    pub show_hidden: bool,
    /// 程序设置：外部终端工具（auto / wt / powershell / cmd）。
    #[serde(default)]
    pub terminal_tool: String,
}

/// 允许的外部终端工具。
pub const TERMINAL_TOOLS: [&str; 4] = ["auto", "wt", "powershell", "cmd"];

fn normalize_terminal_tool(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if TERMINAL_TOOLS.contains(&value.as_str()) {
        value
    } else {
        "auto".to_string()
    }
}

impl Settings {
    /// 默认设置：每种类型显示全部可用列；默认不显示隐藏文件。
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
        Self {
            column_settings,
            show_hidden: false,
            terminal_tool: "auto".to_string(),
        }
    }
}

/// 校验并补全读到的设置：只保留已知类型与已知列，缺失的类型用默认列补齐。
fn sanitize(saved: Settings) -> Settings {
    let mut settings = Settings::defaults();
    settings.show_hidden = saved.show_hidden;
    settings.terminal_tool = normalize_terminal_tool(&saved.terminal_tool);
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
        assert!(!out.show_hidden);
    }

    #[test]
    fn show_hidden_roundtrips_and_is_preserved() {
        let mut saved = Settings::defaults();
        saved.show_hidden = true;
        let out = sanitize(saved);
        assert!(out.show_hidden);
        let json = serde_json::to_string(&out).unwrap();
        assert!(json.contains("\"show_hidden\":true"));
        // 旧配置没有该字段时默认 false
        let old: Settings = serde_json::from_str(r#"{"column_settings":{}}"#).unwrap();
        assert!(!old.show_hidden);
    }

    #[test]
    fn terminal_tool_defaults_and_sanitizes() {
        assert_eq!(Settings::defaults().terminal_tool, "auto");
        let mut saved = Settings::defaults();
        saved.terminal_tool = "WT".to_string();
        assert_eq!(sanitize(saved).terminal_tool, "wt");
        let mut saved = Settings::defaults();
        saved.terminal_tool = "nonsense".to_string();
        assert_eq!(sanitize(saved).terminal_tool, "auto");
        // 旧配置没有该字段：反序列化为空串，加载时归一为 auto
        let old: Settings = serde_json::from_str(r#"{"column_settings":{}}"#).unwrap();
        assert_eq!(sanitize(old).terminal_tool, "auto");
    }

    #[test]
    fn serializer_uses_snake_case_key() {
        let s = Settings::defaults();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"column_settings\""), "JSON 键名应保持兼容");
    }
}
