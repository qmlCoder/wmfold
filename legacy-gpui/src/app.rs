//! WMFold 主界面（资源管理器 + 化学信息面板）。
//!
//! 这里集中了原 Vue 版 Pinia store 的全部状态与交互逻辑，
//! 视图部分用 gpui-kit（GPUI + GPUI Component）实现。

use std::collections::{HashMap, HashSet};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::table::{DataTable, TableEvent, TableState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, InteractiveElementExt as _,
    Sizable as _, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::chem::read_chem_batch;
use crate::chem_columns::{chem_by_kind, chem_def, is_chem_kind, kind_label, FILTER_KINDS};
use crate::format::{chem_color, chem_icon, file_icon, format_date, format_size, kind_short, type_label, natural_cmp};
use crate::fs_ops;
use crate::models::{ChemMeta, DriveInfo, FileEntry, QuickAccess};
use crate::paths::{self, THIS_PC};
use crate::settings::{self, Settings};
use crate::settings_view::SettingsView;
use crate::table::{FileTableDelegate, TableCol};

gpui_kit::actions!([
    GoBack,
    GoForward,
    GoUp,
    RefreshList,
    DeleteSelected,
    RenameSelected,
    CopySelected,
    CutSelected,
    PasteClipboard,
    NewFolder,
    NewFile
]);

/// 排序方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// 剪贴板动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipAction {
    Copy,
    Cut,
}

#[derive(Debug, Clone)]
pub struct Clipboard {
    pub action: ClipAction,
    pub paths: Vec<String>,
}

/// 输入框对话框的用途
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptKind {
    NewFolder,
    NewFile,
    Rename(String),
}

/// 表格事件去重后的处理动作（延后到表格借用结束后执行）
#[derive(Debug, Clone, Copy)]
enum TableAction {
    Open(usize),
    Select(usize),
}

pub struct WMFoldApp {
    // ---- 导航 ----
    pub current_path: String,
    pub history: Vec<String>,
    pub history_index: usize,
    addr_edit: bool,

    // ---- 数据 ----
    pub entries: Vec<FileEntry>,
    /// 过滤 + 排序后的可见条目（表格实时读取）
    pub view: Vec<FileEntry>,
    pub drives: Vec<DriveInfo>,
    pub quick_access: Vec<QuickAccess>,
    pub home_dir: String,
    pub loading: bool,
    pub chem_loading: bool,

    // ---- 选择 / 排序 / 过滤 ----
    pub selected: HashSet<String>,
    pub sort_column: String,
    pub sort_order: SortOrder,
    pub filter_type: String,
    pub search: String,
    pub error: String,

    // ---- 设置 ----
    pub column_settings: HashMap<String, Vec<String>>,

    // ---- 预览 ----
    pub show_chem_panel: bool,
    pub preview_path: Option<String>,

    // ---- 剪贴板 ----
    pub clipboard: Option<Clipboard>,

    // ---- 子视图 ----
    pub table: Option<Entity<TableState<FileTableDelegate>>>,
    search_input: Entity<InputState>,
    addr_input: Entity<InputState>,
    prompt_input: Entity<InputState>,
    prompt_kind: PromptKind,
    settings_view: Entity<SettingsView>,

    // ---- 内部 ----
    load_generation: u64,
    table_columns_key: String,
    _subscriptions: Vec<Subscription>,
}

