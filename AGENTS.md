# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, Codex, etc.) working
in this repository. Read it before making changes.

## Project Overview

**WMFold** is a Windows-style file explorer desktop app built for **chemistry /
wavefunction analysis**. It behaves like the built-in Windows Explorer but adds
specialized columns that show molecular information (formula, atom count, charge,
multiplicity, basis-set size, molecular weight, energy) parsed from common
chemistry files.

- **Stack**: pure **Rust** desktop app built on **GPUI Kit** (`gpui-kit` = GPUI +
  GPUI Base + GPUI Component, the UI framework behind Zed). One binary crate,
  no web frontend, no Tauri.
- **Chemistry parsing**: reuses the user's own wavefunction library **`rswfn`**
  (located at `E:\code\rswfn`, outside this repo) as a path dependency.
- **Platform**: Windows only in practice (MSVC toolchain, GPU rendering via
  wgpu/DirectX). Linear algebra inside `rswfn` uses the pure-Rust `faer` crate —
  **no MKL / BLAS / LAPACK** dependency.
- The pre-migration Tauri + Vue implementation is archived under `legacy/`
  (not built, kept for reference only).

## Commands

```bash
cargo run                 # 运行应用（开发模式，等价于 npm run dev）
cargo build               # 快速编译 + 类型检查
cargo build --release     # 产出 target/release/wmfold.exe
cargo test                # 运行全部单元测试（bin crate，无 --lib）
cargo check               # 仅类型检查
```

npm 脚本（根目录 `package.json`，只调用 cargo / node 脚本）：

```bash
npm run dev          # cargo run
npm run build        # cargo build --release
npm run build:app    # 补丁号 +1 → cargo build --release → 生成带版本号 exe
npm run bump:version # 仅递增补丁号（Cargo.toml + package.json）
npm run rename:exe   # 仅把 wmfold.exe 复制为 WMFold_<version>.exe
```

> 注意：这是 **bin crate**，没有 lib target，因此 `cargo test --lib` 会报
> “no library targets found”，请直接使用 `cargo test`。

## Versioning & named build outputs

- **每次编译，版本号最后一个数 +1**：运行 `npm run build:app` 时，
  `scripts/bump-version.mjs` 会把根目录 `Cargo.toml` 的版本号递增
  （补丁号：如 `0.3.3 → 0.3.4`），并同步写回 `package.json`。
- **生成的程序名包含版本号**：`scripts/rename-exe.mjs` 会把
  `target/release/wmfold.exe` 复制为 `target/release/WMFold_<version>.exe`。
- 版本号以 `Cargo.toml` 的 `[package] version` 为准；不要在
  `beforeBuildCommand` 之类的钩子里改版本。

## Architecture / Layout

```
Cargo.toml              # crate `wmfold`，binary = wmfold（src/main.rs）
src/
  main.rs               # gpui-kit 入口：AllAssets、init(cx)、快捷键绑定、开窗
  app.rs                # 主界面 + 全部状态与交互（对应原 Pinia store + App.vue）
  table.rs              # 文件列表 DataTable 的 delegate（列/行/单元格/右键菜单）
  settings_view.rs      # 设置对话框内容（每种类型显示哪些化学列）
  chem.rs               # 化学文件解析（SAFETY-CRITICAL，见下）
  fs_ops.rs             # 目录列表、文件操作、磁盘、快速访问
  models.rs             # FileEntry / ChemMeta / DriveInfo / QuickAccess
  chem_columns.rs       # 化学列定义、各类型可用列、筛选类型常量
  format.rs             # 大小/日期格式化、图标、配色、自然排序
  paths.rs              # Windows 路径、面包屑、父子路径（THIS_PC 虚拟根）
  settings.rs           # 用户设置持久化（%APPDATA%\wmfold\settings.json）
scripts/                # bump-version.mjs / rename-exe.mjs
legacy/                 # 迁移前的 Vue 前端与 Tauri 后端（不参与构建）
```

### 界面组成（`app.rs`）

`WMFoldApp` 是唯一的根视图（`impl Render`），自上而下渲染：

1. `render_toolbar` —— 后退/前进/向上/刷新、面包屑（双击编辑地址）、搜索框
2. `render_ribbon` —— 新建/删除/重命名/复制/剪切/粘贴/设置/预览开关
3. `render_filter_bar` —— 各文件类型筛选按钮（带颜色圆点与数量）
4. `render_sidebar` + 文件表格（`DataTable`）+ `render_chem_panel`
5. `render_status_bar` —— 项目数、已选数、化学解析状态、当前路径

对话框与消息：

- 文本输入类（新建文件夹 / 新建文件 / 重命名）：`window.open_dialog` +
  `Input`（实体在打开时创建，见 `open_prompt`）。
- 删除确认：`window.open_alert_dialog`（`AlertDialog::confirm()`）。
- 设置：`window.open_dialog` + `SettingsView` 实体（自身持有列设置并可重绘）。
- 轻提示：`window.push_notification(Notification::success/…, cx)`。

### 全局快捷键

