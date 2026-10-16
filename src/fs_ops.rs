//! 文件系统命令：目录列表、磁盘、快速访问以及常规文件操作。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::chem::kind_from_ext;
use crate::models::{DriveInfo, FileEntry, QuickAccess};

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
    let is_symlink = sym_meta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);
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

/// 列出目录内容。
pub fn list_dir(path: &str) -> Result<Vec<FileEntry>, String> {
    let mut out = Vec::new();
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in rd.flatten() {
        out.push(make_entry(&entry.path()));
    }
    Ok(out)
}

/// 当前用户主目录。
pub fn get_home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "C:\\Users".to_string())
}

/// 快速访问项（此电脑、桌面、文档、下载等）。
pub fn get_quick_access() -> Vec<QuickAccess> {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let mut items = Vec::new();

    // 常见用户目录，同时兼容英文名与 Windows 中文名。
    let cands: &[(&str, &str, &str)] = &[
        ("桌面", "Desktop", "folder"),
        ("文档", "Documents", "folder"),
        ("下载", "Downloads", "folder"),
        ("图片", "Pictures", "folder"),
        ("音乐", "Music", "folder"),
        ("视频", "Videos", "folder"),
    ];
    for (cn, en, icon) in cands {
        let mut found: Option<String> = None;
        for base in [home.clone(), Path::new(&home).join("OneDrive").to_string_lossy().to_string()] {
            for name in [*en, *cn] {
                let p = Path::new(&base).join(name);
                if p.exists() {
                    found = Some(p.to_string_lossy().to_string());
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some(p) = found {
            items.push(QuickAccess {
                name: (*cn).to_string(),
                path: p,
                icon: icon.to_string(),
            });
        }
    }

    items.push(QuickAccess {
        name: "此电脑".to_string(),
        path: "::ThisPC".to_string(),
        icon: "computer".to_string(),
    });

    items
}

/// 列出本地磁盘驱动器。
pub fn get_drives() -> Vec<DriveInfo> {
    let mut drives = Vec::new();
    for c in b'A'..=b'Z' {
        let letter = (c as char).to_string();
        let root = format!("{}:\\", letter);
        if Path::new(&root).exists() {
            let label = format!("本地磁盘 ({})", letter);
            drives.push(DriveInfo {
                letter,
                label,
                total: None,
                free: None,
            });
        }
    }
    drives
}

/// 新建文件夹。
pub fn create_folder(path: &str) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())
}

/// 新建空文件。
pub fn create_file(path: &str) -> Result<(), String> {
    std::fs::write(path, "").map_err(|e| e.to_string())
}

/// 重命名。
pub fn rename_path(path: &str, new_name: &str) -> Result<(), String> {
    let p = Path::new(path);
    let parent = p.parent().ok_or_else(|| "无法获取父目录".to_string())?;
    let new_path = parent.join(new_name);
    std::fs::rename(p, &new_path).map_err(|e| e.to_string())
}

/// 删除（可回收站或永久）。
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

/// 移动到目标目录。
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

/// 用系统默认程序打开路径。
pub fn open_path(path: &str) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
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
        std::fs::write(dir.join("water.xyz"), "3\nw\nO 0.0 0.0 0.0\nH 0.757 0.586 0.0\nH -0.757 0.586 0.0\n").unwrap();
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
}