impl WMFoldApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app_entity = cx.entity();
        let loaded = settings::load();

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索当前目录"));
        let addr_input = cx.new(|cx| InputState::new(window, cx));
        let prompt_input = cx.new(|cx| InputState::new(window, cx));
        let settings_view = cx.new(|cx| {
            SettingsView::new(app_entity, loaded.column_settings.clone(), cx)
        });

        // 搜索框变化 -> 重新过滤
        let sub_search = cx.subscribe(&search_input, |app: &mut Self, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let value = app.search_input.read(cx).value().to_string();
                app.search = value;
                app.recompute_view();
                app.sync_table(cx);
                cx.notify();
            }
        });

        // 地址栏回车 / 失焦
        let sub_addr = cx.subscribe(&addr_input, |app: &mut Self, _, ev: &InputEvent, cx| {
            match ev {
                InputEvent::PressEnter { .. } => {
                    let value = app.addr_input.read(cx).value().trim().to_string();
                    app.addr_edit = false;
                    if !value.is_empty() {
                        app.navigate(value, cx);
                    }
                    cx.notify();
                }
                InputEvent::Blur => {
                    if app.addr_edit {
                        app.addr_edit = false;
                        cx.notify();
                    }
                }
                _ => {}
            }
        });

        // 表格事件在 `attach_table` 中订阅（表格需在主视图构造完成后创建）。

        Self {
            current_path: THIS_PC.to_string(),
            history: Vec::new(),
            history_index: 0,
            addr_edit: false,
            entries: Vec::new(),
            view: Vec::new(),
            drives: Vec::new(),
            quick_access: Vec::new(),
            home_dir: String::new(),
            loading: false,
            chem_loading: false,
            selected: HashSet::new(),
            sort_column: "name".to_string(),
            sort_order: SortOrder::Ascending,
            filter_type: "all".to_string(),
            search: String::new(),
            error: String::new(),
            column_settings: loaded.column_settings,
            show_chem_panel: true,
            preview_path: None,
            clipboard: None,
            table: None,
            search_input,
            addr_input,
            prompt_input,
            prompt_kind: PromptKind::NewFolder,
            settings_view,
            load_generation: 0,
            table_columns_key: String::new(),
            _subscriptions: vec![sub_search, sub_addr],
        }
    }

    /// 绑定表格实体（必须在主视图构造完成后调用，
    /// 因为 delegate 会在构造表格时读取本实体）。
    pub fn attach_table(
        &mut self,
        table: Entity<TableState<FileTableDelegate>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sub = cx.subscribe(&table, |_app: &mut Self, _, ev: &TableEvent, cx| {
            let action = match ev {
                TableEvent::DoubleClickedRow(ix) => Some(TableAction::Open(*ix)),
                TableEvent::SelectRow(ix) => Some(TableAction::Select(*ix)),
                TableEvent::RightClickedRow(Some(ix)) => Some(TableAction::Select(*ix)),
                _ => None,
            };
            let Some(action) = action else {
                return;
            };
            // 事件是在 TableState 被借用时同步派发的，
            // 因此先用后台任务让出一次执行权，再回主视图处理。
            cx.spawn(async move |weak, cx| {
                cx.background_executor().spawn(async {}).await;
                let _ = weak.update(cx, |app, cx| match action {
                    TableAction::Open(ix) => app.open_row(ix, cx),
                    TableAction::Select(ix) => app.select_row(ix, cx),
                });
            })
            .detach();
        });
        self._subscriptions.push(sub);

        // 让根元素所在的焦点路径包含 WMFold key context，快捷键才可用
        let focus_handle = table.read(cx).focus_handle(cx);
        window.focus(&focus_handle, cx);
        self.table = Some(table);
        self.sync_table(cx);
    }

    // ================= 启动 =================

    pub fn bootstrap(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |weak, cx| {
            let (home, quick, drives) = cx
                .background_executor()
                .spawn(async {
                    (
                        fs_ops::get_home_dir(),
                        fs_ops::get_quick_access(),
                        fs_ops::get_drives(),
                    )
                })
                .await;

            let target = home.clone();
            let _ = weak.update(cx, |app, cx| {
                app.home_dir = home;
                app.quick_access = quick;
                app.drives = drives;
                cx.notify();
            });
            let _ = weak.update(cx, |app, cx| app.navigate(target, cx));
        })
        .detach();
    }

    // ================= 派生状态 =================

    pub fn is_this_pc(&self) -> bool {
        paths::is_this_pc(&self.current_path)
    }

    pub fn crumb_paths(&self) -> Vec<(String, String)> {
        paths::breadcrumb_paths(&self.current_path)
    }

    pub fn chem_cols_visible(&self) -> bool {
        is_chem_kind(&self.filter_type)
    }

    /// 当前筛选类型下应显示的化学列 key
    pub fn visible_chem_cols(&self) -> Vec<&'static str> {
        if !self.chem_cols_visible() {
            return Vec::new();
        }
        let keys: Vec<String> = match self.column_settings.get(&self.filter_type) {
            Some(v) => v.clone(),
            None => chem_by_kind(&self.filter_type)
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };
        keys.iter()
            .filter_map(|k| chem_def(k).map(|d| d.key))
            .collect()
    }

    /// 表格当前列定义
    pub fn table_columns(&self) -> Vec<TableCol> {
        let mut cols = vec![
            TableCol { key: "__check__".into(), label: String::new(), width: 36.0, min_width: 36.0, right: false, sortable: false, selectable: false, resizable: false },
            TableCol { key: "name".into(), label: "名称".into(), width: 280.0, min_width: 180.0, right: false, sortable: true, selectable: true, resizable: true },
            TableCol { key: "type".into(), label: "类型".into(), width: 160.0, min_width: 100.0, right: false, sortable: true, selectable: true, resizable: true },
            TableCol { key: "size".into(), label: "大小".into(), width: 90.0, min_width: 70.0, right: true, sortable: true, selectable: true, resizable: true },
            TableCol { key: "modified".into(), label: "修改日期".into(), width: 140.0, min_width: 110.0, right: false, sortable: true, selectable: true, resizable: true },
            TableCol { key: "created".into(), label: "创建日期".into(), width: 140.0, min_width: 110.0, right: false, sortable: true, selectable: true, resizable: true },
        ];
        for key in self.visible_chem_cols() {
            if let Some(d) = chem_def(key) {
                cols.push(TableCol {
                    key: d.key.into(),
                    label: d.label.into(),
                    width: d.min_width + 30.0,
                    min_width: d.min_width,
                    right: d.right,
                    sortable: d.sortable,
                    selectable: true,
                    resizable: true,
                });
            }
        }
        cols
    }

    /// 各类型在当前目录下的数量
    pub fn type_counts(&self) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        counts.insert("all".to_string(), self.entries.len());
        for k in FILTER_KINDS {
            let n = self
                .entries
                .iter()
                .filter(|e| !e.is_dir && e.kind == *k)
                .count();
            counts.insert((*k).to_string(), n);
        }
        counts
    }

    fn columns_key(&self) -> String {
        let cols = self
            .column_settings
            .get(&self.filter_type)
            .cloned()
            .unwrap_or_default();
        format!("{}|{}", self.filter_type, cols.join(","))
    }

    /// 把主视图状态推送到表格（列集合未变时只重绘，
    /// 变了才 refresh，避免重置用户拖动的列宽）。
    pub fn sync_table(&mut self, cx: &mut Context<Self>) {
        let Some(table) = self.table.clone() else {
            return;
        };

        let columns = self.table_columns();
        let rows = self.view.clone();
        let selected = self.selected.clone();
        let loading = self.loading;

        let key = self.columns_key();
        let need_refresh = key != self.table_columns_key;
        self.table_columns_key = key;

        table.update(cx, |state, cx| {
            state
                .delegate_mut()
                .set_data(columns, rows, selected, loading);
            if need_refresh {
                state.refresh(cx);
            }
            cx.notify();
        });
    }

    // ================= 数据加载 =================

    pub fn recompute_view(&mut self) {
        let q = self.search.trim().to_lowercase();
        let ft = self.filter_type.clone();

        let mut list: Vec<FileEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if !q.is_empty() && !e.name.to_lowercase().contains(&q) {
                    return false;
                }
                match ft.as_str() {
                    "all" => true,
                    "folder" => e.is_dir,
                    "chem" => e.is_chem,
                    other => e.kind == other,
                }
            })
            .cloned()
            .collect();

        let col = self.sort_column.clone();
        let desc = self.sort_order == SortOrder::Descending;

        let mut cmp = |a: &FileEntry, b: &FileEntry| -> std::cmp::Ordering {
            let ord = compare_field(a, b, &col);
            if desc {
                ord.reverse()
            } else {
                ord
            }
        };

        let mut dirs: Vec<FileEntry> = list.iter().filter(|e| e.is_dir).cloned().collect();
        let mut files: Vec<FileEntry> = list.drain(..).filter(|e| !e.is_dir).collect();
        dirs.sort_by(&mut cmp);
        files.sort_by(&mut cmp);
        dirs.extend(files);
        self.view = dirs;
    }

    pub fn set_sort(&mut self, col: &str, order: SortOrder) {
        self.sort_column = col.to_string();
        self.sort_order = order;
        self.recompute_view();
    }

    pub fn set_selected(&mut self, path: &str, selected: bool) {
        if selected {
            self.selected.insert(path.to_string());
        } else {
            self.selected.remove(path);
        }
    }

    pub fn set_column_enabled(&mut self, kind: &str, col: &str, enabled: bool) {
        let entry = self.column_settings.entry(kind.to_string()).or_default();
        if enabled {
            if !entry.iter().any(|c| c == col) {
                entry.push(col.to_string());
            }
        } else {
            entry.retain(|c| c != col);
        }
        settings::save(&Settings {
            column_settings: self.column_settings.clone(),
        });
    }

    pub fn set_column_settings(&mut self, cols: HashMap<String, Vec<String>>) {
        self.column_settings = cols;
        settings::save(&Settings {
            column_settings: self.column_settings.clone(),
        });
    }

    pub fn set_filter(&mut self, kind: &str, cx: &mut Context<Self>) {
        self.filter_type = kind.to_string();
        self.recompute_view();
        self.sync_table(cx);
        if self.chem_cols_visible() && self.entries.iter().any(|e| e.is_chem && e.chem.is_none()) {
            self.fill_chem(cx);
        }
        cx.notify();
    }

    pub fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.navigate_with_history(path, true, cx);
    }

    fn navigate_with_history(&mut self, path: String, push: bool, cx: &mut Context<Self>) {
        self.current_path = path.clone();
        self.selected.clear();
        self.preview_path = None;
        if push {
            self.history.truncate(self.history_index + 1);
            self.history.push(path);
            self.history_index = self.history.len() - 1;
        }
        self.load_content(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_content(cx);
    }

    fn load_content(&mut self, cx: &mut Context<Self>) {
        self.error.clear();
        self.selected.clear();

        if self.is_this_pc() {
            self.entries = self.drives.iter().map(drive_to_entry).collect();
            self.loading = false;
            self.recompute_view();
            self.sync_table(cx);
            cx.notify();
            return;
        }

        self.loading = true;
        self.entries.clear();
        self.recompute_view();
        self.sync_table(cx);
        cx.notify();

        self.load_generation += 1;
        let generation = self.load_generation;
        let path = self.current_path.clone();

        cx.spawn(async move |weak, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { fs_ops::list_dir(&path) })
                .await;

            let _ = weak.update(cx, |app, cx| {
                if app.load_generation != generation {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.entries = entries;
                        app.error.clear();
                    }
                    Err(e) => {
                        app.error = e;
                        app.entries.clear();
                    }
                }
                app.loading = false;
                app.recompute_view();
                app.sync_table(cx);
                if app.chem_cols_visible() {
                    app.fill_chem(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 为当前目录的化学文件解析元数据（并行，失败不影响列表）
    fn fill_chem(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.is_chem && !e.is_dir)
            .map(|e| e.path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }

        self.chem_loading = true;
        cx.notify();

        let generation = self.load_generation;
        let key_paths = paths.clone();

        cx.spawn(async move |weak, cx| {
            let metas = cx
                .background_executor()
                .spawn(async move { read_chem_batch(&paths) })
                .await;

            let _ = weak.update(cx, |app, cx| {
                if app.load_generation != generation {
                    return;
                }
                let map: HashMap<String, ChemMeta> =
                    key_paths.into_iter().zip(metas.into_iter()).collect();
                for entry in app.entries.iter_mut() {
                    if entry.is_chem {
                        entry.chem = map.get(&entry.path).cloned();
                    }
                }
                app.chem_loading = false;
                app.recompute_view();
                app.sync_table(cx);
                cx.notify();
            });
        })
        .detach();
    }

    // ================= 交互动作 =================

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let path = self.history[self.history_index].clone();
            self.navigate_with_history(path, false, cx);
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let path = self.history[self.history_index].clone();
            self.navigate_with_history(path, false, cx);
        }
    }

    pub fn go_up(&mut self, cx: &mut Context<Self>) {
        let parent = paths::parent_path(&self.current_path);
        if parent != self.current_path {
            self.navigate(parent, cx);
        }
    }

    pub fn go_home(&mut self, cx: &mut Context<Self>) {
        if !self.home_dir.is_empty() {
            self.navigate(self.home_dir.clone(), cx);
        }
    }

    pub fn to_this_pc(&mut self, cx: &mut Context<Self>) {
        self.navigate(THIS_PC.to_string(), cx);
    }

    pub fn select_row(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.view.get(ix).cloned() {
            self.selected.clear();
            self.selected.insert(entry.path.clone());
            self.preview_path = Some(entry.path.clone());
            self.sync_table(cx);
            cx.notify();
        }
    }

    pub fn open_row(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(path) = self.view.get(ix).map(|e| e.path.clone()) {
            self.open_entry_path(&path, cx);
        }
    }

    pub fn open_entry_path(&mut self, path: &str, cx: &mut Context<Self>) {
        let is_dir = self
            .entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.is_dir)
            .unwrap_or(false);

        if is_dir {
            self.navigate(path.to_string(), cx);
        } else {
            let path = path.to_string();
            cx.background_executor()
                .spawn(async move {
                    let _ = fs_ops::open_path(&path);
                })
                .detach();
        }
    }

    pub fn set_preview(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        self.preview_path = path;
        cx.notify();
    }

    pub fn toggle_chem_panel(&mut self, cx: &mut Context<Self>) {
        self.show_chem_panel = !self.show_chem_panel;
        cx.notify();
    }

    pub fn copy_selected_to_clipboard(&mut self, is_copy: bool, path: &str, cx: &mut Context<Self>) {
        if !self.selected.contains(path) {
            self.selected.clear();
            self.selected.insert(path.to_string());
        }
        self.store_clipboard(is_copy);
        cx.notify();
    }

    pub fn copy_selected(&mut self, is_copy: bool, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        self.store_clipboard(is_copy);
        cx.notify();
    }

    fn store_clipboard(&mut self, is_copy: bool) {
        let paths: Vec<String> = self
            .view
            .iter()
            .filter(|e| self.selected.contains(&e.path))
            .map(|e| e.path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        self.clipboard = Some(Clipboard {
            action: if is_copy { ClipAction::Copy } else { ClipAction::Cut },
            paths,
        });
    }

    pub fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(clip) = self.clipboard.clone() else {
            return;
        };
        let dest = self.current_path.clone();
        let result = match clip.action {
            ClipAction::Copy => fs_ops::copy_paths(&clip.paths, &dest),
            ClipAction::Cut => fs_ops::move_paths(&clip.paths, &dest),
        };
        match result {
            Ok(()) => {
                window.push_notification(Notification::success("粘贴完成"), cx);
                if clip.action == ClipAction::Cut {
                    self.clipboard = None;
                }
                self.refresh(cx);
            }
            Err(e) => {
                window.push_notification(Notification::error(format!("粘贴失败：{e}")), cx);
            }
        }
    }

    // ---- 对话框 ----

    pub fn open_new_folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_prompt(
            PromptKind::NewFolder,
            "新建文件夹",
            "创建",
            "新建文件夹",
            window,
            cx,
        );
    }

    pub fn open_new_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_prompt(
            PromptKind::NewFile,
            "新建文件",
            "创建",
            "新建化学文件.xyz",
            window,
            cx,
        );
    }

    pub fn open_rename_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry() else {
            window.push_notification(Notification::warning("请先选择一个项目"), cx);
            return;
        };
        self.open_rename_dialog_for(&entry.path.clone(), &entry.name.clone(), window, cx);
    }

    pub fn open_rename_dialog_for(
        &mut self,
        path: &str,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_prompt(
            PromptKind::Rename(path.to_string()),
            "重命名",
            "确定",
            name,
            window,
            cx,
        );
    }

    fn open_prompt(
        &mut self,
        kind: PromptKind,
        title: &str,
        ok_text: &str,
        default_value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prompt_kind = kind;
        self.prompt_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(SharedString::from(default_value.to_string()))
        });

        let input = self.prompt_input.clone();
        let focus_input = input.clone();
        let app = cx.entity();
        let title: SharedString = title.to_string().into();
        let ok_text: SharedString = ok_text.to_string().into();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input = input.clone();
            let app = app.clone();
            dialog
                .title(title.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(ok_text.clone())
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .content({
                    let input = input.clone();
                    move |content, _window, _cx| content.child(Input::new(&input))
                })
                .on_ok(move |_, window, cx| {
                    let value = input.read(cx).value().to_string();
                    let _ = app.update(cx, |app, cx| app.confirm_prompt(value, window, cx));
                    true
                })
        });

        let focus_handle = focus_input.read(cx).focus_handle(cx);
        window.focus(&focus_handle, cx);
    }

    fn confirm_prompt(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        let value = value.trim().to_string();
        if value.is_empty() {
            return;
        }
        let result = match self.prompt_kind.clone() {
            PromptKind::NewFolder => {
                let p = paths::child_path(&self.current_path, &value);
                fs_ops::create_folder(&p)
            }
            PromptKind::NewFile => {
                let p = paths::child_path(&self.current_path, &value);
                fs_ops::create_file(&p)
            }
            PromptKind::Rename(path) => fs_ops::rename_path(&path, &value),
        };
        match result {
            Ok(()) => {
                window.push_notification(Notification::success("操作成功"), cx);
                self.refresh(cx);
            }
            Err(e) => {
                window.push_notification(Notification::error(e), cx);
            }
        }
    }

    pub fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            window.push_notification(Notification::warning("请先选择要删除的项目"), cx);
            return;
        }
        let paths: Vec<String> = self
            .view
            .iter()
            .filter(|e| self.selected.contains(&e.path))
            .map(|e| e.path.clone())
            .collect();
        self.confirm_delete_paths(paths, window, cx);
    }

    pub fn confirm_delete_paths(
        &mut self,
        paths: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        let description = if paths.len() == 1 {
            let name = paths[0]
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&paths[0])
                .to_string();
            format!("确定要删除「{name}」吗？（移入回收站）")
        } else {
            format!("确定要删除选中的 {} 个项目吗？（移入回收站）", paths.len())
        };

        let app = cx.entity();
        window.open_alert_dialog(cx, move |alert, _window, _cx| {
            let app = app.clone();
            let paths = paths.clone();
            alert
                .icon(Icon::new(IconName::TriangleAlert).size_6())
                .title("删除")
                .description(SharedString::from(description.clone()))
                .confirm()
                .ok_text("删除")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("取消")
                .on_ok(move |_, window, cx| {
                    let paths = paths.clone();
                    let _ = app.update(cx, |app, cx| app.delete_paths_now(paths, window, cx));
                    true
                })
        });
    }

    fn delete_paths_now(
        &mut self,
        paths: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match fs_ops::delete_paths(&paths, false) {
            Ok(()) => {
                window.push_notification(
                    Notification::success(format!("已删除 {} 个项目", paths.len())),
                    cx,
                );
                for p in &paths {
                    self.selected.remove(p);
                }
                self.refresh(cx);
            }
            Err(e) => {
                window.push_notification(Notification::error(format!("删除失败：{e}")), cx);
            }
        }
    }

    pub fn open_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.settings_view.clone();
        let reset_view = view.clone();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view = view.clone();
            let reset_view = reset_view.clone();
            dialog
                .title("设置 · 每种文件类型显示的列")
                .w(px(760.0))
                .content(move |content, _window, _cx| content.child(view.clone()))
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("settings-reset")
                                .label("恢复默认")
                                .on_click(move |_, _window, cx| {
                                    let _ = reset_view.update(cx, |v, cx| v.reset(cx));
                                }),
                        )
                        .child(Button::new("settings-ok").primary().label("确定").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        )),
                )
        });
    }

    fn selected_entry(&self) -> Option<&FileEntry> {
        self.view.iter().find(|e| self.selected.contains(&e.path))
    }

    // ================= 渲染 =================

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let accent = cx.theme().accent;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;

        let can_back = self.history_index > 0;
        let can_forward = self.history_index + 1 < self.history.len();
        let can_up = !self.is_this_pc();

        let crumbs = self.crumb_paths();
        let last_ix = crumbs.len().saturating_sub(1);

        let breadcrumb = if self.addr_edit {
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(&self.addr_input).small())
                .into_any_element()
        } else {
            h_flex()
                .id("wmfold-breadcrumb")
                .flex_1()
                .min_w_0()
                .gap_1()
                .items_center()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .on_double_click(cx.listener(|this, _, window, cx| this.begin_addr_edit(window, cx)))
                .children(crumbs.into_iter().enumerate().map(|(i, (label, path))| {
                    let is_last = i == last_ix;
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .id(ElementId::Name(SharedString::from(format!("crumb-{i}"))))
                                .px_1()
                                .rounded(px(4.0))
                                .text_color(if is_last { fg } else { muted })
                                .when(!is_last, |d| d.cursor_pointer().hover(|s| s.bg(accent)))
                                .child(SharedString::from(label))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.navigate(path.clone(), cx);
                                })),
                        )
                        .when(!is_last, |d| {
                            d.child(
                                Icon::new(IconName::ChevronRight)
                                    .size_3()
                                    .text_color(muted),
                            )
                        })
                        .into_any_element()
                }))
                .into_any_element()
        };

        h_flex()
            .w_full()
            .gap_1()
            .px_2()
            .py_1()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(
                Button::new("nav-back")
                    .icon(IconName::ArrowLeft)
                    .label("后退")
                    .small()
                    .ghost()
                    .disabled(!can_back)
                    .on_click(cx.listener(|this, _, _, cx| this.go_back(cx))),
            )
            .child(
                Button::new("nav-forward")
                    .icon(IconName::ArrowRight)
                    .label("前进")
                    .small()
                    .ghost()
                    .disabled(!can_forward)
                    .on_click(cx.listener(|this, _, _, cx| this.go_forward(cx))),
            )
            .child(
                Button::new("nav-up")
                    .icon(IconName::ArrowUp)
                    .label("向上")
                    .small()
                    .ghost()
                    .disabled(!can_up)
                    .on_click(cx.listener(|this, _, _, cx| this.go_up(cx))),
            )
            .child(
                Button::new("nav-refresh")
                    .icon(IconName::RefreshCw)
                    .label("刷新")
                    .small()
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(div().w(px(8.0)))
            .child(breadcrumb)
            .child(
                div()
                    .w(px(220.0))
                    .child(Input::new(&self.search_input).small().cleanable(true)),
            )
    }

    fn begin_addr_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.current_path.clone();
        self.addr_edit = true;
        self.addr_input.update(cx, |state, cx| {
            state.set_value(SharedString::from(value), window, cx);
        });
        let focus_handle = self.addr_input.read(cx).focus_handle(cx);
        window.focus(&focus_handle, cx);
        cx.notify();
    }

    fn render_ribbon(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let has_selection = !self.selected.is_empty();
        let single = self.selected.len() == 1;
        let has_clipboard = self.clipboard.is_some();
        let panel_on = self.show_chem_panel;

        h_flex()
            .w_full()
            .gap_1()
            .px_2()
            .py_1()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(
                Button::new("rb-new-folder")
                    .icon(IconName::FolderPlus)
                    .label("新建文件夹")
                    .small()
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_new_folder_dialog(window, cx)
                    })),
            )
            .child(
                Button::new("rb-new-file")
                    .icon(IconName::FilePlus)
                    .label("新建文件")
                    .small()
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_new_file_dialog(window, cx)
                    })),
            )
            .child(div().w(px(1.0)).h(px(18.0)).bg(border).mx_1())
            .child(
                Button::new("rb-delete")
                    .icon(IconName::Delete)
                    .label("删除")
                    .small()
                    .ghost()
                    .disabled(!has_selection)
                    .on_click(cx.listener(|this, _, window, cx| this.delete_selected(window, cx))),
            )
            .child(
                Button::new("rb-rename")
                    .icon(IconName::Pencil)
                    .label("重命名")
                    .small()
                    .ghost()
                    .disabled(!single)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_rename_dialog(window, cx)
                    })),
            )
            .child(div().w(px(1.0)).h(px(18.0)).bg(border).mx_1())
            .child(
                Button::new("rb-copy")
                    .icon(IconName::Copy)
                    .label("复制")
                    .small()
                    .ghost()
                    .disabled(!has_selection)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_selected(true, cx))),
            )
            .child(
                Button::new("rb-cut")
                    .icon(IconName::Scissors)
                    .label("剪切")
                    .small()
                    .ghost()
                    .disabled(!has_selection)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_selected(false, cx))),
            )
            .child(
                Button::new("rb-paste")
                    .icon(IconName::ClipboardPaste)
                    .label("粘贴")
                    .small()
                    .ghost()
                    .disabled(!has_clipboard)
                    .on_click(cx.listener(|this, _, window, cx| this.paste(window, cx))),
            )
            .child(div().flex_1())
            .child(
                Button::new("rb-settings")
                    .icon(IconName::Settings)
                    .label("设置")
                    .small()
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings_dialog(window, cx)
                    })),
            )
            .child({
                let button = Button::new("rb-preview")
                    .icon(IconName::Eye)
                    .label("预览")
                    .small();
                let button = if panel_on {
                    button.primary()
                } else {
                    button.ghost()
                };
                button.on_click(cx.listener(|this, _, _, cx| this.toggle_chem_panel(cx)))
            })
    }

    fn render_filter_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let accent = cx.theme().accent;
        let counts = self.type_counts();
        let active = self.filter_type.clone();

        let mut row = h_flex()
            .w_full()
            .flex_wrap()
            .gap_1()
            .px_2()
            .py_1()
            .items_center()
            .border_b_1()
            .border_color(border);

        let all_count = counts.get("all").copied().unwrap_or(0);
        row = row.child(filter_chip(
            "filter-all",
            IconName::Files,
            hex(0x909399),
            "全部文件".to_string(),
            all_count,
            active == "all",
            accent,
            cx.listener(|this, _, _, cx| this.set_filter("all", cx)),
        ));

        for kind in FILTER_KINDS {
            let count = counts.get(*kind).copied().unwrap_or(0);
            let color = chem_color(kind);
            let is_active = active == *kind;
            row = row.child(filter_chip(
                &format!("filter-{kind}"),
                chem_icon(kind),
                color,
                kind_label(kind).to_string(),
                count,
                is_active,
                accent,
                cx.listener(move |this, _, _, cx| this.set_filter(kind, cx)),
            ));
        }

        row
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let bg = theme.sidebar;
        let border = theme.sidebar_border;
        let fg = theme.sidebar_foreground;
        let accent = theme.sidebar_accent;
        let muted = theme.muted_foreground;

        let this_pc_items: Vec<QuickAccess> = self
            .quick_access
            .iter()
            .filter(|q| q.path == THIS_PC)
            .cloned()
            .collect();
        let folder_items: Vec<QuickAccess> = self
            .quick_access
            .iter()
            .filter(|q| q.path != THIS_PC)
            .cloned()
            .collect();
        let drives = self.drives.clone();
        let current = self.current_path.clone();

        v_flex()
            .w(px(220.0))
            .h_full()
            .flex_shrink_0()
            .bg(bg)
            .id("wmfold-sidebar")
            .overflow_y_scroll()
            .border_r_1()
            .border_color(border)
            .py_2()
            .child(sidebar_section("快速访问", muted))
            .children(this_pc_items.into_iter().enumerate().map(|(i, q)| {
                sidebar_item(
                    format!("side-pc-{i}"),
                    IconName::Monitor,
                    q.name.clone(),
                    current == q.path,
                    fg,
                    accent,
                    {
                        let path = q.path.clone();
                        cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx))
                    },
                )
            }))
            .children(folder_items.into_iter().enumerate().map(|(i, q)| {
                sidebar_item(
                    format!("side-folder-{i}"),
                    IconName::Folder,
                    q.name.clone(),
                    current == q.path,
                    fg,
                    accent,
                    {
                        let path = q.path.clone();
                        cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx))
                    },
                )
            }))
            .child(sidebar_section("此电脑", muted))
            .children(drives.into_iter().enumerate().map(|(i, d)| {
                let root = format!("{}:\\", d.letter);
                sidebar_item(
                    format!("side-drive-{i}"),
                    IconName::HardDrive,
                    format!("本地磁盘 ({}:)", d.letter),
                    current == root,
                    fg,
                    accent,
                    {
                        let root = root.clone();
                        cx.listener(move |this, _, _, cx| this.navigate(root.clone(), cx))
                    },
                )
            }))
    }

    fn render_chem_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.border;
        let fg = theme.foreground;
        let muted = theme.muted_foreground;
        let bg = theme.muted.opacity(0.25);

        let entry = self
            .preview_path
            .as_ref()
            .and_then(|p| self.entries.iter().find(|e| &e.path == p))
            .cloned()
            .or_else(|| self.selected_entry().cloned());

        let mut panel = v_flex()
            .w(px(300.0))
            .h_full()
            .flex_shrink_0()
            .bg(bg)
            .border_l_1()
            .border_color(border)
            .overflow_hidden();

        panel = panel.child(
            h_flex()
                .w_full()
                .px_3()
                .py_2()
                .justify_between()
                .items_center()
                .border_b_1()
                .border_color(border)
                .child(div().font_weight(FontWeight::SEMIBOLD).child("预览"))
                .child(
                    Button::new("panel-close")
                        .icon(IconName::Close)
                        .text()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_chem_panel(cx))),
                ),
        );

        let Some(entry) = entry else {
            return panel.child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_color(muted)
                    .child(Icon::new(IconName::Eye).size_8())
                    .child("选择要预览的文件"),
            );
        };

        let icon = file_icon(&entry);
        let icon_color = if entry.is_dir {
            hex(0xE8A33D)
        } else if entry.is_chem {
            chem_color(&entry.kind)
        } else {
            muted
        };

        let mut props: Vec<(String, String)> = Vec::new();
        if entry.is_dir {
            props.push(("名称".into(), entry.name.clone()));
            props.push(("类型".into(), "文件夹".into()));
            props.push(("路径".into(), entry.path.clone()));
        } else {
            props.push(("名称".into(), entry.name.clone()));
            props.push(("类型".into(), type_label(&entry)));
            props.push(("大小".into(), format_size(entry.size)));
            props.push(("修改日期".into(), format_date(entry.modified)));
            props.push(("路径".into(), entry.path.clone()));
            if entry.is_chem {
                if let Some(c) = entry.chem.as_ref() {
                    props.push(("化学式".into(), non_empty(&c.formula)));
                    props.push(("原子数".into(), c.n_atoms.to_string()));
                    props.push(("总电荷".into(), c.charge.to_string()));
                    props.push(("多重度".into(), c.multip.to_string()));
                    props.push(("α/β 电子".into(), format!("{}/{}", c.n_alpha, c.n_beta)));
                    props.push((
                        "基组函数".into(),
                        if c.n_basis > 0 {
                            c.n_basis.to_string()
                        } else {
                            "—".into()
                        },
                    ));
                    props.push((
                        "分子量".into(),
                        if c.mol_w > 0.0 {
                            format!("{:.3} g/mol", c.mol_w)
                        } else {
                            "—".into()
                        },
                    ));
                    props.push((
                        "能量".into(),
                        c.energy
                            .map(|e| format!("{e:.6} a.u."))
                            .unwrap_or_else(|| "—".into()),
                    ));
                }
            }
        }

        panel
            .child(
                v_flex()
                    .px_3()
                    .py_2()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(icon).size_6().text_color(icon_color))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(fg)
                                    .child(SharedString::from(entry.name.clone())),
                            ),
                    )
                    .when(entry.is_chem, |d| {
                        let color = chem_color(&entry.kind);
                        d.child(
                            div()
                                .text_color(color)
                                .child(SharedString::from(format!(
                                    "{} · {}",
                                    kind_short(&entry.kind),
                                    type_label(&entry)
                                ))),
                        )
                    }),
            )
            .child(
                v_flex()
                    .id("chem-props")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .pb_3()
                    .children(props.into_iter().map(|(k, v)| {
                        h_flex()
                            .w_full()
                            .py_1()
                            .gap_2()
                            .items_start()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .w(px(78.0))
                                    .flex_shrink_0()
                                    .text_color(muted)
                                    .child(SharedString::from(k)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(fg)
                                    .child(SharedString::from(v)),
                            )
                            .into_any_element()
                    })),
            )
    }

    fn render_status_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let total = self.entries.len();
        let selected = self.selected.len();
        let path = self.current_path.clone();
        let chem_loading = self.chem_loading;

        let mut left = h_flex().gap_3().items_center().child(format!("{total} 个项目"));
        if selected > 0 {
            left = left.child(format!("{selected} 个已选择"));
        }
        if chem_loading {
            left = left.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .text_color(muted)
                    .child(Icon::new(IconName::LoaderCircle).size_3())
                    .child("读取化学信息…"),
            );
        }

        StatusBar::new()
            .left(left)
            .right(
                div()
                    .text_color(muted)
                    .truncate()
                    .child(SharedString::from(path)),
            )
    }

    fn render_error(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        if self.error.is_empty() {
            return None;
        }
        Some(
            h_flex()
                .w_full()
                .px_3()
                .py_1()
                .gap_2()
                .items_center()
                .bg(cx.theme().danger.opacity(0.15))
                .text_color(cx.theme().danger)
                .child(Icon::new(IconName::TriangleAlert).size_4())
                .child(SharedString::from(self.error.clone())),
        )
    }
}

