# wmfold · 化学文件资源管理器

一个仿照 Windows 自带文件资源管理器（Explorer）构建的桌面应用，
专为**化学计算 / 波函数分析**场景设计。它兼容普通文件资源管理器的全部功能，
并能快速解析常见的化学文件，把分子信息以「表头列」的形式展示出来。

底层基于 [Tauri 2](https://tauri.app/)（Rust 后端）+ **Vue 3 + TypeScript + Element Plus**（前端），
化学文件的解析复用你自研的波函数分析库 **`rswfn`**（`E:\code\rswfn`）。

项目提供一个合并的可执行程序 **`wmfold`**，桌面端与 `-cli` 机器接口共用同一套核心逻辑
[wmfold-core/](./wmfold-core)（化学解析 / 目录列表 / 文件操作 / 数据模型）：

- `wmfold`：启动 **Windows 桌面端**（Tauri 2 + Vue 3，本 README 主线）；
- `wmfold -cli <命令>`：**机器接口**，输出 JSON（脚本调用 / 远程服务器上的文件信息查询）。

机器接口的源码在 [wmfold-cli/](./wmfold-cli)（说明见 [wmfold-cli/README.md](./wmfold-cli/README.md)），同时保留一个独立 `wmfold-cli` 可执行文件，
方便在无法构建 Tauri 的服务器 / Linux 上单独编译（不拉 WebView / Node 依赖）。它只输出
JSON，不进入任何交互界面，桌面端的远程模式与脚本共用同一套协议。

> 实现历史：0.3.x 期间曾短暂迁移到纯 Rust / GPUI Kit（不依赖 WebView），
> 该实现已归档在 [legacy-gpui/](./legacy-gpui)，不参与当前构建。
>
> 面向 AI 编码 Agent 的仓库说明见 [AGENTS.md](./AGENTS.md)。

## 功能特性

### 常规文件管理（仿 Windows Explorer）
- **自定义标题栏**（无原生边框）：左侧应用名 `wmfold` 后显示当前版本号（如 `v0.3.23`）、机器 tab 和「+」按钮，右侧是设置与最小化 / 最大化 / 关闭按钮；检测到新版本时会在设置按钮左侧出现「升级」按钮，点击可直接下载安装并重启；标题栏空白处可拖动、双击最大化，窗口边缘可缩放
- **资源管理器右键集成**：在 Windows 文件夹空白处右键 →「通过 wmfold 打开」，会打开 wmfold 并定位到该目录；wmfold 已在运行时复用窗口并跳转（Windows 11 经典菜单可能位于「显示更多选项」下）
- **机器 tab（在标题栏里）**：主窗口默认只打开「本机」；标题栏的「+」按钮列出已保存的远程机器，点击即可打开一个新 tab。**每台机器都有自己独立的路径 / 历史 / 筛选 / 选择 / 预览**；切换机器会恢复它上次停留的状态，已连接过的机器复用缓存连接，不再重复检查/安装
- **多窗口**：机器 tab 可以**拖动出来**成为独立窗口，也可以拖到另一个窗口上合并；双击 tab 快速拆分/合并，右键 tab 也有「在新窗口打开 / 合并到主窗口」；独立窗口关闭时机器 tab 自动回到主窗口
- **横向快速访问**：像浏览器收藏夹一样排标题栏下方，按当前机器切换：
  - Windows 下默认只有「此电脑」和「用户目录」；Linux 下默认只有「用户目录」；
  - 连接到远程机器后，快速访问会切换成该机器的「用户目录」与它自己的固定项；
  - 右键文件夹「添加到快速访问」对本机和远程机器都可用（远程固定项按机器分别保存，重启后仍在；悬停 × 可移除）；
  - 目录被删除 / 移动后，该固定项会显示为灰色「不可用」，点击会提示，仍可悬停 × 删除。
- **图标按钮**：工具栏 / 命令栏只显示图标，名称在鼠标悬停一段时间后以提示显示（不再在图标下方显示文字）
- **可拖动分隔条**：文件列表与右侧预览窗格之间可以拖动调整宽度（宽度会记住）
- **右侧作业查询**：远程机器可在设置中启用「作业查询」，右侧面板会多出「作业查询」页签，执行配置的命令（默认 `squeue -u $USER`）并按设定间隔自动刷新；squeue 这类空白分隔输出会自动渲染成表格
- **提交作业**：在任意机器（本机或远程）的文件上右键 →「提交作业」，选择一台配置了提交命令的机器；目标机器会接收文件并执行命令，`$PATH` 会替换为远端文件路径。每台机器可设置**默认提交路径**（默认 `~/wmfold`）；当目标机器当前正在浏览某个文件夹时直接提交到该文件夹，否则提交到默认路径。还可开启「清除 chk 路径」，提交 gjf 时把 `%chk=/path/name.chk` 改成 `%chk=name.chk`
- **三维分子预览**：选中化学文件后，右侧预览面板**最上方**用 **three.js** 显示球棍模型，使用 TrackballControls（左键旋转 / 滚轮缩放 / 右键平移）；三维区域高度可拖动调整并记住，面板宽度变化会自动重算相机/画布。本机与远程机器都支持，远程原子坐标由服务器端 `wmfold -cli structure` 解析后传回
- **gjf / log 作业信息**：预览面板会显示从 Gaussian route 段提取的作业关键词（如 `p b3lyp/6-31g(d) opt freq`），即使没有开启化学列也能看到；文件列表切到「高斯输入 / 高斯日志」类型时也会多出一列「作业」
- **log 正常结束**：使用 rswfn 的 `read_normalend` 判断日志最后一行是否为 `Normal termination`；「高斯日志」类型新增「正常结束」列，预览面板也会显示「正常结束 / 未正常结束」
- **在系统终端中打开**：命令栏终端图标、右键菜单「在终端中打开」或 `Ctrl+`` 会用电脑上的终端打开当前文件夹；终端工具可在 设置 → 程序设置 中选择（自动 / Windows Terminal / PowerShell / cmd）。远程机器会新开一个 `ssh` 窗口并自动 `cd` 到远端目录（密码登录的机器在窗口里输入密码即可）。应用本身不再内置终端面板
- **文件选择**：单击一行只高亮当前行，不勾选左侧复选框；按住 `Shift` 再点击会从锚点行连选到目标行，并勾选这些行的复选框（再次单击可回到单选）。右键菜单会自动调整位置，保证在窗口四边都能完整显示；右键文件夹或文件列表空白处有「在文件资源管理器中显示」，用 Windows 资源管理器打开该文件夹或当前文件夹（仅本机）。
- **自动更新**：设置 → 程序设置 → 软件更新 可检查并安装新版本（Tauri 2 updater，签名校验）。更新文件发布在 `/home/fly/share/programs/wmfold/windows`，通过 `latest-version-info.json` 下发版本清单
- 目录浏览 / 后退 / 前进 / 向上 / 刷新
- 面包屑地址栏（点击跳转，双击可编辑路径）
- 名称 / 类型 / 大小 / 创建日期 / 修改日期列，列头排序（文件夹始终置顶）
- 新建文件夹、新建文件、重命名、复制、剪切、粘贴、删除（默认进回收站）；**复制 / 剪切 / 粘贴支持跨设备**：本机 ↔ 远程机器、远程 A ↔ 远程 B 都能直接传文件或整个目录
- 双击打开文件（系统默认程序）/ 打开文件夹
- 右键上下文菜单、键盘快捷键（`F2` 重命名、`Delete` 删除、`Backspace` 上级、`F5` 刷新、
  `Ctrl+C/X/V` 复制/剪切/粘贴、`Ctrl+N` 新建文件、`Ctrl+Shift+N` 新建文件夹、
  `Alt+←/→` 后退/前进）

### 扫描与 AI 对话

- **扫描**：命令栏的「扫描」按钮会把当前文件夹及**所有子文件夹**中的文件一次性读出来，平铺在主列表里；上方进度条实时显示已扫描的文件夹 / 文件数量、当前路径和用时，可随时「取消」。扫描视图多出「所在文件夹」列，搜索与类型筛选作用于全部已扫描文件。文件列表使用 Element Plus 的 **TableV2 虚拟滚动**，只渲染视口内的二十来行，8,000+ 文件也可以直接滚动，不再需要分页或「加载更多」；达到 10 万条读取上限会提示。扫描目前仅支持本机文件夹；点进度条右侧「退出扫描」或导航到其他目录会回到普通列表，`F5` 会重新扫描。
- **化学列自动后台解析**：进入文件夹或完成扫描后，会在后台自动解析**全部**化学文件；当前视图中的可见行会插队优先解析，首屏通常约 1 秒填充。后台每批 6 个文件、批间让出事件循环，不会阻塞滚动和点击。解析结果保存在 `%APPDATA%\wmfold\chem_cache.json`，文件大小/修改时间没变且解析格式版本一致时直接复用；状态栏会显示「解析化学信息 已完成/总数」和进度条。尚未解析完的单元格显示「—」，并不代表文件没有化学数据；只有日志缺少 `Input orientation` / `Standard orientation` 或解析失败时才会一直显示「—」。
- **AI 对话（DeepSeek）**：右侧面板新增「AI 对话」页签（也可点命令栏的 AI 按钮打开）。AI 会根据需求调用只读工具（列目录 / 读文件 / 搜索文件名或内容 / 查看化学信息）来**实际读取当前文件夹中的文件**，再回答问题；默认只读取当前文件夹第一层，只有开启扫描后才允许读取子文件夹。工具调用过程会显示在回答上方，可看到读取了哪些文件。工具返回的文件和文件夹会显示为可点击结果，回答正文里的相对路径（如 `sub/a.gjf`、`sub/`）也会自动变成可点击链接：扫描模式下保留扫描列表并选中/预览文件（点文件夹则把扫描列表限定到该子目录），普通模式下文件会跳到父目录并选中，文件夹会直接进入。点文件夹导致列表被限定后，扫描按钮后面会出现「取消过滤」按钮，一键恢复完整列表。
  - API Key 优先使用 设置 → AI 对话 中保存的 Key（Windows 下用 DPAPI 加密保存在 `%APPDATA%\wmfold\ai.json`），留空则读取环境变量 `DEEPSEEK_APIKEY`；两者都没有时会在发送前提示去配置。
  - 支持填写模型（默认 `deepseek-flash`，可选 `deepseek-v4-pro`）与 OpenAI 兼容的 Base URL（默认 `https://api.deepseek.com`），设置页可「测试连接」。
  - 可单独开启「思考模式」并选择强度（`low` / `high` / `max`），对应官网的 `thinking` 与 `reasoning_effort` 参数；默认关闭以保持响应速度。
  - 所有工具都被限制在当前文件夹及其子文件夹内，越界路径会被拒绝；大文件只读取前 96 KB（可让模型指定 `maxBytes`，上限 256 KB）。AI 对话暂仅支持本机文件。

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

选中化学文件后，右侧「预览」页签会展示更完整的分子信息（可通过「预览」按钮开关，默认开启）；启用了作业查询的远程机器，右侧还会出现「作业查询」页签。

## 类型筛选与设置

- **文件类型筛选**：命令栏下方是一排**横向文件类型按钮**（全部文件 / 高斯输入 / 高斯日志 / 量化输出 / 检查点 / XYZ / MOL / SDF / Molden / DMol / Orca 输入 / 格点）。点击某个类型，列表只显示该类型文件，并自动切换到该类型专属的「有限表头」列。
- **设置**：命令栏右侧的「设置」按钮打开设置窗口，包含四个页签：
  - **展示信息**：针对每种文件类型勾选要显示的化学列，支持「恢复默认」；
  - **程序设置**：是否**显示隐藏文件**（Windows 隐藏属性 / Linux 点文件；关闭时列表与类型计数都会隐藏它们）；选择**外部终端工具**（自动 / Windows Terminal / PowerShell / cmd）；**软件更新**（检查 / 下载并安装 / 自动重启）；
  - **AI 对话**：填写 DeepSeek API Key（留空则用 `DEEPSEEK_APIKEY` 环境变量）、模型、思考模式/强度与 Base URL，可测试连接；Key 用 Windows DPAPI 加密保存在 `%APPDATA%\wmfold\ai.json`；
  - **远程机器**：添加 / 编辑 / 删除 SSH 远程服务器；可配置密码、是否启用作业查询、查询命令（如 `squeue -u $USER`）和刷新间隔；还可配置**提交作业命令**（必须包含 `$PATH`）、**默认提交路径**（默认 `~/wmfold`）以及提交 gjf 时是否**清除 chk 路径**。
  设置保存在 `%APPDATA%\wmfold\settings.json`（非 Windows 用 `$HOME`），与 0.3.4 的 GPUI 版共用同一份配置（`show_hidden` / `terminal_tool` 对旧配置向后兼容）。

## 机器接口与远程服务器

### `wmfold -cli`（JSON）

非交互模式，输出稳定、机器可读的 JSON（协议版本 1），供桌面端远程模式与脚本共用：

```bash
wmfold -cli version
wmfold -cli list /home/user/calc --chem
wmfold -cli info /home/user/calc/water.xyz --chem
```

- `list [PATH] [--chem]`：返回 `{"protocol":1,"path":"…","entries":[…]}`，`entries` 与桌面端
  文件表条目一致（`name/path/isDir/size/created/modified/extension/kind/isChem/isHidden/isSymlink/chem`）；
  路径统一返回绝对路径，不加 `--chem` 时不解析化学元数据（列目录更快）。
- `info <PATH> [--chem]`：返回 `{"protocol":1,"entry":{…}}`，单个文件 / 目录的条目信息。
- `version`：返回 `{"protocol":1,"name":"wmfold-cli","version":"…"}`。

桌面端远程模式调用的就是这套接口：先通过 SSH 把随包的服务器版二进制放到远端
`~/.local/bin/wmfold`，之后用 `wmfold -cli list/info` 取回 JSON。

独立编译：服务器 / Linux 上可以直接

```bash
cd wmfold-cli && cargo build --release     # 产物 wmfold-cli，参数与 wmfold -cli 完全一致
```

Windows 上交叉编译随包的 Linux x86_64-musl 静态版本（即 `src-tauri/resources/wmfold-server`）：

```bash
npm run build:server                       # 需先 rustup target add x86_64-unknown-linux-musl
```

协议版本定义在 `wmfold-cli/src/cli.rs` 的 `PROTOCOL`；字段有不兼容变化时必须递增版本，
并同步桌面端 `src-tauri/src/remotes.rs` 里的 `PROTOCOL` 与结构体，两端不匹配时桌面端会提示重新安装。

### 远程服务器（SSH）

1. 打开 **设置 → 远程机器**，添加机器（名称 / 主机 / 端口 / 用户名 / 可选私钥路径 / 可选密码）；
2. 点标题栏机器 tab 右侧的 **「+」按钮**，从已保存的机器里选择要打开的机器（主窗口默认只有「本机」）。首次选择时会通过系统 `ssh`
   检查环境，若服务器上没有 `wmfold`，自动把随包的 **Linux x86_64（musl 静态）** 二进制
   上传到 `~/.local/bin/wmfold` 并 `chmod 755`；此时快速访问切换为该机器的「用户目录」
   （远程右键添加的固定目录也会出现在这里）；
3. 切换后主文件列表直接浏览该机器：路径为 POSIX、表格显示远端条目，支持排序 / 搜索 /
   类型筛选 / 化学列 / 预览；数据来自远端 `wmfold -cli list <path> [--chem]` 返回的 JSON。

说明：

- 远程模式下**新建 / 删除 / 重命名暂不可用**，避免误操作；但**复制 / 剪切 / 粘贴可用**，
  并且支持跨设备传输：本地 → 远程、远程 → 本地、远程 A → 远程 B（文件与整个目录都支持），
  剪切在传输成功后会删除源路径；
  切回「本机」后恢复。
- 认证支持两种方式：
  - **密钥**（不填密码）：使用系统 OpenSSH 与用户已有的密钥（ssh-agent / `~/.ssh/id_*` / `~/.ssh/config`），`BatchMode=yes`；
  - **密码**：填写密码后使用随包的 PuTTY `plink` 连接（Windows 自带 OpenSSH 无法非交互输入密码）。
    密码用 Windows DPAPI 加密后保存在本机 `remotes.json`，连接时写入临时 `-pwfile` 并在命令结束后删除；
    首次连接会自动探测并缓存服务器主机密钥指纹（`-hostkey`），之后不再询问。
- 首次密钥连接自动接受 host key（`StrictHostKeyChecking=accept-new`）。
- 随包二进制位于 `src-tauri/resources/wmfold-server`；更新命令：`npm run build:server`
  （需 `rustup target add x86_64-unknown-linux-musl`，纯 Rust 静态链接，无需 Linux 工具链）。
  PuTTY `plink.exe` 位于 `src-tauri/resources/plink.exe`（MIT 许可，见同目录
  `PUTTY-LICENCE.txt`）。
- 也可以直接登录服务器构建：`cd wmfold-cli && cargo build --release`，把产物
  `target/release/wmfold-cli` 放到 `~/.local/bin/wmfold` 即可被桌面端识别。
- 协议版本见 `wmfold-cli/src/cli.rs` 的 `PROTOCOL`，两端不匹配时桌面端会提示重新安装。

## 安全防护说明

`rswfn` 的几个读取器（如 `GjfReader`、`LogReader`）在遇到不完整的文件时
可能调用 `std::process::exit` 或 `panic!`。为避免拖垮整个资源管理器，
后端做了**三层防护**（详见 `wmfold-core/src/chem.rs`）：

1. **预检**：对易触发 `exit` 的格式（gjf、log）先检查必要标记（电荷/多重度行、几何结构行）。
2. **`catch_unwind`**：所有解析都包在 `catch_unwind` 中，拦截任何 panic。
3. **结果降级**：解析失败时对应元数据列为「—」，绝不影响浏览其它文件。

> ⚠️ 因此请**不要**在别处直接调用 `rswfn::base::Mole::from_file`，而应统一通过
> `chem::parse_chem` / `chem::read_atoms_basis` 入口解析，否则可能拖垮应用。

## 项目结构

```
wmfold/
├── wmfold-core/                  # 共享核心（无 GUI 依赖，桌面版与 -cli 共用）
│   ├── src/chem.rs               # 化学文件解析（复用 rswfn，含安全防护）
│   ├── src/fs.rs                 # 目录列表 / 文件操作
│   ├── src/models.rs             # 数据模型（serde camelCase）
│   ├── src/columns.rs            # 化学列定义与各类型可用列
│   └── src/lib.rs
├── src-tauri/                    # 合并启动器 + 桌面端（Tauri 2）
│   ├── src/main.rs               # wmfold 入口：默认开窗口，`-cli` 走机器接口
│   ├── src/commands.rs           # Tauri 命令（转发给 wmfold-core）
│   ├── src/remotes.rs            # 远程服务器：SSH 检查 / 安装 / 调用远端 wmfold -cli
│   ├── src/quick_access.rs       # 快速访问固定项（quick_access.json）
│   ├── src/settings.rs           # 用户设置持久化（settings.json）
│   ├── resources/wmfold-server   # 随包的 Linux x86_64 静态二进制（远程安装用）
│   └── src/lib.rs                # 命令注册（generate_handler）
├── src/                          # Vue 前端
│   ├── api/tauri.ts              # 后端命令的 TS 封装（invoke 封装）
│   ├── components/               # 资源管理器组件（工具栏/命令栏/表格/预览/设置/三维结构查看器…）
│   ├── composables/              # 右键菜单；新建/删除/重命名（快捷键与按钮共用）
│   ├── stores/explorer.ts        # Pinia 状态（路径历史/排序/剪切板/化学元数据/列设置）
│   ├── styles/windows.css        # Windows 11 风格样式
│   ├── types/index.ts            # 与 Rust 对齐的类型
│   ├── utils/                    # 格式化 / 路径工具
│   ├── App.vue                   # 布局 + 全局快捷键 + 错误提示条
│   └── main.ts                   # 挂载 Vue / Pinia / Element Plus / 图标全局注册
├── wmfold-cli/                   # 机器接口（JSON）+ 独立 bin（服务器 / Linux）
│   ├── src/lib.rs                # 入口 cli_main / cli_main_named（剥离 -cli、help/version、静默 panic）
│   ├── src/cli.rs                # 协议实现：list / info / version → JSON（PROTOCOL = 1）
│   └── src/main.rs               # 独立 wmfold-cli bin（只依赖 core + serde）
├── scripts/                      # bump-version / rename-exe / build-server（交叉编译服务器版）
├── legacy-gpui/                  # 归档：纯 Rust / GPUI Kit 实现（不参与构建）
└── AGENTS.md                     # 面向 AI 编码 Agent 的仓库说明
```

## 开发运行

前置条件：Node.js ≥ 18、Rust 工具链（含 `cargo-tauri`）、[WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（Windows 一般已自带）。

> **注意**：共享核心以路径依赖方式引用 `rswfn`（`wmfold-core/Cargo.toml` 中
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

# 运行共享核心单测（化学解析 / 目录列表 / 文件操作）
cd wmfold-core && cargo test

# 运行 Tauri 后端单测（设置 / 快速访问）
cd src-tauri && cargo test --lib

# 仅编译 Rust 产物（不打包，产 target/release/wmfold.exe）
cd src-tauri && cargo build --release

# 合并启动器：桌面端 / 机器接口
./src-tauri/target/release/wmfold.exe
./src-tauri/target/release/wmfold.exe -cli list . --chem

# 单独编译 / 测试机器接口（不拉 Tauri / WebView 依赖）
npm run test:cli
cd wmfold-cli && cargo build --release

# 交叉编译随包的服务器二进制并同步到 src-tauri/resources/
npm run build:server
```

## 构建安装包

```bash
# 推荐：每次编译自动把次版本号 +1，并生成带版本号的程序/安装包
npm run build:app

# 或仅打包（不递增版本号；不会生成 updater 签名）
npm run tauri build
```

> 版本号机制：`npm run build:app` 会先在 `package.json` / `Cargo.toml` / `tauri.conf.json`
> 三处同步把**版本号最后一个数 +1**（补丁号，如 `0.1.0 → 0.1.1`），再打包，最后把 `wmfold.exe`
> 复制为带版本号的 `wmfold_<version>.exe`。安装包文件名本身也带版本号。
>
> `wmfold-cli` 的版本独立（`wmfold-cli/Cargo.toml`，当前 `0.2.2`），不参与 `build:app`
> 的补丁号递增；`wmfold-core` 跟随桌面版版本号。

产物位置（`<version>` 为当前版本号）：

| 产物 | 路径 |
| --- | --- |
| 独立可执行文件（带版本号） | `src-tauri/target/release/wmfold_<version>.exe` |
| MSI 安装包 | `src-tauri/target/release/bundle/msi/wmfold_<version>_x64_en-US.msi` |
| NSIS 安装程序 | `src-tauri/target/release/bundle/nsis/wmfold_<version>_x64-setup.exe` |
| 更新签名 | `src-tauri/target/release/bundle/nsis/wmfold_<version>_x64-setup.exe.sig` |

> 相关 Tauri 配置：`src-tauri/tauri.conf.json`（产品名 **wmfold**、主二进制名 **wmfold**、
> 标识 `com.wmfold.explorer`、主窗口 1200×780、bundle 目标 `all`、
> `createUpdaterArtifacts: true`、updater 公钥与清单地址）。

## 自动更新（Tauri 2 updater）

桌面端通过 Tauri 2 官方 updater 检查更新，清单地址：

```
https://www.xiaofei911.top/statics/wmfold-updates/windows/latest-version-info.json
```

服务器目录（Caddy 通过 `/home/fly/statics/wmfold-updates` 符号链接公开）：

```
/home/fly/share/programs/wmfold/windows/
├── latest-version-info.json
├── 0.3.12/
│   ├── wmfold_0.3.12_x64-setup.exe
│   └── wmfold_0.3.12_x64-setup.exe.sig
└── ...
```

发布流程：

```bash
# 1) 带签名的构建（读取 .tauri/wmfold.key，自动递增补丁号）
npm run build:app

# 2) 上传当前版本 + 刷新 latest-version-info.json
npm run publish:update -- "本次更新说明"
```

- 私钥 `.tauri/wmfold.key` 不进入 git（`.gitignore`）；公钥写在 `tauri.conf.json` 的
  `plugins.updater.pubkey`。换机器构建时需要把该私钥一起带过去，否则无法生成可验证的更新。
- `build-app.mjs` 通过 `TAURI_SIGNING_PRIVATE_KEY` 调用 Tauri CLI，生成
  `-setup.exe.sig`；`publish-update.mjs` 把安装包和签名放入版本文件夹，并生成
  `latest-version-info.json`（`windows-x86_64` 平台）。
- 自动更新从 **0.3.12** 起引入；**0.3.14** 修复了 Vue 代理破坏 `Update` 私有字段导致安装失败的问题。
  更早版本需要手动安装一次 0.3.34 或更新版本。当前发布版本为 **0.3.47**（三维预览 + gjf/log 作业信息与正常结束标识、作业列 + 标题栏升级入口 + 资源管理器右键打开 + 提交作业 + 扫描 + DeepSeek AI 对话 + 虚拟滚动文件列表 + AI Markdown 表格）。
