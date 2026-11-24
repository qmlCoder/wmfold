//! 用户自定义「快速访问」（固定目录）的持久化。
//!
//! 存储位置：`%APPDATA%\wmfold\quick_access.json`（非 Windows 用 `$HOME`），
//! 内容为目录路径的 JSON 数组，例如 `["E:\\code\\wmfold", "D:\\data"]`。
//!
//! 与 `settings.json` 分开存放：前端每次修改化学列都会整体保存 settings，
//! 如果把固定项放进 settings.json，会被前端保存的旧结构覆盖。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use wmfold_core::models::QuickAccess;

use crate::settings::config_dir;

/// 固定项文件名（存放于配置目录）。
const FILE_NAME: &str = "quick_access.json";

/// 固定项存储文件路径。
pub fn pins_path() -> PathBuf {
    config_dir().join(FILE_NAME)
}

/// 路径比较用的归一化 key。
///
/// 去掉末尾的分隔符（`C:\` 与 `C:` 视为同一项），Windows 下再忽略大小写。
pub fn path_key(path: &str) -> String {
    let trimmed = path.trim_end_matches(&['\\', '/'][..]);
    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    }
}

/// 从指定文件读取固定路径；文件不存在 / 内容损坏时返回空列表。
pub fn load_pins_from(file: &Path) -> Vec<String> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let raw: Vec<String> = serde_json::from_str(&text).unwrap_or_default();
    let mut seen = HashSet::new();
    raw.into_iter()
        .filter(|p| !p.trim().is_empty())
        .filter(|p| seen.insert(path_key(p)))
        .collect()
}

/// 把固定路径写入指定文件（自动创建配置目录）。
pub fn save_pins_to(file: &Path, pins: &[String]) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(pins).map_err(|e| e.to_string())?;
    std::fs::write(file, text).map_err(|e| e.to_string())
}

/// 读取当前用户的固定路径。
pub fn load_pins() -> Vec<String> {
    load_pins_from(&pins_path())
}

/// 把固定项写入指定文件；已存在（忽略末尾分隔符与大小写）时返回 `Ok(false)`。
fn add_pin_to(file: &Path, path: &str) -> Result<bool, String> {
    if !Path::new(path).is_dir() {
        return Err("只能把文件夹添加到快速访问".to_string());
    }
    let mut pins = load_pins_from(file);
    let key = path_key(path);
    if pins.iter().any(|p| path_key(p) == key) {
        return Ok(false);
    }
    pins.push(path.to_string());
    save_pins_to(file, &pins)?;
    Ok(true)
}

/// 从指定文件移除固定项（不存在时静默成功）。
fn remove_pin_from(file: &Path, path: &str) -> Result<(), String> {
    let mut pins = load_pins_from(file);
    let key = path_key(path);
    let before = pins.len();
    pins.retain(|p| path_key(p) != key);
    if pins.len() != before {
        save_pins_to(file, &pins)?;
    }
    Ok(())
}

/// 添加固定项；已固定时返回 `Ok(false)`（幂等，便于前端提示“已在快速访问中”）。
pub fn add_pin(path: &str) -> Result<bool, String> {
    add_pin_to(&pins_path(), path)
}

/// 取消固定项。
pub fn remove_pin(path: &str) -> Result<(), String> {
    remove_pin_from(&pins_path(), path)
}

/// 固定路径在侧边栏显示的名称。
///
/// 取最后一级目录名；盘符根（`C:\`）显示为「本地磁盘 (C:)」，与「此电脑」中的命名一致。
pub fn display_name(path: &str) -> String {
    let trimmed = path.trim_end_matches(&['\\', '/'][..]);
    if let Some(name) = Path::new(trimmed).file_name().and_then(|n| n.to_str()) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    // 盘符根："C:\" 去掉末尾分隔符后为 "C:"
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("本地磁盘 ({trimmed})")
    } else if trimmed.is_empty() {
        path.to_string()
    } else {
        trimmed.to_string()
    }
}

/// 把固定路径转换为侧边栏条目（`icon` 固定为 folder，`pinned` 为 true）。
pub fn pinned_items(pins: &[String]) -> Vec<QuickAccess> {
    pins.iter()
        .map(|p| QuickAccess {
            name: display_name(p),
            path: p.clone(),
            icon: "folder".to_string(),
            pinned: true,
        })
        .collect()
}

// ---- 远程机器的固定项 ----
//
// 远程路径是本机无法校验的 POSIX 路径，因此单独存一份
// `remote_quick_access.json`：`{ "<profile id>": ["/home/user/calc", ...] }`。

/// 远程固定项文件名（存放于配置目录）。
const REMOTE_FILE_NAME: &str = "remote_quick_access.json";

/// 远程固定项存储文件路径。
pub fn remote_pins_path() -> PathBuf {
    config_dir().join(REMOTE_FILE_NAME)
}

