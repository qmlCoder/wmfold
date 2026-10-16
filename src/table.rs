//! 文件列表表格的 delegate（基于 gpui-kit / gpui-component 的 DataTable）。
//!
//! 注意：delegate 不直接读取 `WMFoldApp` 实体。gpui 的实体是单借用模型，
//! 表格的 refresh/render 常常发生在主视图已被借用的时候，此时再去读主视图
//! 会 panic。因此所有展示数据（列、行、选中项）都由主视图推送到这里。
//! `app` 只用于把用户操作回调回主视图。

use std::collections::HashSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{SortOrder, WMFoldApp};
use crate::format::{
    chem_color, chem_icon, file_icon, format_date, format_size, kind_short, type_label,
};
use crate::models::FileEntry;

/// 一列的展示配置
#[derive(Debug, Clone)]
pub struct TableCol {
    pub key: String,
    pub label: String,
    pub width: f32,
    pub min_width: f32,
    pub right: bool,
    pub sortable: bool,
    pub selectable: bool,
    pub resizable: bool,
}

/// 文件列表 delegate。
pub struct FileTableDelegate {
    /// 仅用于回调主视图（点击 / 菜单），不在渲染期间读取。
    pub app: WeakEntity<WMFoldApp>,
    pub columns: Vec<TableCol>,
    pub rows: Vec<FileEntry>,
    pub selected: HashSet<String>,
    pub loading: bool,
}

impl FileTableDelegate {
    pub fn new(app: WeakEntity<WMFoldApp>) -> Self {
        Self {
            app,
            columns: Vec::new(),
            rows: Vec::new(),
            selected: HashSet::new(),
            loading: false,
        }
    }

    /// 用主视图的最新状态刷新本地数据。
    pub fn set_data(
        &mut self,
        columns: Vec<TableCol>,
        rows: Vec<FileEntry>,
        selected: HashSet<String>,
        loading: bool,
    ) {
        self.columns = columns;
        self.rows = rows;
        self.selected = selected;
        self.loading = loading;
    }
}

/// 单元格中显示的化学数值文本
fn chem_text(entry: &FileEntry, key: &str) -> String {
    let Some(c) = entry.chem.as_ref().filter(|_| entry.is_chem) else {
        return dash();
    };
    match key {
        "formula" => non_empty(&c.formula),
        "nAtoms" => (c.n_atoms > 0).then(|| c.n_atoms.to_string()).unwrap_or_else(dash),
        "charge" => c.charge.to_string(),
        "multip" => c.multip.to_string(),
        "nBasis" => (c.n_basis > 0).then(|| c.n_basis.to_string()).unwrap_or_else(dash),
        "molW" => (c.mol_w > 0.0).then(|| format!("{:.3}", c.mol_w)).unwrap_or_else(dash),
        "energy" => c.energy.map(|e| format!("{:.6}", e)).unwrap_or_else(dash),
        _ => dash(),
    }
}

fn dash() -> String {
    "—".to_string()
}

fn non_empty(s: &str) -> String {
    if s.is_empty() {
        dash()
    } else {
        s.to_string()
    }
}

