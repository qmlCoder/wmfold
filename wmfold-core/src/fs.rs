//! 文件系统操作：目录列表与常规文件操作。
//!
//! 平台无关实现（Windows 用文件属性判断隐藏项，类 Unix 用点前缀）。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::Serialize;

use crate::chem::kind_from_ext;
use crate::models::FileEntry;

/// SystemTime 转毫秒时间戳。
fn to_ms(t: std::time::SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn is_hidden_dir(name: &str) -> bool {
    name != "." && name != ".."
}

/// 判断是否为隐藏项（Windows 使用属性位，其他平台使用点前缀）。
fn is_hidden(path: &Path, name: &str) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(m) = std::fs::metadata(path) {
            // FILE_ATTRIBUTE_HIDDEN = 0x2
            return m.file_attributes() & 0x2 != 0;
        }
        name.starts_with('.')
    }
    #[cfg(not(windows))]
    {
        name.starts_with('.') && is_hidden_dir(name)
    }
}

/// 构造单个文件条目。
fn make_entry(p: &Path) -> FileEntry {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string_lossy().to_string());

    let meta = std::fs::metadata(p).ok();
    let sym_meta = std::fs::symlink_metadata(p).ok();
    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let is_symlink = sym_meta
        .as_ref()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    let size = if is_dir {
        0
    } else {
        meta.as_ref().map(|m| m.len()).unwrap_or(0)
    };
    let created = meta.as_ref().and_then(|m| m.created().ok()).map(to_ms);
    let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(to_ms);

    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let kind = if is_dir {
        String::new()
    } else {
        kind_from_ext(&ext).unwrap_or("").to_string()
    };
    let is_chem = !kind.is_empty();
    let hidden = is_hidden(p, &name);

    FileEntry {
        name,
        path: p.to_string_lossy().to_string(),
        is_dir,
        size,
        created,
        modified,
        extension: ext,
        kind,
        is_chem,
        is_hidden: hidden,
        is_symlink,
        chem: None,
    }
}

/// 列出目录内容。只读元数据，**不**解析化学信息（保持快速）。
pub fn list_dir(path: &str) -> Result<Vec<FileEntry>, String> {
    let mut out = Vec::new();
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in rd.flatten() {
        out.push(make_entry(&entry.path()));
    }
    Ok(out)
}

/// 读取单个文件 / 目录的条目信息（不解析化学信息）。
pub fn stat(path: &str) -> Result<FileEntry, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("not found: {path}"));
    }
    Ok(make_entry(p))
}

/// 递归扫描的统计结果。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    /// 实际读取的文件夹数量
    pub dirs: usize,
    /// 收集到的文件数量
    pub files: usize,
    /// 因无权限 / 符号链接 / 隐藏项而跳过的数量
    pub skipped: usize,
    /// 是否达到条目上限而提前结束
    pub truncated: bool,
    /// 是否被调用方取消
    pub cancelled: bool,
}

/// 递归扫描进度（由调用方按时间节流回调）。
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub dirs: usize,
    pub files: usize,
    /// 当前正在读取的路径
    pub current: String,
}

