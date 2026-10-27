//! WMFold —— 化学文件资源管理器（GPUI Kit 版）。
//!
//! 原 Tauri + Vue 实现已迁移为纯 Rust 桌面应用：
//! - 界面：`gpui-kit`（GPUI + GPUI Base + GPUI Component）
//! - 化学解析：用户自研的 `rswfn`（保留了全部安全防护逻辑）

// Windows 下 release 构建不弹出额外的控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chem;
mod chem_columns;
mod format;
mod fs_ops;
mod models;
mod paths;
mod settings;
mod settings_view;
mod table;

use gpui_kit::assets::AllAssets;
use gpui_kit::component::table::TableState;
use gpui_kit::component::Root;
use gpui_kit::*;

use app::{
    CopySelected, CutSelected, DeleteSelected, GoBack, GoForward, GoUp, NewFile, NewFolder,
    PasteClipboard, RefreshList, RenameSelected, WMFoldApp,
};
use table::FileTableDelegate;

fn main() {
    application().with_assets(AllAssets).run(|cx: &mut App| {
        init(cx);

        // 全局快捷键：仅在 WMFold 上下文（非输入框）中生效。
        cx.bind_keys([
            KeyBinding::new("alt-left", GoBack, Some("WMFold")),
            KeyBinding::new("alt-right", GoForward, Some("WMFold")),
            KeyBinding::new("backspace", GoUp, Some("WMFold")),
            KeyBinding::new("f5", RefreshList, Some("WMFold")),
            KeyBinding::new("delete", DeleteSelected, Some("WMFold")),
            KeyBinding::new("f2", RenameSelected, Some("WMFold")),
            KeyBinding::new("ctrl-c", CopySelected, Some("WMFold")),
            KeyBinding::new("ctrl-x", CutSelected, Some("WMFold")),
            KeyBinding::new("ctrl-v", PasteClipboard, Some("WMFold")),
            KeyBinding::new("ctrl-shift-n", NewFolder, Some("WMFold")),
            KeyBinding::new("ctrl-n", NewFile, Some("WMFold")),
        ]);

        let bounds = Bounds::centered(None, size(px(1200.0), px(780.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.0), px(560.0))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("WMFold - 化学文件资源管理器");

                // 先构造主视图，再创建表格：
                // 表格 delegate 需要读取主视图实体，因此不能在主视图构造期间创建。
                let app = cx.new(|cx| WMFoldApp::new(window, cx));
                let delegate = FileTableDelegate::new(app.downgrade());
                let table = cx.new(|cx| {
                    TableState::new(delegate, window, cx)
                        .row_selectable(true)
                        .cell_selectable(false)
                        .col_movable(false)
                        .col_resizable(true)
                        .sortable(true)
                });
                app.update(cx, |app, cx| app.attach_table(table, window, cx));
                app.update(cx, |app, cx| app.bootstrap(cx));

                cx.new(|cx| Root::new(app, window, cx))
            },
        )
        .expect("failed to open window");

        cx.activate(true);
    });
}