/// 从指定文件读取「机器 → 路径列表」映射；文件不存在 / 损坏时返回空。
pub fn load_remote_pins_from(file: &Path) -> HashMap<String, Vec<String>> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(_) => return HashMap::new(),
    };
    let raw: HashMap<String, Vec<String>> = serde_json::from_str(&text).unwrap_or_default();
    let mut out = HashMap::new();
    for (machine, paths) in raw {
        let mut seen = HashSet::new();
        let list: Vec<String> = paths
            .into_iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .filter(|p| seen.insert(path_key(p)))
            .collect();
        if !list.is_empty() {
            out.insert(machine, list);
        }
    }
    out
}

/// 写入「机器 → 路径列表」映射。
pub fn save_remote_pins_to(
    file: &Path,
    pins: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(pins).map_err(|e| e.to_string())?;
    std::fs::write(file, text).map_err(|e| e.to_string())
}

/// 某台机器当前的固定路径。
pub fn load_remote_pins(machine: &str) -> Vec<String> {
    load_remote_pins_from(&remote_pins_path())
        .remove(machine)
        .unwrap_or_default()
}

fn add_remote_pin_to(file: &Path, machine: &str, path: &str) -> Result<bool, String> {
    let path = path.trim();
    if machine.trim().is_empty() {
        return Err("远程机器标识为空".to_string());
    }
    if path.is_empty() || !path.starts_with('/') {
        return Err("只能添加远程绝对路径".to_string());
    }
    let mut map = load_remote_pins_from(file);
    let list = map.entry(machine.to_string()).or_default();
    let key = path_key(path);
    if list.iter().any(|p| path_key(p) == key) {
        return Ok(false);
    }
    list.push(path.to_string());
    save_remote_pins_to(file, &map)?;
    Ok(true)
}

fn remove_remote_pin_from(file: &Path, machine: &str, path: &str) -> Result<(), String> {
    let mut map = load_remote_pins_from(file);
    let key = path_key(path);
    let (changed, empty) = if let Some(list) = map.get_mut(machine) {
        let before = list.len();
        list.retain(|p| path_key(p) != key);
        (list.len() != before, list.is_empty())
    } else {
        (false, false)
    };
    if empty {
        map.remove(machine);
    }
    if changed {
        save_remote_pins_to(file, &map)?;
    }
    Ok(())
}

/// 添加远程固定项；已存在时返回 `Ok(false)`。
pub fn add_remote_pin(machine: &str, path: &str) -> Result<bool, String> {
    add_remote_pin_to(&remote_pins_path(), machine, path)
}

/// 取消远程固定项（不存在时静默成功）。
pub fn remove_remote_pin(machine: &str, path: &str) -> Result<(), String> {
    remove_remote_pin_from(&remote_pins_path(), machine, path)
}

/// 删除某台机器的全部固定项（删除远程机器配置时调用）。
pub fn remove_remote_machine(machine: &str) -> Result<(), String> {
    let mut map = load_remote_pins_from(&remote_pins_path());
    if map.remove(machine).is_some() {
        save_remote_pins_to(&remote_pins_path(), &map)?;
    }
    Ok(())
}

/// 远程路径在侧边栏显示的名称（POSIX basename；`/` 显示为「根目录」）。
pub fn remote_display_name(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "根目录".to_string();
    }
    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

