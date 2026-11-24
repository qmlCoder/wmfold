//! 递归读取：扫描当前文件夹下的所有子文件夹，并按进度向界面推送事件。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use wmfold_core::fs::{self, ScanProgress};
use wmfold_core::models::FileEntry;

/// 同时只允许一个扫描任务；记录当前任务 id。
static ACTIVE_SCAN: AtomicU64 = AtomicU64::new(0);
/// 当前任务是否请求取消。
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

/// 单次递归扫描的默认文件上限，避免误扫整个磁盘时占用过多内存。
pub const DEFAULT_MAX_ENTRIES: usize = 100_000;

/// 进度事件（`scan-progress`）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressEvent {
    pub scan_id: u64,
    pub dirs: usize,
    pub files: usize,
    /// 当前正在读取的路径
    pub current: String,
    pub elapsed_ms: u64,
    /// 扫描结束（正常结束 / 取消 / 达到上限）
    pub done: bool,
    pub cancelled: bool,
    pub truncated: bool,
}

/// 递归扫描结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub entries: Vec<FileEntry>,
    pub dirs: usize,
    pub files: usize,
    pub skipped: usize,
    pub truncated: bool,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

/// 递归读取 `path` 下所有子文件夹中的文件。
///
/// - 扫描在专用阻塞线程中执行（`spawn_blocking`），不会卡住界面；
/// - 通过 `scan-progress` 事件实时推送已扫描的文件夹 / 文件数量；
/// - `cancel_scan(scan_id)` 可中途取消并返回已读取的部分结果。
#[tauri::command]
pub async fn scan_recursive(
    app: AppHandle,
    path: String,
    scan_id: u64,
    include_hidden: bool,
    max_entries: Option<usize>,
) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        scan_recursive_blocking(app, path, scan_id, include_hidden, max_entries)
    })
    .await
    .map_err(|err| format!("递归读取线程异常：{err}"))?
}

fn scan_recursive_blocking(
    app: AppHandle,
    path: String,
    scan_id: u64,
    include_hidden: bool,
    max_entries: Option<usize>,
) -> Result<ScanResult, String> {
    let started = Instant::now();
    ACTIVE_SCAN.store(scan_id, Ordering::SeqCst);
    CANCEL_REQUESTED.store(false, Ordering::SeqCst);

    let cancel = || {
        CANCEL_REQUESTED.load(Ordering::SeqCst) && ACTIVE_SCAN.load(Ordering::SeqCst) == scan_id
    };
    let mut last_dirs = usize::MAX;
    let mut last_files = usize::MAX;

    let outcome = fs::scan_recursive(
        &path,
        include_hidden,
        max_entries.unwrap_or(DEFAULT_MAX_ENTRIES),
        &cancel,
        |p: &ScanProgress| {
            // 核心库已按 80 ms 节流；这里只在计数变化时推送，进一步减少事件量
            if p.dirs == last_dirs && p.files == last_files {
                return;
            }
            last_dirs = p.dirs;
            last_files = p.files;
            let _ = app.emit(
                "scan-progress",
                ScanProgressEvent {
                    scan_id,
                    dirs: p.dirs,
                    files: p.files,
                    current: p.current.clone(),
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    done: false,
                    cancelled: false,
                    truncated: false,
                },
            );
        },
    );

    if ACTIVE_SCAN.load(Ordering::SeqCst) == scan_id {
        ACTIVE_SCAN.store(0, Ordering::SeqCst);
    }

    match outcome {
        Ok((entries, stats)) => {
            let elapsed_ms = started.elapsed().as_millis() as u64;
            let _ = app.emit(
                "scan-progress",
                ScanProgressEvent {
                    scan_id,
                    dirs: stats.dirs,
                    files: stats.files,
                    current: path,
                    elapsed_ms,
                    done: true,
                    cancelled: stats.cancelled,
                    truncated: stats.truncated,
                },
            );
            Ok(ScanResult {
                entries,
                dirs: stats.dirs,
                files: stats.files,
                skipped: stats.skipped,
                truncated: stats.truncated,
                cancelled: stats.cancelled,
                elapsed_ms,
            })
        }
        Err(err) => Err(err),
    }
}

/// 取消正在进行的递归扫描（只对当前任务生效）。
#[tauri::command]
pub fn cancel_scan(scan_id: u64) {
    if ACTIVE_SCAN.load(Ordering::SeqCst) == scan_id {
        CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    }
}