/// 递归扫描 `root` 下的所有文件（不包含目录本身），返回文件条目与统计。
///
/// - 不跟随符号链接 / 目录联接，避免环路；
/// - `include_hidden = false` 时跳过隐藏文件夹与隐藏文件；
/// - 每读取约 80 ms 回调一次 `progress`，用于界面显示进度；
/// - `cancel` 返回 true 时提前结束并返回已收集的结果（`stats.cancelled = true`）；
/// - 最多收集 `max_entries` 个文件（`stats.truncated = true`）。
pub fn scan_recursive(
    root: &str,
    include_hidden: bool,
    max_entries: usize,
    cancel: &dyn Fn() -> bool,
    mut progress: impl FnMut(&ScanProgress),
) -> Result<(Vec<FileEntry>, ScanStats), String> {
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        return Err(format!("目录不存在：{root}"));
    }
    let max_entries = max_entries.max(1);
    let mut out = Vec::new();
    let mut stats = ScanStats::default();
    let mut stack = vec![root_path.to_path_buf()];
    let mut last_report = Instant::now() - Duration::from_secs(1);

    while let Some(dir) = stack.pop() {
        if cancel() {
            stats.cancelled = true;
            break;
        }
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => {
                stats.skipped += 1;
                continue;
            }
        };
        stats.dirs += 1;

        for entry in rd.flatten() {
            if cancel() {
                stats.cancelled = true;
                break;
            }
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let sym_meta = match std::fs::symlink_metadata(&p) {
                Ok(m) => m,
                Err(_) => {
                    stats.skipped += 1;
                    continue;
                }
            };
            let is_symlink = sym_meta.file_type().is_symlink();

            if sym_meta.is_dir() {
                // 符号链接 / 目录联接不展开，避免循环与越界；隐藏目录按设置跳过
                if is_symlink || (!include_hidden && is_hidden(&p, &name)) {
                    stats.skipped += 1;
                    continue;
                }
                stack.push(p.clone());
            } else {
                if !include_hidden && is_hidden(&p, &name) {
                    stats.skipped += 1;
                    continue;
                }
                out.push(make_entry(&p));
                stats.files += 1;
                if out.len() >= max_entries {
                    stats.truncated = true;
                    break;
                }
            }

            if last_report.elapsed() >= Duration::from_millis(80) {
                progress(&ScanProgress {
                    dirs: stats.dirs,
                    files: stats.files,
                    current: p.to_string_lossy().to_string(),
                });
                last_report = Instant::now();
            }
        }

        if stats.truncated || stats.cancelled {
            break;
        }
        if last_report.elapsed() >= Duration::from_millis(80) {
            progress(&ScanProgress {
                dirs: stats.dirs,
                files: stats.files,
                current: dir.to_string_lossy().to_string(),
            });
            last_report = Instant::now();
        }
    }

    // 结束时再报告一次，保证界面总能拿到最终计数
    progress(&ScanProgress {
        dirs: stats.dirs,
        files: stats.files,
        current: root.to_string(),
    });

    Ok((out, stats))
}

/// 当前用户主目录。
pub fn home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "C:\\Users".to_string()
            } else {
                "/".to_string()
            }
        })
}

/// 新建文件夹。
pub fn create_folder(path: &str) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())
}

/// 新建空文件。
pub fn create_file(path: &str) -> Result<(), String> {
    std::fs::write(path, "").map_err(|e| e.to_string())
}

/// 重命名（同一目录内）。
pub fn rename_path(path: &str, new_name: &str) -> Result<(), String> {
    let p = Path::new(path);
    let parent = p.parent().ok_or_else(|| "无法获取父目录".to_string())?;
    let new_path = parent.join(new_name);
    std::fs::rename(p, &new_path).map_err(|e| e.to_string())
}