fn non_empty(s: &str) -> String {
    if s.is_empty() {
        "—".to_string()
    } else {
        s.to_string()
    }
}

fn hex(c: u32) -> Hsla {
    rgb(c).into()
}

fn drive_to_entry(d: &DriveInfo) -> FileEntry {
    FileEntry {
        name: if d.label.is_empty() {
            format!("{}:", d.letter)
        } else {
            d.label.clone()
        },
        path: format!("{}:\\", d.letter),
        is_dir: true,
        size: 0,
        created: None,
        modified: None,
        extension: String::new(),
        kind: String::new(),
        is_chem: false,
        is_hidden: false,
        is_symlink: false,
        chem: None,
    }
}

fn compare_field(a: &FileEntry, b: &FileEntry, col: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match col {
        "name" => natural_cmp(&a.name, &b.name),
        "type" => natural_cmp(&type_label(a), &type_label(b)),
        "size" => {
            let va = if a.is_dir { -1.0 } else { a.size as f64 };
            let vb = if b.is_dir { -1.0 } else { b.size as f64 };
            va.partial_cmp(&vb).unwrap_or(Ordering::Equal)
        }
        "modified" => a.modified.unwrap_or(0).cmp(&b.modified.unwrap_or(0)),
        "created" => a.created.unwrap_or(0).cmp(&b.created.unwrap_or(0)),
        "formula" => natural_cmp(
            &a.chem.as_ref().map(|c| c.formula.clone()).unwrap_or_default(),
            &b.chem.as_ref().map(|c| c.formula.clone()).unwrap_or_default(),
        ),
        "nAtoms" => chem_num(a, |c| c.n_atoms as f64).partial_cmp(&chem_num(b, |c| c.n_atoms as f64)).unwrap_or(Ordering::Equal),
        "charge" => (chem_num(a, |c| c.charge as f64) as i64).cmp(&(chem_num(b, |c| c.charge as f64) as i64)),
        "multip" => chem_num(a, |c| c.multip as f64).partial_cmp(&chem_num(b, |c| c.multip as f64)).unwrap_or(Ordering::Equal),
        "nBasis" => chem_num(a, |c| c.n_basis as f64).partial_cmp(&chem_num(b, |c| c.n_basis as f64)).unwrap_or(Ordering::Equal),
        "molW" => chem_num(a, |c| c.mol_w).partial_cmp(&chem_num(b, |c| c.mol_w)).unwrap_or(Ordering::Equal),
        "energy" => {
            let va = a.chem.as_ref().and_then(|c| c.energy).unwrap_or(f64::NEG_INFINITY);
            let vb = b.chem.as_ref().and_then(|c| c.energy).unwrap_or(f64::NEG_INFINITY);
            va.partial_cmp(&vb).unwrap_or(Ordering::Equal)
        }
        _ => natural_cmp(&a.name, &b.name),
    }
}

