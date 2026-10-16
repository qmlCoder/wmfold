# WMFold · 化学文件资源管理器

一个仿照 Windows 自带文件资源管理器（Explorer）构建的桌面应用，
专为**化学计算 / 波函数分析**场景设计。它覆盖普通文件资源管理器的常用功能，
并能快速解析常见的化学文件，把分子信息以「表头列」的形式展示出来。

界面与窗口全部由 **纯 Rust** 实现，基于 [GPUI Kit](https://gpui-kit.com/)
（GPUI + GPUI Base + GPUI Component，即 Zed 编辑器的 UI 框架及其组件库）。
化学文件的解析复用你自研的波函数分析库 **`rswfn`**（`E:\code\rswfn`）。

> 面向 AI 编码 Agent 的仓库说明见 [AGENTS.md](./AGENTS.md)。
> 迁移前的 Tauri + Vue 实现保存在 [legacy/](./legacy)。

## 技术栈

| 层 | 方案 |
| --- | --- |
| 窗口 / 渲染 / 布局 | `gpui`（经 `gpui-kit` 统一暴露） |
| 组件库 | `gpui-component`（Table / Button / Input / Checkbox / Dialog / PopupMenu / Notification / StatusBar / Sidebar…） |
| 图标 | `gpui-kit-assets`（内置完整 Lucide 图标集，`AllAssets`） |
| 化学解析 | `rswfn`（路径依赖，含安全防护封装） |
| 异步 / 并发 | GPUI 的 `cx.spawn` + 后台执行器，`rayon` 并行解析 |

无需 Node.js / WebView2；Windows 上通过 GPU（wgpu / DirectX）渲染。

## 功能特性

### 常规文件管理（仿 Windows Explorer）
- 目录浏览 / 后退 / 前进 / 向上 / 刷新
- 面包屑地址栏（点击跳转，双击可编辑路径，回车确认）
- 左侧导航栏：快速访问 + 本地磁盘
- 名称 / 类型 / 大小 / 创建日期 / 修改日期列，列头排序（文件夹始终置顶）
- 复选框多选、「全选」表头复选框
- 新建文件夹、新建文件、重命名、复制、剪切、粘贴、删除（默认进回收站）
- 双击打开文件（系统默认程序）/ 打开文件夹
- 右键上下文菜单、键盘快捷键（`F2` 重命名、`Delete` 删除、`Backspace` 上级、
  `Ctrl+C/X/V` 复制/剪切/粘贴、`Ctrl+N` 新建文件、`Ctrl+Shift+N` 新建文件夹、
  `Alt+←/→` 后退/前进、`F5` 刷新）

### 化学文件专属功能
通过 `rswfn` 快速读取常见化学文件，解析后把分子信息**以表头列**显示：

| 列 | 说明 |
| --- | --- |
| 化学式 | 如 `H2O`、`C2H5OH`（已规范化为标准写法） |
| 原子数 | 分子中原子的数量 |
| 电荷 | 总电荷（`charge`） |
| 多重度 | 自旋多重度（`multip`） |
| 基组函数 | 原子轨道 / 基函数数量 |
| 分子量 | 由原子量累加得到（g/mol） |
| 能量 (a.u.) | 从输出文件尽力提取的总能量（Hartree，如 `SCF Done` / `FINAL SINGLE POINT ENERGY`） |

支持的文件类型：

- **Gaussian**：`.gjf` / `.com`（输入），`.log`（输出），`.fch` / `.fchk`（检查点）
- **Orca**：`.inp`（输入），`.out`（输出）
- **结构文件**：`.xyz`、`.mol`、`.sdf`
- **Molden**：`.molden` / `.mde`
- **其他**：`.dmo` / `.dmi`、`.cub`

选中化学文件后，右侧「预览」窗格会展示更完整的分子信息
（含 α/β 电子数，可通过「预览」按钮开关，默认开启）。

## 类型筛选与设置

- **文件类型筛选**：命令栏下方是一排**文件类型按钮**（全部文件 / 高斯输入 / 高斯日志 /
  量化输出 / 检查点 / XYZ / MOL / SDF / Molden / DMol / Orca 输入 / 格点），
  每个按钮带颜色圆点与当前目录内的数量。点击某个类型后，列表只显示该类型文件，
  并自动切换到该类型专属的「有限表头」列。
- **设置**：命令栏右侧的「设置」按钮打开设置窗口，可**针对每种文件类型勾选**
  要显示哪些数据列，并支持「恢复默认」。设置保存在
  `%APPDATA%\wmfold\settings.json`（非 Windows 用 `$HOME`）。

## 安全防护说明

`rswfn` 的几个读取器（如 `GjfReader`、`LogReader`）在遇到不完整的文件时
可能调用 `std::process::exit` 或 `panic!`。为避免拖垮整个资源管理器，
后端做了**三层防护**（详见 `src/chem.rs`）：

1. **预检**：对易触发 `exit` 的格式（gjf、log）先检查必要标记（电荷/多重度行、几何结构行）。
2. **`catch_unwind`**：所有解析都包在 `catch_unwind` 中，拦截任何 panic。
3. **结果降级**：解析失败时对应元数据列为「—」，绝不影响浏览其它文件。

> ⚠️ 因此请**不要**在别处直接调用 `rswfn::base::Mole::from_file`，而应统一通过
> `chem::parse_chem` / `chem::read_atoms_basis` 入口解析，否则可能拖垮应用。

## 项目结构

```
wmfold/
├── Cargo.toml                    # 应用 crate（package = wmfold，binary = wmfold）
├── src/
│   ├── main.rs                   # 入口：gpui-kit 应用、资源、窗口、全局快捷键
│   ├── app.rs                    # 主界面（工具栏 / 命令栏 / 筛选栏 / 侧边栏 / 表格 / 预览 / 状态栏）
│   ├── table.rs                  # 文件列表 DataTable delegate（列/行/单元格/右键菜单）
│   ├── settings_view.rs          # 设置对话框内容（每种类型显示哪些化学列）
│   ├── chem.rs                   # 化学文件解析（复用 rswfn，含安全防护）
│   ├── fs_ops.rs                 # 目录列表 / 文件操作 / 磁盘与快速访问
│   ├── models.rs                 # 数据结构（FileEntry / ChemMeta / DriveInfo / QuickAccess）
│   ├── chem_columns.rs           # 化学列定义与各类型可用列
│   ├── format.rs                 # 大小/日期格式化、图标、配色、自然排序
│   ├── paths.rs                  # Windows 路径、面包屑、父子路径
│   └── settings.rs               # 用户设置持久化
├── scripts/                      # 版本号递增 / 带版本号产物改名
├── legacy/                       # 迁移前的 Tauri + Vue 实现（不参与构建）
└── AGENTS.md
```

## 开发运行

前置条件：Rust 工具链（stable，Windows MSVC target）。

> **注意**：后端以路径依赖方式引用 `rswfn`（`Cargo.toml` 中
> `rswfn = { path = "../rswfn" }`），因此项目需放在 `E:\code\wmfold` 目录下，
> 且 `E:\code\rswfn` 为其自研波函数分析库。若移动工程，请同步修改该路径。
>
> `rswfn` 现已改用纯 Rust 的 `faer` 做线性代数，**不再依赖 Intel MKL**，因此整个
> 项目构建时无需系统级 BLAS / LAPACK / MKL，编译更轻量。

```bash
# 运行（开发模式；可直接 cargo run，也可 npm run dev）
cargo run

# 运行单元测试（化学解析 / 目录列表 / 路径工具）
cargo test

# 编译 release 产物（target/release/wmfold.exe）
cargo build --release
```

## 带版本号构建

```bash
npm run build:app
# 1) 补丁号（版本号最后一个数）+1，写回 Cargo.toml（并同步 package.json）
# 2) cargo build --release
# 3) 把 target/release/wmfold.exe 复制为 target/release/WMFold_<version>.exe
```

其它脚本：

```bash
npm run bump:version   # 仅递增补丁号
npm run rename:exe     # 仅生成带版本号的 exe 副本
npm run build          # 仅 cargo build --release（不递增版本）
```
