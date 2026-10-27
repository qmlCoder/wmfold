//! 设置对话框内容：每种文件类型显示哪些化学列。

use std::collections::HashMap;

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::*;

use crate::app::WMFoldApp;
use crate::chem_columns::{chem_by_kind, chem_def, kind_label, FILTER_KINDS};
use crate::format::chem_color;

pub struct SettingsView {
    app: Entity<WMFoldApp>,
    columns: HashMap<String, Vec<String>>,
}

impl SettingsView {
    pub fn new(
        app: Entity<WMFoldApp>,
        columns: HashMap<String, Vec<String>>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self { app, columns }
    }

    fn is_enabled(&self, kind: &str, col: &str) -> bool {
        self.columns
            .get(kind)
            .map(|v| v.iter().any(|c| c == col))
            .unwrap_or(false)
    }

    fn set_enabled(&mut self, kind: &str, col: &str, enabled: bool) {
        let entry = self.columns.entry(kind.to_string()).or_default();
        if enabled {
            if !entry.iter().any(|c| c == col) {
                entry.push(col.to_string());
            }
        } else {
            entry.retain(|c| c != col);
        }
    }

    /// 恢复默认列设置，并同步到主应用。
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.columns = crate::settings::Settings::defaults().column_settings;
        let cols = self.columns.clone();
        let _ = self
            .app
            .update(cx, |app, cx| {
                app.set_column_settings(cols);
                cx.notify();
            });
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let radius_full = cx.theme().radius_full();

        v_flex()
            .gap_4()
            .children(FILTER_KINDS.iter().map(|kind| {
                let kind_s = (*kind).to_string();
                let color = chem_color(kind);

                v_flex()
                    .gap_2()
                    .pb_3()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().size_2().rounded(radius_full).bg(color))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(kind_label(kind)),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_x_4()
                            .gap_y_2()
                            .children(chem_by_kind(kind).iter().map(|col| {
                                let col_s = (*col).to_string();
                                let kind_c = kind_s.clone();
                                let checked = self.is_enabled(kind, col);
                                let label = chem_def(col).map(|d| d.label).unwrap_or(col);

                                Checkbox::new(SharedString::from(format!("set-{kind}-{col}")))
                                    .checked(checked)
                                    .label(label)
                                    .on_change(cx.listener(
                                        move |this, checked: &bool, _window, cx| {
                                            let enabled = *checked;
                                            this.set_enabled(&kind_c, &col_s, enabled);
                                            let (k, c) = (kind_c.clone(), col_s.clone());
                                            let _ = this.app.update(cx, |app, cx| {
                                                app.set_column_enabled(&k, &c, enabled);
                                                cx.notify();
                                            });
                                            cx.notify();
                                        },
                                    ))
                            })),
                    )
            }))
    }
}
