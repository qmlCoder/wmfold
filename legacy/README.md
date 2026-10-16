# legacy —— 迁移前的实现（Tauri + Vue）

本目录保存 **WMFold 在迁移到 GPUI Kit 之前**的完整实现，仅作参考/对照，
**不参与当前应用的构建**：

- `web/` —— 旧的 Vue 3 + TypeScript + Element Plus 前端（Vite 工程）。
- `src-tauri/` —— 旧的 Tauri 2 Rust 后端（`chem.rs` / `fs_cmds.rs` / `models.rs`）。

迁移后的实现位于仓库根目录的 Rust crate：

```
Cargo.toml
src/
├── main.rs           # 入口：gpui-kit 应用、窗口、全局快捷键
├── app.rs            # 主界面（工具栏 / 命令栏 / 筛选栏 / 侧边栏 / 表格 / 预览 / 状态栏）
├── table.rs          # 文件列表 DataTable delegate
├── settings_view.rs  # 设置对话框内容（每种类型显示哪些化学列）
├── chem.rs           # 化学文件解析（原样迁移，含全部安全防护）
├── fs_ops.rs         # 目录列表 / 文件操作 / 磁盘与快速访问
├── models.rs         # 数据结构
├── chem_columns.rs   # 化学列定义（原 utils/chemColumns.ts）
├── format.rs         # 格式化 / 图标 / 配色（原 utils/format.ts）
├── paths.rs          # Windows 路径与面包屑（原 utils/path.ts）
└── settings.rs       # 用户设置持久化（替代 localStorage）
```

如需彻底移除旧实现，删除本目录即可。