fn chem_num(entry: &FileEntry, f: impl Fn(&ChemMeta) -> f64) -> f64 {
    entry
        .chem
        .as_ref()
        .filter(|_| entry.is_chem)
        .map(f)
        .unwrap_or(f64::NEG_INFINITY)
}

fn sidebar_section(label: &str, color: Hsla) -> impl IntoElement {
    div()
        .px_3()
        .pt_3()
        .pb_1()
        .text_xs()
        .text_color(color)
        .child(SharedString::from(label.to_string()))
}

fn sidebar_item(
    id: String,
    icon: IconName,
    label: String,
    active: bool,
    fg: Hsla,
    accent: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    h_flex()
        .id(ElementId::Name(SharedString::from(id)))
        .w_full()
        .px_3()
        .py_1()
        .gap_2()
        .items_center()
        .cursor_pointer()
        .text_color(fg)
        .when(active, |d| d.bg(accent))
        .hover(|d| d.bg(accent))
        .child(Icon::new(icon).size_4())
        .child(div().truncate().child(SharedString::from(label)))
        .on_click(on_click)
}

#[allow(clippy::too_many_arguments)]
fn filter_chip(
    id: &str,
    icon: IconName,
    color: Hsla,
    label: String,
    count: usize,
    active: bool,
    accent: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    h_flex()
        .id(ElementId::Name(SharedString::from(id.to_string())))
        .px_2()
        .py_1()
        .gap_1()
        .items_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .when(active, |d| d.bg(accent))
        .hover(|d| d.bg(accent))
        .child(Icon::new(icon).size_3().text_color(color))
        .child(div().text_sm().child(SharedString::from(label)))
        .child(
            div()
                .text_xs()
                .text_color(color)
                .child(SharedString::from(count.to_string())),
        )
        .on_click(on_click)
}

