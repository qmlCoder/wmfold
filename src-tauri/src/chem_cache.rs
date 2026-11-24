//! 化学元数据的本地磁盘缓存。
//!
//! 缓存键是「规范化路径」，有效性由文件大小 + 修改时间 + 解析格式版本决定。
//! 文件没有变化且 `CHEM_CACHE_VERSION` 一致时直接复用，不再调用 rswfn。
//!
//! 存储位置：`%APPDATA%\wmfold\chem_cache.json`（非 Windows 用 `$HOME`）。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use wmfold_core::chem::{self, CHEM_CACHE_VERSION};
use wmfold_core::models::ChemMeta;

const FILE_NAME: &str = "chem_cache.json";
/// 批量解析时不必每批都写整个 JSON；攒够这么多条或超过 5 秒再落盘。
const SAVE_EVERY_ENTRIES: usize = 128;
const SAVE_EVERY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// 文件字节数
    size: u64,
    /// 修改时间（Unix 毫秒）
    modified: i64,
    chem: ChemMeta,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

impl CacheData {
    fn empty() -> Self {
        Self {
            version: CHEM_CACHE_VERSION,
            entries: HashMap::new(),
        }
    }
}

struct Cache {
    path: PathBuf,
    data: CacheData,
    dirty: bool,
    dirty_count: usize,
    last_save: Instant,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn cache_file() -> PathBuf {
    crate::settings::config_dir().join(FILE_NAME)
}

/// 路径 key：Windows 忽略大小写和分隔符差异；末尾分隔符不影响。
fn cache_key(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if cfg!(windows) {
        trimmed.replace('/', "\\").to_lowercase()
    } else {
        trimmed.to_string()
    }
}

fn fingerprint(path: &str) -> Option<(u64, i64)> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as i64;
    Some((meta.len(), modified))
}

fn load_from(path: &Path) -> CacheData {
    let Ok(text) = fs::read_to_string(path) else {
        return CacheData::empty();
    };
    let Ok(mut data) = serde_json::from_str::<CacheData>(&text) else {
        return CacheData::empty();
    };
    if data.version != CHEM_CACHE_VERSION {
        return CacheData::empty();
    }
    data
}

/// 写出缓存。先写临时文件再替换，避免程序中断留下半个 JSON。
fn save_to(path: &Path, data: &CacheData) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string(data).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    if fs::rename(&tmp, path).is_ok() {
        return Ok(());
    }
    // Windows 某些情况下 rename 不能覆盖已存在文件，重试一次。
    let _ = fs::remove_file(path);
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| {
        let path = cache_file();
        Mutex::new(Cache {
            data: load_from(&path),
            path,
            dirty: false,
            dirty_count: 0,
            last_save: Instant::now(),
        })
    })
}

/// 读取一批化学元数据：命中缓存直接返回，未命中才真正解析，并在解析后写盘。
pub fn read_chem_batch_cached(paths: &[String]) -> Vec<ChemMeta> {
    if paths.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<Option<ChemMeta>> = vec![None; paths.len()];
    let mut missing: Vec<(usize, String, Option<(u64, i64)>)> = Vec::new();

    {
        let guard = cache().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for (index, path) in paths.iter().enumerate() {
            let fp = fingerprint(path);
            let hit = fp.and_then(|(size, modified)| {
                guard
                    .data
                    .entries
                    .get(&cache_key(path))
                    .filter(|entry| entry.size == size && entry.modified == modified)
                    .map(|entry| entry.chem.clone())
            });
            if let Some(chem) = hit {
                out[index] = Some(chem);
            } else {
                missing.push((index, path.clone(), fp));
            }
        }
    }

    if missing.is_empty() {
        return out
            .into_iter()
            .enumerate()
            .map(|(index, chem)| chem.unwrap_or_else(|| chem::read_chem(&paths[index])))
            .collect();
    }

    let parse_paths: Vec<String> = missing.iter().map(|(_, path, _)| path.clone()).collect();
    let parsed = chem::read_chem_batch(&parse_paths);

    {
        let mut guard = cache().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for ((index, path, old_fp), chem) in missing.iter().zip(parsed.into_iter()) {
            out[*index] = Some(chem.clone());
            // 解析后重新 stat，避免解析期间文件被更新却缓存成旧结果
            if let Some((size, modified)) = fingerprint(path).or(*old_fp) {
                guard.data.entries.insert(
                    cache_key(path),
                    CacheEntry {
                        size,
                        modified,
                        chem,
                    },
                );
                guard.dirty = true;
                guard.dirty_count += 1;
            }
        }
        let should_save = guard.dirty
            && (guard.dirty_count >= SAVE_EVERY_ENTRIES || guard.last_save.elapsed() >= SAVE_EVERY);
        if should_save && save_to(&guard.path, &guard.data).is_ok() {
            guard.dirty = false;
            guard.dirty_count = 0;
            guard.last_save = Instant::now();
        }
    }

    out.into_iter()
        .enumerate()
        .map(|(index, chem)| chem.unwrap_or_else(|| chem::read_chem(&paths[index])))
        .collect()
}

/// 立即把内存中的脏条目写入磁盘；批量后台解析结束时调用一次。
pub fn flush() {
    let mut guard = cache().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !guard.dirty {
        return;
    }
    if save_to(&guard.path, &guard.data).is_ok() {
        guard.dirty = false;
        guard.dirty_count = 0;
        guard.last_save = Instant::now();
    }
}

/// 单个文件的缓存版本；主要供预览面板使用。
pub fn read_chem_cached(path: &str) -> ChemMeta {
    let chem = read_chem_batch_cached(&[path.to_string()])
        .into_iter()
        .next()
        .unwrap_or_else(|| chem::read_chem(path));
    flush();
    chem
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wmfold_chem_cache_{}_{}_{}",
            std::process::id(),
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cache_roundtrip_and_fingerprint_invalidation() {
        let dir = temp_dir("roundtrip");
        let file = dir.join("water.xyz");
        fs::write(&file, "3\nwater\nO 0.0 0.0 0.0\nH 0.0 0.0 1.0\nH 1.0 0.0 0.0\n")
            .unwrap();
        let path = file.to_string_lossy().to_string();

        let cache_path = dir.join("cache.json");
        let mut data = CacheData::empty();
        data.entries.insert(
            cache_key(&path),
            CacheEntry {
                size: fs::metadata(&file).unwrap().len(),
                modified: fingerprint(&path).unwrap().1,
                chem: chem::read_chem(&path),
            },
        );
        save_to(&cache_path, &data).unwrap();
        let loaded = load_from(&cache_path);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(
            loaded.entries.get(&cache_key(&path)).unwrap().chem.formula,
            "H2O"
        );

        // 文件变化后指纹不同
        let old = fingerprint(&path).unwrap();
        fs::write(&file, "3\nwater\nO 0.0 0.0 0.0\nH 0.0 0.0 1.0\nH 1.0 0.0 0.1\n").unwrap();
        let new = fingerprint(&path).unwrap();
        assert_ne!(old, new);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn version_mismatch_discards_entries() {
        let dir = temp_dir("version");
        let file = dir.join("cache.json");
        let data = CacheData {
            version: CHEM_CACHE_VERSION + 1,
            entries: HashMap::new(),
        };
        save_to(&file, &data).unwrap();
        assert!(load_from(&file).entries.is_empty());
        fs::remove_dir_all(&dir).ok();
    }
}