impl TableDelegate for FileTableDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        let Some(col) = self.columns.get(col_ix) else {
            return Column::new("", "");
        };

        let mut column = Column::new(col.key.clone(), col.label.clone())
            .width(px(col.width))
            .min_width(px(col.min_width))
            .resizable(col.resizable)
            .movable(false)
            .selectable(col.selectable);
        if col.right {
            column = column.text_right();
        }
        if col.sortable {
            column = column.sortable();
        }
        column
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let Some(key) = self.columns.get(col_ix).map(|c| c.key.clone()) else {
            return;
        };
        let order = match sort {
            ColumnSort::Descending => SortOrder::Descending,
            _ => SortOrder::Ascending,
        };

        let Some(app) = self.app.upgrade() else {
            return;
        };
        // 只更新主视图状态，不要在其中调用 sync_table（会重入表格借用）。
        let _ = app.update(cx, |app, cx| {
            app.set_sort(&key, order);
            cx.notify();
        });
        let (rows, selected, loading) = app.read_with(cx, |app, _| {
            (app.view.clone(), app.selected.clone(), app.loading)
        });
        self.rows = rows;
        self.selected = selected;
        self.loading = loading;
        cx.notify();
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let is_check_col = self
            .columns
            .get(col_ix)
            .map(|c| c.key == "__check__")
            .unwrap_or(false);

        if is_check_col {
            let all_selected =
                !self.rows.is_empty() && self.rows.iter().all(|e| self.selected.contains(&e.path));
            let weak = self.app.clone();
            let rows = self.rows.clone();
            return Checkbox::new("wmfold-select-all")
                .checked(all_selected)
                .on_change(move |checked, _, cx| {
                    let checked = *checked;
                    let rows = rows.clone();
                    let _ = weak.update(cx, |app, cx| {
                        if checked {
                            app.selected.extend(rows.into_iter().map(|e| e.path));
                        } else {
                            app.selected.clear();
                        }
                        app.sync_table(cx);
                        cx.notify();
                    });
                })
                .into_any_element();
        }

        let label = self
            .columns
            .get(col_ix)
            .map(|c| c.label.clone())
            .unwrap_or_default();
        div()
            .truncate()
            .text_color(cx.theme().muted_foreground)
            .child(SharedString::from(label))
            .into_any_element()
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (Some(entry), Some(col)) = (self.rows.get(row_ix).cloned(), self.columns.get(col_ix).cloned())
        else {
            return div().into_any_element();
        };

        let theme = cx.theme();
        let fg = theme.foreground;
        let muted = theme.muted_foreground;

        match col.key.as_str() {
            "__check__" => {
                let selected = self.selected.contains(&entry.path);
                let weak = self.app.clone();
                let path = entry.path.clone();
                Checkbox::new(SharedString::from(format!("wmfold-check-{row_ix}")))
                    .checked(selected)
                    .on_change(move |checked, _, cx| {
                        let checked = *checked;
                        let path = path.clone();
                        let _ = weak.update(cx, |app, cx| {
                            app.set_selected(&path, checked);
                            app.sync_table(cx);
                            cx.notify();
                        });
                    })
                    .into_any_element()
            }
            "name" => {
                let icon = file_icon(&entry);
                let icon_color = if entry.is_dir {
                    rgb(0xE8A33D).into()
                } else if entry.is_chem {
                    chem_color(&entry.kind)
                } else {
                    muted
                };
                h_flex()
                    .gap_2()
                    .items_center()
                    .min_w_0()
                    .child(Icon::new(icon).size_4().text_color(icon_color))
                    .child(
                        div()
                            .truncate()
                            .text_color(fg)
                            .child(SharedString::from(entry.name.clone())),
                    )
                    .into_any_element()
            }
            "type" => {
                if entry.is_chem {
                    let color = chem_color(&entry.kind);
                    div()
                        .truncate()
                        .text_color(color)
                        .child(SharedString::from(format!(
                            "{} · {}",
                            kind_short(&entry.kind),
                            type_label(&entry)
                        )))
                        .into_any_element()
                } else {
                    div()
                        .truncate()
                        .text_color(muted)
                        .child(SharedString::from(type_label(&entry)))
                        .into_any_element()
                }
            }
            "size" => div()
                .text_color(muted)
                .child(SharedString::from(if entry.is_dir {
                    "—".to_string()
                } else {
                    format_size(entry.size)
                }))
                .into_any_element(),
            "modified" => div()
                .text_color(muted)
                .child(SharedString::from(format_date(entry.modified)))
                .into_any_element(),
            "created" => div()
                .text_color(muted)
                .child(SharedString::from(format_date(entry.created)))
                .into_any_element(),
            other => {
                let text = chem_text(&entry, other);
                let is_formula = other == "formula";
                div()
                    .truncate()
                    .when(is_formula, |d| d.text_color(fg))
                    .when(!is_formula, |d| d.text_color(muted))
                    .child(SharedString::from(text))
                    .into_any_element()
            }
        }
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(IconName::FolderOpen).size_8())
            .child("此文件夹为空")
            .into_any_element()
    }

    fn loading(&self, _cx: &App) -> bool {
        self.loading
    }

    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> PopupMenu {
        let Some(entry) = self.rows.get(row_ix).cloned() else {
            return menu;
        };

        let weak = self.app.clone();
        let path = entry.path.clone();
        let name = entry.name.clone();

        // 打开
        let mut menu = menu.item({
            let weak = weak.clone();
            let path = path.clone();
            PopupMenuItem::new("打开")
                .icon(IconName::FolderOpen)
                .on_click(move |_, _, cx| {
                    let path = path.clone();
                    let _ = weak.update(cx, |app, cx| app.open_entry_path(&path, cx));
                })
        });

        if entry.is_chem {
            menu = menu.item({
                let weak = weak.clone();
                let path = path.clone();
                let kind = entry.kind.clone();
                PopupMenuItem::new("查看化学详情")
                    .icon(chem_icon(&kind))
                    .on_click(move |_, _, cx| {
                        let path = path.clone();
                        let _ = weak.update(cx, |app, cx| {
                            app.preview_path = Some(path.clone());
                            app.show_chem_panel = true;
                            cx.notify();
                        });
                    })
            });
        }

        menu = menu.separator();

        menu = menu.item({
            let weak = weak.clone();
            let path = path.clone();
            PopupMenuItem::new("复制")
                .icon(IconName::Copy)
                .on_click(move |_, _, cx| {
                    let path = path.clone();
                    let _ = weak.update(cx, |app, cx| app.copy_selected_to_clipboard(true, &path, cx));
                })
        });

        menu = menu.item({
            let weak = weak.clone();
            let path = path.clone();
            PopupMenuItem::new("剪切")
                .icon(IconName::Scissors)
                .on_click(move |_, _, cx| {
                    let path = path.clone();
                    let _ = weak.update(cx, |app, cx| app.copy_selected_to_clipboard(false, &path, cx));
                })
        });

        menu = menu.item({
            let weak = weak.clone();
            let path = path.clone();
            let name = name.clone();
            PopupMenuItem::new("重命名")
                .icon(IconName::Pencil)
                .on_click(move |_, window, cx| {
                    let path = path.clone();
                    let name = name.clone();
                    let _ = weak.update(cx, |app, cx| {
                        app.open_rename_dialog_for(&path, &name, window, cx)
                    });
                })
        });

        menu = menu.separator();

        menu.item({
            let weak = weak.clone();
            let path = path.clone();
            PopupMenuItem::new("删除")
                .icon(IconName::Delete)
                .on_click(move |_, window, cx| {
                    let path = path.clone();
                    let _ = weak.update(cx, |app, cx| app.confirm_delete_paths(vec![path], window, cx));
                })
        })
    }
}