在 `main.rs` 用 `cx.bind_keys` 注册，作用域为 `Some("WMFold")`；
根元素通过 `.key_context("WMFold")` 提供该上下文（`app.rs::render`）。
Action 类型在 `app.rs` 顶部用 `gpui_kit::actions!` 定义，通过
`.on_action(cx.listener(...))` 处理。输入框有更深的 `Input` key context，
因此 `Delete` / `Backspace` / `Ctrl+C` 等在输入时不会误触发。

| 快捷键 | 动作 |
| --- | --- |
| `Alt+←` / `Alt+→` | 后退 / 前进 |
| `Backspace` | 上一级 |
| `F5` | 刷新 |
| `Delete` | 删除选中（回收站） |
| `F2` | 重命名 |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | 复制 / 剪切 / 粘贴 |
| `Ctrl+N` / `Ctrl+Shift+N` | 新建文件 / 新建文件夹 |

## CRITICAL Safety Constraint (chemistry parsing)

**Never call `rswfn::base::Mole::from_file` directly.**

Several of `rswfn`'s readers terminate the whole process or panic on malformed input:

- `GjfReader::get_atoms` calls `std::process::exit(0)` when no charge/multiplicity
  line is found.
- `LogReader::get_coefs` calls `std::process::exit(0)` when the log lacks `pop=full`.
  (`Mole::from_file` always calls `get_coefs`, so it is unsafe for almost all `.log` files.)
- Various readers use `panic!`/`unwrap`/`expect` on incomplete data.

Because a panic or `exit` would crash the entire file explorer, all parsing must go
through the hardened entry points in `src/chem.rs`:

- **`read_atoms_basis(path, kind)`** — the only safe gateway that (1) pre-checks
  gjf / log files to avoid `process::exit`, (2) wraps reader calls in
  `catch_unwind`, (3) only calls `get_atoms()` and `get_basis()` — **never `get_coefs()`**.
- **`parse_chem(path)`** / public `read_chem` / `read_chem_batch` — use
  `read_atoms_basis` internally and fall back to safe defaults on failure.
- **`kind_from_ext(ext)`** — maps an extension to a chem kind. Use this instead of
  `rswfn::reader::get_fileType` (which `panic!`s on unknown types).

When adding a new format, add it to `kind_from_ext`, `type_label`, and the `match`
inside `read_atoms_basis`, plus the frontend labels in `src/format.rs` and
`src/chem_columns.rs`.

## Important Constraints / Gotchas

### GPUI entity borrow rules (最容易踩的坑)

GPUI 的 `Entity<T>` 是**单借用**模型：`Entity::read` / `update` 在实体已被借用时
会 **panic**（`cannot read ... while it is already being updated`）。因此：

- **不要在实体自身的构造函数里读取自己。**
  例如 `cx.new(|cx| WMFoldApp::new(cx))` 期间，任何 `app.read(cx)` 都会 panic。
  需要初始数据时直接作为参数传入（参见 `SettingsView::new(app, columns, cx)`）。
- **表格 delegate 不要读取 `WMFoldApp`。**
  表格的 `refresh` / `render_td` 往往发生在主视图正被借用时（例如
  `app.update(...)` 里调用 `sync_table` → `table.refresh()`）。
  `FileTableDelegate` 因此**自带** `columns / rows / selected / loading`，
  由 `WMFoldApp::sync_table` 在进入表格借用之前算好并推送；`app` 弱引用只用于
  把点击/菜单操作回调回主视图。
- **不要在 `TableState` 事件回调里同步操作表格。**
  `TableEvent` 是在 `TableState` 被借用时同步派发的，事件里再 `table.update(...)`
  会重入。`attach_table` 中的双击处理先用后台任务让出一次执行权再处理。
- 表格创建必须**在**主视图构造完成之后（见 `main.rs` 的顺序），否则
  `TableState::new` 内部调用 `columns_count` 会读到正被构造的主视图。

### gpui-kit 的模块路径

`gpui_kit::component::*` 的**根**只导出少量项（`Icon`、`h_flex`、`v_flex`、
`ActiveTheme`、`Sizable`、`Disableable`、`WindowExt`、`InteractiveElementExt`、
`StyledExt`、`Root`…）。大部分组件要从子模块导入：

```rust
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::table::{Column, ColumnSort, DataTable, TableDelegate, TableEvent, TableState};
```

GPUI 本体用 `use gpui_kit::*;`（即 `gpui`），`prelude::FluentBuilder`（提供
`.when()`）需要显式 `use gpui_kit::prelude::FluentBuilder as _;`。
`gpui_kit::actions!` 用于定义 action（避免要求下游依赖名为 `gpui` 的 crate）。

### Icons

图标来自 `gpui-kit-assets` 的 **完整 Lucide 目录**，通过
`gpui_kit::assets::IconName`（如 `Folder`、`FileText`、`Scissors`、`Monitor`、
`HardDrive`、`RefreshCw`、`TriangleAlert`…）引用；`main.rs` 注册
`AllAssets` 以嵌入全部图标。名称由 svg 文件名 PascalCase 得到
（`clipboard-paste.svg` → `IconName::ClipboardPaste`）。
用法：`Icon::new(IconName::Folder).size_4().text_color(color)`，
按钮上直接 `.icon(IconName::Folder)`。