impl Render for WMFoldApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = cx.theme().background;
        let fg = cx.theme().foreground;

        v_flex()
            .key_context("WMFold")
            .size_full()
            .bg(bg)
            .text_color(fg)
            .on_action(cx.listener(|this, _: &GoBack, _w, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &GoForward, _w, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &GoUp, _w, cx| this.go_up(cx)))
            .on_action(cx.listener(|this, _: &RefreshList, _w, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &DeleteSelected, window, cx| {
                this.delete_selected(window, cx)
            }))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| {
                this.open_rename_dialog(window, cx)
            }))
            .on_action(cx.listener(|this, _: &CopySelected, _w, cx| this.copy_selected(true, cx)))
            .on_action(cx.listener(|this, _: &CutSelected, _w, cx| this.copy_selected(false, cx)))
            .on_action(cx.listener(|this, _: &PasteClipboard, window, cx| this.paste(window, cx)))
            .on_action(cx.listener(|this, _: &NewFolder, window, cx| {
                this.open_new_folder_dialog(window, cx)
            }))
            .on_action(cx.listener(|this, _: &NewFile, window, cx| {
                this.open_new_file_dialog(window, cx)
            }))
            .child(self.render_toolbar(cx))
            .child(self.render_ribbon(window, cx))
            .child(self.render_filter_bar(cx))
            .children(self.render_error(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.render_sidebar(cx))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(match self.table.clone() {
                                        Some(table) => DataTable::new(&table)
                                            .stripe(true)
                                            .bordered(true)
                                            .into_any_element(),
                                        None => div().into_any_element(),
                                    }),
                            )
                            .when(self.show_chem_panel, |d| {
                                d.child(self.render_chem_panel(cx))
                            }),
                    ),
            )
            .child(self.render_status_bar(cx))
    }
}
