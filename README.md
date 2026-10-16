# WMFold · 化学文件资源管理器

一个仿照 Windows 自带文件资源管理器（Explorer）构建的桌面应用，
专为**化学计算 / 波函数分析**场景设计。它兼容普通文件资源管理器的全部功能，
并能快速解析常见的化学文件，把分子信息以「表头列」的形式展示出来。

底层基于 [Tauri 2](https://tauri.app/)（Rust 后端）+ **Vue 3 + TypeScript + Element Plus**（前端），
化学文件的解析复用你自研的波函数分析库 **`rswfn`**（`E:\code\rswfn`）。

> 面向 AI 编码 Agent 的仓库说明见 [AGENTS.md](./AGENTS.md)。

## 功能特性

### 常规文件管理（仿 Windows Explorer）
- 目录浏览 / 后退 / 前进 / 向上 / 刷新
- 面包屑地址栏（点击跳转，双击可编辑路径）
- 左侧导航栏：快速访问 + 本地磁盘
- 名称 / 类型 / 大小 / 创建日期 / 修改日期列，列头排序（文件夹始终置顶）
- 新建文件夹、新建文件、重命名、复制、剪切、粘贴、删除（默认进回收站）
- 双击打开文件（系统默认程序）/ 打开文件夹
- 右键上下文菜单、键盘快捷键（`F2` 重命名、`Delete` 删除、`Backspace` 上级、
  `Ctrl+C/X/V` 复制/剪切/粘贴、`Alt+←/→` 后退/前进）

### 化学文件专属功能
通过 `rswfn` 快速读取常见化学文件，解析后把分子信息**以表头列**显示：

| 列 | 说明 |
| --- | --- |
| 化学式 | 如 `H2O`、`C2H5OH`（已规范化为标准写法） |
| 原子数 | 分子中原子的数量 |
| 电荷 | 总电荷（`charge`） |
| 多重度 | 自旋多重度（`multip`） |
| α/β 电子 | α、β 电子数 |
| 基组函数 | 原子轨道 / 基函数数量 |
| 分子量 | 由原子量累加得到（g/mol） |
| 能量 (a.u.) | 从输出文件尽力提取的总能量（Hartree，如 `SCF Done` / `FINAL SINGLE POINT ENERGY`） |

支持的文件类型：

- **Gaussian**：`.gjf` / `.com`（输入），`.log`（输出），`.fch` / `.fchk`（检查点）
- **Orca**：`.inp`（输入），`.out`（输出）
- **结构文件**：`.xyz`、`.mol`、`.sdf`
- **Molden**：`.molden` / `.mde`
- **其他**：`.dmo` / `.dmi`、`.cub`

选中化学文件后，右侧「预览」窗格会展示更完整的分子信息（可通过「预览」按钮开关，默认开启）。

## 类型筛选与设置

- **文件类型筛选**：命令栏下方是一排**横向文件类型按钮**（全部文件 / 高斯输入 / 高斯日志 / 量化输出 / 检查点 / XYZ / MOL / SDF / Molden / DMol / Orca 输入 / 格点）。点击某个类型，列表只显示该类型文件，并自动切换到该类型专属的「有限表头」列。
- **设置**：命令栏右侧的「设置」按钮打开设置窗口，可**针对每种文件类型勾选**要显示哪些数据列（用于多选项），并支持「恢复默认」。

## 安全防护说明

`rswfn` 的几个读取器（如 `GjfReader`、`LogReader`）在遇到不完整的文件时
可能调用 `std::process::exit` 或 `panic!`。为避免拖垮整个资源管理器，
后端做了**三层防护**（详见 `src-tauri/src/chem.rs`）：

1. **预检**：对易触发 `exit` 的格式（gjf、log）先检查必要标记（电荷/多重度行、几何结构行）。
2. **`catch_unwind`**：所有解析都包在 `catch_unwind` 中，拦截任何 panic。
3. **结果降级**：解析失败时对应元数据列为「—」，绝不影响浏览其它文件。

> ⚠️ 因此请**不要**在别处直接调用 `rswfn::base::Mole::from_file`，而应统一通过
> `chem::parse_chem` / `chem::read_atoms_basis` 入口解析，否则可能拖垮应用。

## 项目结构

```
wmfold/
├── src/                          # Vue 前端
│   ├── api/tauri.ts              # 后端命令的 TS 封装（invoke 封装）
│   ├── components/               # 资源管理器组件（工具栏/命令栏/表格/侧边栏/状态栏…）
│   ├── composables/useContextMenu.ts  # 右键菜单共享状态
│   ├── stores/explorer.ts        # Pinia 状态（路径历史/排序/剪切板/化学元数据）
│   ├── styles/windows.css        # Windows 11 风格样式
│   ├── types/index.ts            # 与 Rust 对齐的类型
│   ├── utils/                    # 格式化 / 路径工具
│   ├── App.vue                   # 布局 + 全局快捷键
│   └── main.ts                   # 挂载 Vue / Pinia / Element Plus / 图标全局注册
├── src-tauri/                    # Rust 后端
│   ├── src/chem.rs               # 化学文件解析（复用 rswfn，含安全防护）
│   ├── src/fs_cmds.rs            # 目录列表 / 文件操作 / 磁盘与快速访问
│   ├── src/models.rs             # 序列化模型（serde camelCase）
│   ├── src/lib.rs                # 命令注册（generate_handler）
│   └── src/main.rs               # 入口，调用 wmfold_lib::run()
└── AGENTS.md                     # 面向 AI 编码 Agent 的仓库说明
```

## 开发运行

前置条件：Node.js ≥ 18、Rust 工具链（含 `cargo-tauri`）、[WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（Windows 一般已自带）。

> **注意**：后端以路径依赖方式引用 `rswfn`（`src-tauri/Cargo.toml` 中
> `rswfn = { path = "../../rswfn" }`），因此项目需放在 `E:\code\wmfold` 目录下，
> 且 `E:\code\rswfn` 为其自研波函数分析库。若移动工程，请同步修改该路径。
>
> `rswfn` 现已改用纯 Rust 的 `faer` 做线性代数，**不再依赖 Intel MKL**，因此整个
> 项目构建时无需系统级 BLAS / LAPACK / MKL，编译更轻量。

```bash
# 安装前端依赖
npm install

# 启动开发模式（自动拉起 vite + 编译 Rust + 打开窗口）
npm run tauri dev

# 触发一次前端类型检查 + 生产打包（vue-tsc + vite build）
npm run build

# 仅用 vite 起前端（不含 Rust 窗口，用于纯前端调试）
npm run dev

# 运行后端单元测试（化学解析 / 目录列表）
cd src-tauri && cargo test --lib

# 仅编译 Rust 产物（不打包，产 target/release/wmfold.exe）
cd src-tauri && cargo build --release
```

## 构建安装包

```bash
# 推荐：每次编译自动把次版本号 +1，并生成带版本号的程序/安装包
npm run build:app

# 或仅打包（不递增版本号）
npm run tauri build
```

> 版本号机制：`npm run build:app` 会先在 `package.json` / `Cargo.toml` / `tauri.conf.json`
> 三处同步把**版本号最后一个数 +1**（补丁号，如 `0.1.0 → 0.1.1`），再打包，最后把 `wmfold.exe`
> 复制为带版本号的 `WMFold_<version>.exe`。安装包文件名本身也带版本号。

产物位置（`<version>` 为当前版本号）：

| 产物 | 路径 |
| --- | --- |
| 独立可执行文件（带版本号） | `src-tauri/target/release/WMFold_<version>.exe` |
| MSI 安装包 | `src-tauri/target/release/bundle/msi/WMFold_<version>_x64_en-US.msi` |
| NSIS 安装程序 | `src-tauri/target/release/bundle/nsis/WMFold_<version>_x64-setup.exe` |

> 相关 Tauri 配置：`src-tauri/tauri.conf.json`（产品名 **WMFold**、标识 `com.wmfold.explorer`、
> 主窗口 1200×780、bundle 目标 `all`）。