### Async / 并发

- 文件系统与化学解析都放到后台执行器：
  `cx.spawn(async move |weak, cx| { let r = cx.background_executor().spawn(async { … }).await; weak.update(cx, |app, cx| …).ok(); }).detach();`
- **不要**在渲染或事件回调里做阻塞 IO。
- 快速切换目录时用 `load_generation` 计数丢弃过期结果（见 `load_content` / `fill_chem`）。

### rswfn path dependency

`Cargo.toml` 有 `rswfn = { path = "../rswfn" }`。这要求项目位于
`E:\code\wmfold` 且 `E:\code\rswfn` 存在。移动项目会破坏构建；请同步修改路径。

### rswfn uses pure-Rust `faer` (no MKL / BLAS)

`rswfn` 已移除 Intel MKL / `ndarray-linalg`，改用纯 Rust 的 `faer`，
`default = []`。后端构建**不需要任何系统 BLAS / LAPACK / MKL**。
旧的 `intel-mkl` / `ndarray-linalg` / `use-lina` / `use-imkl` / `use-blas`
feature 名已不存在，**不要**尝试重新启用。

### rswfn panics are still "noisy"

`catch_unwind` 能阻止崩溃，但默认 panic hook 仍会把回溯打印到 stderr。
开发时控制台会看到这些输出；发布版本 stderr 被丢弃。这是预期行为——
**不要**通过移除 `catch_unwind` 来“修复”。

### Release panic mode

根 `Cargo.toml` 的 `[profile.release]` **没有**设置 `panic = "abort"`，
所以 release 也会 unwind（`catch_unwind` 才能生效）。
`rswfn` 自己的 `[profile.release] panic = "abort"` 作为依赖时不生效。

### Windows path handling

后端路径使用反斜杠，`src/paths.rs` 负责规范化，`::ThisPC` 是虚拟“此电脑”根
（列出所有盘符）。**不要**在别处手动拼接路径，统一走 `paths::` 里的
`parent_path` / `child_path` / `join_path` / `breadcrumb_paths`。

### File listing performance

`fs_ops::list_dir` **不**解析化学元数据（保持快速）。列表渲染后，只有当筛选类型
是化学类型时，才调用 `read_chem_batch` 填充化学列。
**保持这个分离**——不要在 `list_dir` 里解析化学信息。

### 化学列设置持久化

列设置存在 `%APPDATA%\wmfold\settings.json`（非 Windows 用 `$HOME`），
由 `settings.rs` 读写。加载时会过滤未知类型/未知列，避免坏配置导致界面异常。

### Table 选择模型

GPUI Component 的 `TableState` 只支持**单选**（`selected_row`）。
多选由我们自己在复选框列 + `WMFoldApp::selected: HashSet<String>` 中实现，
「全选」是表头的复选框（`render_th` 的第 0 列）。行单击会选中该行并更新预览，
双击打开（目录进入、文件用系统默认程序打开）。

### 版本号来源

版本号只在根 `Cargo.toml`（`[package] version`）与 `package.json` 中，
由 `scripts/bump-version.mjs` 同步；`scripts/rename-exe.mjs` 读取 `Cargo.toml`。

## Development Conventions

- **状态**：所有界面状态都在 `WMFoldApp`（`src/app.rs`）。
  组件方法是 `&mut self` 的渲染函数；避免在别处另建状态。
- **排序**：文件夹永远置顶；列排序由 `set_sort` 更新 `sort_column`/`sort_order`
  后 `recompute_view` 重算 `view`，再把结果推给表格。
- **过滤/搜索**：改动 `filter_type` 或 `search` 后调用 `recompute_view()` +
  `sync_table(cx)` + `cx.notify()`。
- **表格列**：`WMFoldApp::table_columns()` 生成列定义（复选框列 + 名称/类型/大小/
  修改日期/创建日期 + 当前筛选类型的化学列）。只有列集合变化时才 `refresh()`
  （否则会重置用户拖动的列宽）。
- **错误处理**：IO/解析错误写入 `WMFoldApp::error` 或通过
  `window.push_notification(Notification::error(..))` 提示，绝不 panic。
- **测试**：单元测试位于 `chem.rs` / `fs_ops.rs` / `paths.rs` 的
  `#[cfg(test)] mod tests`，用 `cargo test` 运行。
  它们创建临时文件并断言解析结果。

## Sample Test Data

测试用的示例文件（XYZ / GJF / MOL / log 能量行）由测试模块用 `std::fs`
在临时目录内联生成。注意：XYZ 解析要求坐标带小数点（如 `O 0.0 0.0 0.0`），
GJF 必须包含 `charge mult` 行。

## Legacy code

`legacy/web`（Vue 3 + Element Plus）与 `legacy/src-tauri`（Tauri 2 后端）
是迁移前的实现，**不参与构建、不要在里面改功能**。如需参考旧行为可以阅读，
但新功能一律写在根目录的 Rust crate 中。
