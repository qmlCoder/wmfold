//! 多窗口支持：把某个机器 tab 拖出成独立窗口、合并回其他窗口。
//!
//! 主窗口 label 为 `main`；独立窗口由前端 `WebviewWindow` 创建，并在打开后
//! 调用 `register_machine_window` 注册，关闭时自动把机器 tab 归还主窗口。

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

/// 窗口 label → (机器 id, 初始目录)。
fn window_machines() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static MAP: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registered_windows() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 给窗口挂上“关闭时归还 tab”的事件处理（每个 label 只挂一次）。
fn attach_close_handler(app: &AppHandle, window: &tauri::WebviewWindow, label: &str) {
    {
        let mut set = match registered_windows().lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if !set.insert(label.to_string()) {
            return;
        }
    }
    let app = app.clone();
    let label = label.to_string();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let machines = window_machines()
                .lock()
                .ok()
                .and_then(|mut map| map.remove(&label))
                .unwrap_or_default();
            for machine in machines {
                let _ = app.emit_to(
                    "main",
                    "machine-attach",
                    json!({ "machine": machine, "path": "" }),
                );
            }
        }
    });
}

/// JS 创建的独立窗口注册到后端，关闭时把机器 tab 归还主窗口。
#[tauri::command]
pub fn register_machine_window(
    app: AppHandle,
    label: String,
    machine: String,
    path: String,
) -> Result<(), String> {
    {
        let mut map = window_machines().lock().map_err(|e| e.to_string())?;
        let list = map.entry(label.clone()).or_default();
        if !list.contains(&machine) {
            list.push(machine);
        }
    }
    if let Some(window) = app.get_webview_window(&label) {
        attach_close_handler(&app, &window, &label);
    }
    Ok(())
}

/// 返回屏幕坐标 (x, y) 下方的窗口 label（用于拖动合并）。
#[tauri::command]
pub fn window_at_point(app: AppHandle, x: i32, y: i32) -> Option<String> {
    for (label, window) in app.webview_windows() {
        if !window.is_visible().unwrap_or(false) {
            continue;
        }
        let pos = match window.outer_position() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let size = match window.outer_size() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if x >= pos.x
            && x < pos.x + size.width as i32
            && y >= pos.y
            && y < pos.y + size.height as i32
        {
            return Some(label);
        }
    }
    None
}

/// 把某台机器的 tab 合并到目标窗口（通过事件通知目标窗口）。
#[tauri::command]
pub fn merge_machine_window(
    app: AppHandle,
    target: String,
    machine: String,
    path: String,
) -> Result<(), String> {
    app.emit_to(
        &target,
        "machine-attach",
        json!({ "machine": machine, "path": path }),
    )
    .map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window(&target) {
        let _ = window.set_focus();
    }
    Ok(())
}