/// 把远程固定路径转换为侧边栏条目。
pub fn remote_pinned_items(paths: &[String]) -> Vec<QuickAccess> {
    paths
        .iter()
        .map(|p| QuickAccess {
            name: remote_display_name(p),
            path: p.clone(),
            icon: "folder".to_string(),
            pinned: true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNT: AtomicU32 = AtomicU32::new(0);

    /// 每个测试独立的临时文件路径（不创建文件本身）。
    fn tmp_file(tag: &str) -> PathBuf {
        let id = COUNT.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("wmfold_qa_{}_{}_{}.json", std::process::id(), tag, id));
        let _ = std::fs::remove_file(&p);
        p
    }

    /// 每个测试独立的临时目录。
    fn tmp_dir(tag: &str) -> PathBuf {
        let id = COUNT.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("wmfold_qa_dir_{}_{}_{}", std::process::id(), tag, id));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn missing_or_broken_file_is_empty() {
        let file = tmp_file("missing");
        assert!(load_pins_from(&file).is_empty());

        std::fs::write(&file, "not json").unwrap();
        assert!(load_pins_from(&file).is_empty());

        std::fs::write(&file, "\"just a string\"").unwrap();
        assert!(load_pins_from(&file).is_empty());

        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn load_drops_empty_and_duplicate_entries() {
        let file = tmp_file("dedupe");
        std::fs::write(&file, r#"["E:\\a", "", "   ", "E:\\a\\"]"#).unwrap();
        let pins = load_pins_from(&file);
        assert_eq!(pins, vec!["E:\\a".to_string()]);
        std::fs::remove_file(&file).ok();
    }

    #[cfg(windows)]
    #[test]
    fn load_dedupes_case_insensitively_on_windows() {
        let file = tmp_file("case");
        std::fs::write(&file, r#"["E:\\Code", "e:\\code"]"#).unwrap();
        assert_eq!(load_pins_from(&file).len(), 1);
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn add_pin_is_idempotent_and_can_be_removed() {
        let file = tmp_file("add");
        let a = tmp_dir("a");
        let b = tmp_dir("b");
        let a_str = a.to_string_lossy().to_string();
        let b_str = b.to_string_lossy().to_string();

        assert!(add_pin_to(&file, &a_str).unwrap());
        // 同一目录带尾部分隔符时视为重复
        assert!(!add_pin_to(&file, &format!("{a_str}\\")).unwrap());
        assert!(add_pin_to(&file, &b_str).unwrap());

        let pins = load_pins_from(&file);
        assert_eq!(pins.len(), 2);

        remove_pin_from(&file, &a_str).unwrap();
        let pins = load_pins_from(&file);
        assert_eq!(pins, vec![b_str.clone()]);

        // 重复移除 / 移除不存在的项都静默成功
        remove_pin_from(&file, &a_str).unwrap();
        remove_pin_from(&file, "C:\\不存在的目录").unwrap();
        assert_eq!(load_pins_from(&file), vec![b_str]);

        std::fs::remove_file(&file).ok();
        std::fs::remove_dir_all(&a).ok();
        std::fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn add_pin_rejects_files_and_missing_paths() {
        let file = tmp_file("reject");
        let dir = tmp_dir("reject_dir");
        let file_path = dir.join("plain.txt");
        std::fs::write(&file_path, "hi").unwrap();

        assert!(add_pin_to(&file, &file_path.to_string_lossy()).is_err());
        assert!(add_pin_to(&file, "C:\\这不存在的目录").is_err());
        assert!(load_pins_from(&file).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn display_name_uses_last_segment() {
        assert_eq!(display_name("E:\\code\\wmfold"), "wmfold");
        assert_eq!(display_name("E:\\code\\wmfold\\"), "wmfold");
        assert_eq!(display_name("E:/code/wmfold"), "wmfold");
    }

    #[cfg(windows)]
    #[test]
    fn display_name_handles_drive_root() {
        assert_eq!(display_name("D:\\"), "本地磁盘 (D:)");
        assert_eq!(display_name("D:"), "本地磁盘 (D:)");
    }

    #[test]
    fn pinned_items_are_marked_pinned() {
        let pins = vec!["E:\\code\\wmfold".to_string()];
        let items = pinned_items(&pins);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "wmfold");
        assert_eq!(items[0].path, "E:\\code\\wmfold");
        assert_eq!(items[0].icon, "folder");
        assert!(items[0].pinned);
    }

    #[test]
    fn remote_pins_roundtrip_and_dedupe() {
        let file = tmp_file("remote");
        std::fs::write(
            &file,
            r#"{"m1":["/home/u", "/home/u/", "", "  "], "m2":[]}"#,
        )
        .unwrap();
        let loaded = load_remote_pins_from(&file);
        assert_eq!(loaded["m1"], vec!["/home/u".to_string()]);
        assert!(!loaded.contains_key("m2"));

        save_remote_pins_to(&file, &loaded).unwrap();
        assert_eq!(load_remote_pins_from(&file)["m1"], vec!["/home/u".to_string()]);
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn remote_add_and_remove_pin() {
        let file = tmp_file("remote_add");
        assert!(add_remote_pin_to(&file, "m1", "/home/u/calc").unwrap());
        assert!(!add_remote_pin_to(&file, "m1", "/home/u/calc/").unwrap());
        assert_eq!(
            load_remote_pins_from(&file)["m1"],
            vec!["/home/u/calc".to_string()]
        );
        remove_remote_pin_from(&file, "m1", "/home/u/calc").unwrap();
        assert!(load_remote_pins_from(&file).is_empty());
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn remote_add_rejects_relative_path() {
        let file = tmp_file("remote_rel");
        assert!(add_remote_pin_to(&file, "m1", "home/u").is_err());
        assert!(add_remote_pin_to(&file, "m1", "/home/u").is_ok());
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn remote_display_name_uses_posix_basename() {
        assert_eq!(remote_display_name("/home/user/calc"), "calc");
        assert_eq!(remote_display_name("/home/user/calc/"), "calc");
        assert_eq!(remote_display_name("/"), "根目录");
        let items = remote_pinned_items(&["/home/u/calc".to_string()]);
        assert_eq!(items[0].name, "calc");
        assert!(items[0].pinned);
    }
}