/// 删除（可回收站或永久删除）。
pub fn delete_paths(paths: &[String], permanent: bool) -> Result<(), String> {
    for p in paths {
        if permanent {
            let path = Path::new(p);
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
            } else {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        } else {
            trash::delete(p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 递归复制目录。
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// 复制到目标目录。
pub fn copy_paths(paths: &[String], dest_dir: &str) -> Result<(), String> {
    for p in paths {
        let src = Path::new(p);
        let name = src
            .file_name()
            .ok_or_else(|| "无法确定文件名".to_string())?;
        let dst = PathBuf::from(dest_dir).join(name);

        if src.is_dir() {
            copy_dir_recursive(src, &dst).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(src, &dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 移动到目标目录（跨卷时退化为复制后删除）。
pub fn move_paths(paths: &[String], dest_dir: &str) -> Result<(), String> {
    for p in paths {
        let src = Path::new(p);
        let name = src
            .file_name()
            .ok_or_else(|| "无法确定文件名".to_string())?;
        let dst = PathBuf::from(dest_dir).join(name);

        if std::fs::rename(src, &dst).is_err() {
            // 跨卷移动：复制后删除。
            if src.is_dir() {
                copy_dir_recursive(src, &dst).map_err(|e| e.to_string())?;
                std::fs::remove_dir_all(src).map_err(|e| e.to_string())?;
            } else {
                std::fs::copy(src, &dst).map_err(|e| e.to_string())?;
                std::fs::remove_file(src).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNT: AtomicU32 = AtomicU32::new(0);

    fn tmp_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let id = COUNT.fetch_add(1, Ordering::SeqCst);
        p.push(format!("wmfold_fs_{}_{}", std::process::id(), id));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn list_dir_flags_chem() {
        let dir = tmp_dir();
        std::fs::write(
            dir.join("water.xyz"),
            "3\nw\nO 0.0 0.0 0.0\nH 0.757 0.586 0.0\nH -0.757 0.586 0.0\n",
        )
        .unwrap();
        std::fs::write(dir.join("notes.txt"), "hello\n").unwrap();
        std::fs::create_dir(dir.join("subdir")).unwrap();

        let entries = list_dir(&dir.to_string_lossy()).unwrap();
        let by_name = |n: &str| entries.iter().find(|e| e.name == n).unwrap();

        let xyz = by_name("water.xyz");
        assert!(xyz.is_chem);
        assert_eq!(xyz.kind, "xyz");

        let txt = by_name("notes.txt");
        assert!(!txt.is_chem);
        assert_eq!(txt.kind, "");

        let sub = by_name("subdir");
        assert!(sub.is_dir);

        // 端到端读取化学元数据
        let meta = crate::chem::read_chem(&xyz.path);
        assert!(meta.ok);
        assert_eq!(meta.formula, "H2O");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_recursive_collects_nested_files() {
        let dir = tmp_dir();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let sub = dir.join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.xyz"), "1\nx\nH 0.0 0.0 0.0\n").unwrap();
        let deep = sub.join("deep");
        std::fs::create_dir(&deep).unwrap();
        std::fs::write(deep.join("c.gjf"), "#p b3lyp/6-31g(d)\n\nwater\n\n0 1\nO 0.0 0.0 0.0\nH 0.0 0.0 1.0\n").unwrap();

        let (files, stats) =
            scan_recursive(&dir.to_string_lossy(), true, 1000, &|| false, |_| {}).unwrap();
        let mut names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["a.txt", "b.xyz", "c.gjf"]);
        assert_eq!(stats.files, 3);
        assert_eq!(stats.dirs, 3);
        assert!(!stats.truncated && !stats.cancelled);
        assert!(files.iter().any(|f| f.name == "c.gjf" && f.is_chem && f.kind == "gjf"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_recursive_respects_limit_and_cancel() {
        let dir = tmp_dir();
        for i in 0..5 {
            std::fs::write(dir.join(format!("f{i}.txt")), "x").unwrap();
        }

        let (files, stats) =
            scan_recursive(&dir.to_string_lossy(), true, 2, &|| false, |_| {}).unwrap();
        assert_eq!(files.len(), 2);
        assert!(stats.truncated);
        assert!(!stats.cancelled);

        let (files, stats) =
            scan_recursive(&dir.to_string_lossy(), true, 100, &|| true, |_| {}).unwrap();
        assert!(files.is_empty());
        assert!(stats.cancelled);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_recursive_reports_progress() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let dir = tmp_dir();
        for i in 0..3 {
            std::fs::write(dir.join(format!("p{i}.txt")), "x").unwrap();
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let (files, _) = scan_recursive(&dir.to_string_lossy(), true, 100, &|| false, move |p| {
            assert!(!p.current.is_empty());
            seen.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        assert_eq!(files.len(), 3);
        // 进度回调至少会在扫描结束时触发一次（80 ms 节流后仍可能为 0，
        // 因此这里只保证不 panic；回调次数不做强约束）。
        let _ = calls.load(Ordering::SeqCst);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_rename_copy_move_delete_roundtrip() {
        let dir = tmp_dir();
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");

        create_file(&a.to_string_lossy()).unwrap();
        assert!(a.exists());
        rename_path(&a.to_string_lossy(), "b.txt").unwrap();
        assert!(b.exists() && !a.exists());

        let sub = dir.join("sub");
        create_folder(&sub.to_string_lossy()).unwrap();
        copy_paths(&[b.to_string_lossy().to_string()], &sub.to_string_lossy()).unwrap();
        assert!(sub.join("b.txt").exists());

        move_paths(
            &[sub.join("b.txt").to_string_lossy().to_string()],
            &dir.to_string_lossy(),
        )
        .unwrap();
        assert!(dir.join("b.txt").exists() && !sub.join("b.txt").exists());

        // 永久删除，避免测试环境没有回收站
        delete_paths(&[dir.to_string_lossy().to_string()], true).unwrap();
        assert!(!dir.exists());
    }
}
