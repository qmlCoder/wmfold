# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, Codex, etc.) working
in this repository. Read it before making changes.

## Project Overview

**wmfold** is a Windows-style file explorer desktop app built for **chemistry /
wavefunction analysis**. It behaves like the built-in Windows Explorer but adds
specialized columns that show molecular information (formula, atom count, charge,
multiplicity, basis-set size, molecular weight, energy) parsed from common
chemistry files.

- **Merged launcher**: `wmfold` starts the desktop app; `wmfold -cli <cmd>` emits
  machine-readable JSON. Both live in one binary; dispatch happens in
  `src-tauri/src/main.rs` (the `-cli` branch forwards to `wmfold_cli::cli_main_named`).
- **Desktop app**: Tauri 2 (Rust backend) + Vue 3 + TypeScript + Element Plus + Pinia.
- **Machine interface**: `wmfold-cli/` — JSON-only CLI (`list` / `info` / `structure` / `version`,
  `PROTOCOL = 1`) with no interactive UI. It depends only on `wmfold-core`, `serde` and
  `serde_json`. It is built both as a library (embedded in `wmfold` as `wmfold -cli`)
  and as a standalone `wmfold-cli` bin for server / Linux builds; the desktop app calls
  it over SSH for remote browsing. See `wmfold-cli/README.md`.
- **Custom title bar / machine tabs**: the window uses `decorations: false`; `TitleBar.vue`
  draws the app name, the machine tabs (`MachineTabs.vue` is embedded in the title bar),
  the "+" menu that opens already-saved machines, the settings gear (left of minimize)
  and the window controls. Dragging is handled in JS (`appWindow.startDragging()` on
  mousedown over non-interactive blank areas) because the old 24px
  `data-tauri-drag-region` strip was unusable; double-clicking the blank title bar
  toggles maximize. There is no 「机器」 dropdown menu anymore — the main window starts
  with only the 本机 tab. Below the title bar `QuickAccessBar.vue` is the browser-style
  bookmarks row. There is no left sidebar anymore.
- **Per-machine state**: every machine has its own snapshot (path, history, selection,
  filter, search, preview) kept in `store.machineSnapshots`; switching tabs saves the
  current snapshot and restores the target one. `lastPaths` is only a fallback. There is
  no embedded terminal anymore — the terminal action opens the system terminal (see the
  external terminal section below).
- **Multi-window**: machine tabs are draggable; dragging one out (or double-click /
  right-click → 在新窗口打开) creates a detached window via the frontend
  `WebviewWindow` API, removes the tab from the current window, and stores
  `{machine,path}` in `localStorage['wmfold.window.<label>']` for the new window to read.
  Dropping a tab on another window / 合并到主窗口 emits `machine-attach`; closing a
  detached window makes the Rust side emit the same event so the tab returns to `main`.
  See `src-tauri/src/windows.rs` (`register_machine_window`, `window_at_point`,
  `merge_machine_window`).
- **Remote servers**: managed in **Settings → 远程机器** (tab) and opened from the title
  bar "+" menu (the main window starts with only the 本机 tab). The desktop app connects
  via the system OpenSSH client (key auth) or the bundled PuTTY `plink` (password auth),
  installs the bundled Linux `wmfold` binary (Tauri resource `wmfold-server`) into
  `~/.local/bin/wmfold`, then browses through `wmfold -cli list/info`. See
  `src-tauri/src/remotes.rs` and the remote state in `src/stores/explorer.ts`.
- **Shared core**: `wmfold-core/` — chemistry parsing, directory listing, file ops and
  models shared by the desktop app and the `-cli` interface. It must stay GUI-free and
  portable.
- **Chemistry parsing**: reuses the user's own wavefunction library **`rswfn`**
  (located at `E:\code\rswfn`, outside this repo) as a path dependency of `wmfold-core`.
- **Platform**: the desktop app is Windows-only in practice (`cargo-tauri`, MSVC
  toolchain, WebView2); `wmfold-cli` builds on Windows and Linux. Linear algebra inside
  `rswfn` uses the pure-Rust `faer` crate — **no MKL / BLAS / LAPACK** dependency.
- **Implementation history**: 0.3.x briefly shipped a pure-Rust **GPUI Kit** build
  (no WebView). It is archived in `legacy-gpui/` and is **not** part of the build;
  the active stack is the Tauri 2 project at the repository root.

## Commands

### Frontend
```bash
npm install            # install deps
npm run dev            # start only Vite dev server (port 1420), no Rust window
npm run build          # vue-tsc --noEmit && vite build  (typecheck + prod bundle)
npm run tauri dev      # full dev: starts Vite + compiles Rust + opens window
```

### Backend
```bash
cd src-tauri
cargo check                         # fast type-check
cargo build --release               # build target/release/wmfold.exe
cargo test --lib                    # settings / quick-access unit tests
cargo run -- -cli list . --chem     # debug build: JSON machine interface via merged launcher
```

### Shared core
```bash
cd wmfold-core
cargo test                          # chemistry / fs / columns unit tests
```

### Build with auto version bump (recommended)
```bash
npm run build:app
# 1) bumps the last version number (补丁号) +1
# 2) runs `npm run tauri build`
# 3) copies `target/release/wmfold.exe` to `target/release/wmfold_<version>.exe`
# outputs:
#   src-tauri/target/release/wmfold_<version>.exe      (带版本号的程序，内含 GUI + -cli)
#   src-tauri/target/release/bundle/msi/wmfold_<version>_x64_en-US.msi
#   src-tauri/target/release/bundle/nsis/wmfold_<version>_x64-setup.exe
```

### Machine interface (`-cli`)
```bash
# JSON out: {"protocol":1,"path":"...","entries":[...]}
wmfold -cli list <DIR> --chem
wmfold -cli info <FILE> --chem
wmfold -cli structure <FILE>
wmfold -cli version

# through the merged launcher (debug)
cargo run --manifest-path src-tauri/Cargo.toml -- -cli list <DIR> --chem

# standalone server build (no Tauri / WebView deps)
npm run test:cli                    # JSON protocol tests, from the repo root
cd wmfold-cli
cargo build --release               # bin `wmfold-cli`, same args as `wmfold -cli`
cargo test                          # same tests run from inside wmfold-cli/

# Cross-compile the bundled Linux server binary (static musl) and refresh the resource
npm run build:server                # rustup target add x86_64-unknown-linux-musl first
```

### Other build commands
```bash
npm run build          # 仅前端：vue-tsc --noEmit && vite build
npm run tauri build    # 打包（不自动递增版本；用 build:app 才会递增）
npm run bump:version   # 仅递增次版本号并同步 package.json / Cargo.toml / tauri.conf.json
```

## Versioning & named build outputs

- **每次编译，版本号最后一个数 +1**：运行 `npm run build:app` 时，`scripts/bump-version.mjs` 会把
  `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 三处的版本号一起递增
  （补丁号，即最后一个数 +1：如 `0.1.0 → 0.1.1`）。
- **生成的程序名包含版本号**：`scripts/rename-exe.mjs` 会把构建产物
  `target/release/wmfold.exe` 复制为 `target/release/wmfold_<version>.exe`；安装包文件名
  本身也带版本号（`wmfold_<version>_x64-setup.exe` 与 `..._x64_en-US.msi`）。
- **注意**：tauri CLI 在运行 `beforeBuildCommand` **之前**就读取版本号，因此不要把版本递增
  放进 `beforeBuildCommand`——否则捆绑包仍会用旧版本。版本递增必须在 `npm run tauri build`
  **之前**执行（`scripts/build-app.mjs` 已按此顺序编排）。
- `npm run build:app` 还负责加载 updater 私钥并生成 `-setup.exe.sig`；私钥默认在
  `.tauri/wmfold.key`（gitignored），缺失时会直接报错。发布更新用
  `npm run publish:update -- "更新说明"`。
- 三个版本声明文件必须保持一致（Cargo.toml 与 tauri.conf.json 不一致会导致打包失败/警告）。
- `wmfold-cli` 的版本独立（`wmfold-cli/Cargo.toml`，当前 `0.2.2`），不参与
  `build:app` 的补丁号递增；`wmfold-core` 跟随桌面版版本号。

## Architecture / Layout

```
src/                    # Vue frontend
  api/tauri.ts          # invoke() wrappers for every backend command
  components/           # Toolbar, CommandRibbon, FilterBar, FileList, StatusBar, ChemPanel, ContextMenu,
                        #   TitleBar (custom caption + machine tabs + "+" menu), MachineTabs, QuickAccessBar, SettingsDialog,
                        #   RightPanel (预览 / AI 对话 / 作业查询 tabs), AiChatPanel, ScanBar, JobPanel, MoleculeViewer
  composables/useContextMenu.ts  # global right-click menu state (reactive)
  composables/useFileActions.ts  # new folder/file, delete, rename — shared by CommandRibbon and keyboard shortcuts
  stores/explorer.ts    # Pinia store: path history, sorting, clipboard, chem metadata, per-type column settings,
                        #   recursive scan state, AI chat state
  styles/windows.css    # Windows 11 look; styling overrides for Element Plus
  types/index.ts        # TS types mirroring Rust Serialize structs
  utils/format.ts       # size/date formatting, icon + type labels, chem colors
  utils/path.ts         # Windows path helpers (split/join, THIS_PC virtual root)
  utils/markdown.ts     # safe minimal Markdown renderer for AI replies
  utils/chemColumns.ts  # chem column defs + per-kind available columns (shared by FileList/FilterBar/SettingsDialog)
  App.vue               # layout + global keyboard shortcuts + dismissible error bar
  main.ts               # mounts Vue + Pinia + ElementPlus; registers all icons globally

src-tauri/              # Rust backend (desktop shell) + merged launcher
  Cargo.toml            # crate `wmfold`, lib `wmfold_lib`, bin `wmfold`; depends on ../wmfold-core + ../wmfold-cli
  src/main.rs           # merged launcher: GUI by default; `-cli` -> wmfold_cli
                        #   (release = GUI subsystem; terminal modes attach a console)
  src/lib.rs            # tauri Builder + invoke_handler (registers commands)
  src/commands.rs       # #[tauri::command] adapters → wmfold-core; desktop-only
                        #   quick access merging / drives / opener
  src/scan.rs           # recursive scan command + `scan-progress` events (cancel by scan id)
  src/chem_cache.rs     # persistent chemistry metadata cache (%APPDATA%\wmfold\chem_cache.json)
  src/ai.rs             # DeepSeek chat + tool loop (list_dir/read_file/search_files/file_info)
  src/secret.rs         # DPAPI encrypt/decrypt for remote passwords and the AI API key
  src/remotes.rs        # SSH remotes: profiles, remote_check/install/list via wmfold -cli
  resources/wmfold-server  # bundled Linux x86_64-musl `wmfold` (installed on remotes)
  src/quick_access.rs   # pinned quick-access folders (%APPDATA%\wmfold\quick_access.json)
  src/settings.rs       # column settings persistence (%APPDATA%\wmfold\settings.json)
  tauri.conf.json       # window config, bundle config, devUrl http://localhost:1420

wmfold-core/            # shared core (no GUI deps; used by desktop + -cli)
  Cargo.toml            # crate `wmfold-core`; rswfn path ../../rswfn
  src/models.rs         # Serialize structs (#[serde(rename_all="camelCase")])
  src/chem.rs           # chemistry metadata via rswfn (SAFETY-CRITICAL — see below)
  src/fs.rs             # directory listing + file ops (create/rename/delete/copy/move)
  src/columns.rs        # chem column defs + per-kind available columns
  src/lib.rs            # re-exports

wmfold-cli/             # JSON machine interface (server build)
  Cargo.toml            # crate `wmfold-cli`, lib `wmfold_cli`, standalone bin `wmfold-cli`;
                        #   depends on ../wmfold-core + serde + serde_json
  src/lib.rs            # cli_main() / cli_main_named(): strips -cli, --help/--version,
                        #   routes to cli::run(); silent panic hook -> wmfold-cli-panics.log
  src/cli.rs            # `-cli` JSON protocol (list/info/version); PROTOCOL constant
  src/main.rs           # standalone bin (thin wrapper around wmfold_cli::cli_main)
```

## Tauri Commands

Registered in `src-tauri/src/lib.rs`; called from frontend via `src/api/tauri.ts`.

| Command | Rust signature | JS args |
| --- | --- | --- |
| `list_dir` | `(path: String)` | `{ path }` |
| `get_home_dir` | `() -> String` | `{}` |
| `get_quick_access` | `() -> Vec<QuickAccess>` | `{}` |
| `add_quick_access` | `(path: String) -> Result<bool, String>` | `{ path }` |
| `remove_quick_access` | `(path: String) -> Result<(), String>` | `{ path }` |
| `get_drives` | `() -> Vec<DriveInfo>` | `{}` |
| `create_folder` | `(path: String)` | `{ path }` |
| `create_file` | `(path: String)` | `{ path }` |
| `rename_path` | `(path, newName)` | `{ path, newName }` |
| `delete_paths` | `(paths: Vec<String>, permanent: bool)` | `{ paths, permanent }` |
| `copy_paths` | `(paths: Vec<String>, dest_dir: String)` | `{ paths, destDir }` |
| `move_paths` | `(paths: Vec<String>, dest_dir: String)` | `{ paths, destDir }` |
| `open_path` | `(app: AppHandle, path: String)` | `{ path }` |
| `open_in_explorer` | `(path: String)` | `{ path }` |
| `read_chem` | `(path: String) -> ChemMeta` | `{ path }` |
| `read_chem_batch` | `(paths: Vec<String>) -> Vec<ChemMeta>` | `{ paths }` |
| `flush_chem_cache` | `()` | `{}` |
| `read_structure` | `(path: String) -> MolStructure` | `{ path }` |
| `scan_recursive` | `(app, path, scan_id: u64, include_hidden, max_entries: Option<usize>) -> ScanResult` | `{ path, scanId, includeHidden, maxEntries }` |
| `cancel_scan` | `(scan_id: u64)` | `{ scanId }` |
| `ai_config` | `() -> AiConfigView` | `{}` |
| `ai_save_config` | `(api_key, model, base_url, thinking: Option<bool>, reasoning_effort: Option<String>) -> AiConfigView` | `{ apiKey, model, baseUrl, thinking, reasoningEffort }` |
| `ai_test` | `() -> Result<String, String>` | `{}` |
| `ai_chat` | `(app, request_id: u64, root, messages, selected, recursive) -> AiReply` | `{ requestId, root, messages, selected, recursive }` |
| `remote_structure` | `(profile, path) -> MolStructure` | `{ profile, path }` |
| `load_settings` | `() -> Settings` | `{}` |
| `save_settings` | `(settings: Settings)` | `{ settings }` |
| `remote_profiles` | `() -> Vec<RemoteProfile>` | `{}` |
| `remote_save_profile` | `(profile: RemoteProfile)` | `{ profile }` |
| `remote_delete_profile` | `(id: String)` | `{ id }` |
| `remote_check` | `(profile: RemoteProfile) -> RemoteStatus` | `{ profile }` |
| `remote_install` | `(app, profile) -> String` | `{ profile }` |
| `remote_list` | `(profile, path, chem) -> RemoteListing` | `{ profile, path, chem }` |
| `remote_info` | `(profile, path) -> FileEntry` | `{ profile, path }` |
| `remote_home` | `(profile) -> String` | `{ profile }` |
| `remote_quick_access` | `(machine: String) -> Vec<QuickAccess>` | `{ machine }` |
| `remote_add_quick_access` | `(machine, path) -> Result<bool, String>` | `{ machine, path }` |
| `remote_remove_quick_access` | `(machine, path) -> Result<(), String>` | `{ machine, path }` |
| `remote_upload` | `(profile, sources, dest_dir) -> Result<(), String>` | `{ profile, sources, destDir }` |
| `remote_download` | `(profile, sources, dest_dir) -> Result<(), String>` | `{ profile, sources, destDir }` |
| `remote_copy_within` | `(profile, sources, dest_dir, move_mode: bool)` | `{ profile, sources, destDir, moveMode }` |
| `remote_delete` | `(profile, paths) -> Result<(), String>` | `{ profile, paths }` |
| `remote_transfer` | `(from, to, sources, dest_dir, move_mode)` | `{ from, to, sources, destDir, moveMode }` |
| `submit_job` | `(source_profile, source_path, target_profile, target_dir)` | `{ sourceProfile, sourcePath, targetProfile, targetDir }` |
| `remote_job_query` | `(profile, command: String) -> String` | `{ profile, command }` |
| `paths_exist` | `(paths: Vec<String>) -> Vec<bool>` | `{ paths }` |
| `remote_paths_exist` | `(profile, paths) -> Result<Vec<bool>, String>` | `{ profile, paths }` |
| `open_in_terminal` | `(path: String, remote: Option<RemoteProfile>)` | `{ path, remote }` |
| `window_at_point` | `(x, y) -> Option<String>` | `{ x, y }` |
| `merge_machine_window` | `(target, machine, path) -> Result<(), String>` | `{ target, machine, path }` |
| `register_machine_window` | `(label, machine, path) -> Result<(), String>` | `{ label, machine, path }` |

**Note on naming**: Tauri v2 converts Rust `snake_case` command params to **camelCase**
on the JS side by default (the `#[tauri::command]` macro does `rename_all = "camelCase"`).
So a Rust param `new_name` is invoked from JS as `newName`, and `dest_dir` as `destDir`.
In `src/api/tauri.ts` the JS keys are therefore camelCase. If you add/rename a command
param, update BOTH the Rust signature and the `api/tauri.ts` wrapper.

## CRITICAL Safety Constraint (chemistry parsing)

**Never call `rswfn::base::Mole::from_file` directly.**

Several of `rswfn`'s readers terminate the whole process or panic on malformed input:

- `GjfReader::get_atoms` calls `std::process::exit(0)` when no charge/multiplicity
  line is found.
- `LogReader::get_coefs` calls `std::process::exit(0)` when the log lacks `pop=full`.
  (`Mole::from_file` always calls `get_coefs`, so it is unsafe for almost all `.log` files.)
- Various readers use `panic!`/`unwrap`/`expect` on incomplete data.

Because a panic or `exit` would crash the entire file explorer, all parsing must go
through the hardened entry points in `wmfold-core/src/chem.rs`:

- **`read_atoms_basis(path, kind)`** — the only safe gateway that (1) pre-checks
  gjf / log files to avoid `process::exit`, (2) wraps reader calls in
  `catch_unwind`, (3) only calls `get_atoms()` and `get_basis()` — **never `get_coefs()`**.
- **`parse_chem(path)`** / commands `read_chem` / `read_chem_batch` — use
  `read_atoms_basis` internally and fall back to safe defaults on failure.
- **`kind_from_ext(ext)`** — maps an extension to a chem kind. Use this instead of
  `rswfn::reader::get_fileType` (which `panic!`s on unknown types).

When adding a new format, add it to `kind_from_ext`, `type_label`, and the `match`
inside `read_atoms_basis`, plus `available_columns`/`kind_code` in
`wmfold-core/src/columns.rs` and the frontend labels in `src/utils/format.ts`.

## Important Constraints / Gotchas

### rswfn path dependency
`wmfold-core/Cargo.toml` has `rswfn = { path = "../../rswfn" }`. This requires the
project to live at `E:\code\wmfold` with `E:\code\rswfn` present. Moving the project
breaks the build; update the path if relocated. `src-tauri` and `wmfold-cli` reach
`rswfn` only through `wmfold-core`.

### rswfn uses pure-Rust `faer` (no MKL / BLAS)
`rswfn` no longer depends on Intel MKL / `ndarray-linalg`. Its linear algebra now uses
the pure-Rust **`faer`** crate, and `default = []` (no default features). So the backend
builds **without any system BLAS / LAPACK / MKL** — much lighter and faster to compile.
The old `intel-mkl` / `ndarray-linalg` / `use-lina` / `use-imkl` / `use-blas` feature
names no longer exist in `rswfn`; do **not** try to re-enable them.

### rswfn panics are still "noisy"
`catch_unwind` prevents crashes but the default panic hook still prints a backtrace to
stderr for malformed chemistry files. In dev the console shows these; in the packaged
app stderr is discarded. This is expected behavior — do not "fix" it by removing the
`catch_unwind` wrapper.

### Release panic mode
The repo's `src-tauri/Cargo.toml` does NOT set `panic = "abort"` in `[profile.release]`,
so release builds unwind (needed for `catch_unwind`). Note `rswfn`'s own
`[profile.release] panic = "abort"` does **not** apply when rswfn is a dependency — the
consuming workspace profile governs, so `catch_unwind` works in release.

### Merged launcher and Windows console
`src-tauri/src/main.rs` is the `wmfold` entry. Release builds use the **GUI**
subsystem (`#![windows_subsystem = "windows"]`), so launching from Explorer never
creates a console window. Terminal modes (`-cli`, `--help`, `--version`) call
`attach_console()` first: if stdout/stderr are already redirected to a file or pipe they
are left untouched (so `wmfold -cli ... > out.json` works); otherwise it calls
`AttachConsole(ATTACH_PARENT_PROCESS)`, opens `CONIN$` / `CONOUT$`, `SetStdHandle`, with
`AllocConsole()` fallback. GUI mode runs Tauri in-process — there is no
`FreeConsole`/detached `--gui-child` child anymore (the legacy `--gui-child` argument is
still accepted). Debug builds keep the console subsystem for `tauri dev` logs, and
`-tui`/`ensure_console()` no longer exist. Because a GUI-subsystem process is
asynchronous to PowerShell, scripted CLI calls should run through `cmd /c` (redirection
is preserved); interactive `wmfold -cli` in a real console works.
Because the app itself is GUI-subsystem, every non-terminal child process must be
spawned hidden: `remotes.rs::hide_console()` sets `CREATE_NO_WINDOW` on `ssh`, `plink`
and `tar` (`CommandExt::creation_flags`). The only intentional visible console is
`open_in_terminal`; do not remove those flags or remote browsing/submission will flash
terminal windows.

### Shared core (`wmfold-core`)
Never add Tauri or any other desktop-only dependency to `wmfold-core`: it must build on
a Linux server with no WebView2 and no desktop environment (that is how `wmfold-cli` is
built). Platform differences are handled with `#[cfg(windows)]` inside core (the
hidden-file attribute) or stay in `src-tauri` (drive letters, quick-access merging,
opener plugin). File operations (create/rename/delete/copy/move) live in core so the
desktop app and `wmfold-cli` can both use them; `src-tauri/src/commands.rs` is only a
thin `#[tauri::command]` adapter.

### Dependency pin (pest / stacker)
`wmfold-core/Cargo.lock` and `wmfold-cli/Cargo.lock` pin `pest` **2.9.0** (matching
`src-tauri/Cargo.lock`). Newer pest pulls `pest_stack_guard → stacker`, which compiles a
C file and broke MSVC builds on this machine. Do not run `cargo update` blindly in those
crates; if you must update, verify the stacker build still succeeds.

### SSH remotes and the `-cli` protocol
`src-tauri/src/remotes.rs` supports **two authentication modes** per profile:

- no password → system `ssh` with `BatchMode=yes` (`StrictHostKeyChecking=accept-new`,
  key-based auth only);
- password set → the bundled **`plink.exe`** (PuTTY 0.85, embedded as
  `src-tauri/resources/plink.exe`, extracted to `%APPDATA%\wmfold\plink.exe`) because
  Windows OpenSSH cannot accept a password non-interactively. The password goes through
  a temporary `-pwfile` (deleted right after the command), never on the command line.
  The first connection probes the host key: plink in `-batch` mode refuses the unknown
  key but prints its `SHA256:...` fingerprint on stderr; that fingerprint is cached as
  `profile.host_key` and passed with `-hostkey` on later calls. Passwords are encrypted
  at rest with Windows DPAPI (`save_to`/`load_from`), so `remotes.json` never contains
  plaintext. The bundled plink is MIT-licensed; see `src-tauri/resources/PUTTY-LICENCE.txt`.

Remote commands run `wmfold -cli list/info/structure`; the JSON contract lives in
`wmfold-cli/src/cli.rs` (`PROTOCOL`, `ListResponse`, `InfoResponse`, `StructureResponse`)
and is mirrored by `RemoteListing` / `RemoteInfoResponse` / `RemoteStructureResponse` in
`remotes.rs`. Change both sides together and bump `PROTOCOL` if fields change.
The `structure`/`normalEnd` changes bumped `wmfold-cli` to 0.2.2; `RemoteStatus.outdated` lets the desktop
re-upload the bundled server binary when its `-cli version` differs from
`wmfold_cli::SERVER_VERSION`.

The server binary is uploaded over **ssh stdin** (`cat > ~/.local/bin/wmfold`), not
`scp`: Windows `scp` rejects some local paths with “hostname contains invalid
characters”. The bundled Linux x86_64-musl binary lives at
`src-tauri/resources/wmfold-server`; rebuild with `npm run build:server` (needs the
`x86_64-unknown-linux-musl` rustup target; links with `rust-lld`). Paths passed into
remote commands must go through `sh_quote`.

Frontend remote state lives in `src/stores/explorer.ts`: `remoteProfiles`,
`activeRemote`, `isRemote`; switching machines clears history and uses POSIX breadcrumbs
(`posixCrumbs`). New folder / rename / direct delete still reject remote mode through
`requireLocal()`, and opening a remote file only previews it. Copy / cut / paste are
cross-device: the clipboard stores `{ action, sources: [{ path, profile|null }] }`, and
`paste()` dispatches by target machine — local↔local uses core copy/move, local→remote
`remote_upload`, remote→local `remote_download`, same-remote `remote_copy_within`, and
remote→remote `remote_transfer` (via a local temp dir). “Cut” deletes the source after a
successful transfer. Machines are managed in the settings dialog's **远程机器** tab and
opened from the title bar "+" menu (the main window starts with only 本机).

### Cross-device transfer (tar over ssh)
File and directory transfers stream `tar` archives through the ssh pipe (never `scp`):

- local → remote: `tar` the local path into a temp archive, pipe it to
  `ssh 'mkdir -p <dest> && tar -xf - -C <dest>'`;
- remote → local: `ssh 'tar -cf - -C <parent> -- <name>' > temp.tar`, then extract locally;
- remote → remote: download to a local temp dir, upload to the target, optionally delete
  the source (`remote_transfer`);
- same remote machine: `cp -a -- … <dest>` / `mv -- … <dest>`.

Windows ships `tar.exe` (bsdtar), and the Linux servers need `tar` (standard). Temp
archives/dirs go under `%TEMP%` and are removed afterwards. Paths must be quoted with
`sh_quote`; never interpolate them raw. Large transfers stream file→stdin/stdout, not into
memory.

### Serde camelCase
Rust models use `#[serde(rename_all = "camelCase")]`, so fields serialize as camelCase
(e.g., `n_atoms` → `nAtoms`, `mol_w` → `molW`). The matching TS types live in
`src/types/index.ts`. Keep both in sync.

### Settings persistence
Settings live in `%APPDATA%\wmfold\settings.json` (fallback `$HOME`), shape
`{ "column_settings": { "<kind>": ["formula", ...] }, "show_hidden": false }`. Keep the
**snake_case** keys because this is the format written by the archived 0.3.4 GPUI build —
the two implementations share one config file; `show_hidden` is `#[serde(default)]`, so
old files without it load as `false`. `src-tauri/src/settings.rs` validates on load:
unknown kinds are dropped, unknown column keys are filtered, missing kinds get defaults,
and `show_hidden` is preserved. The Pinia store keeps `showHidden` and saves the full
`Settings` object on every change (`save_settings`); `store.visibleEntries` filters
`isHidden` entries for both the list and the filter-bar counts. The settings dialog has
four tabs: 展示信息 (columns), 程序设置 (show hidden files / terminal), AI 对话
(DeepSeek key / model / base URL), 远程机器 (SSH profiles); `store.settingsTab` /
`settingsVisible` control it. AI config is stored separately in
`%APPDATA%\wmfold\ai.json` (never in `settings.json`, because every column change
rewrites that file).

### Quick access pins
Quick access is the horizontal bookmarks row (`QuickAccessBar.vue`) below the title bar.
Default items come from `commands::get_quick_access()`: Windows → 此电脑 (`::ThisPC`)
+ 用户目录 (home); non-Windows → 用户目录 only. User-pinned folders are persisted
separately in `%APPDATA%\wmfold\quick_access.json` (JSON array of paths) via
`src-tauri/src/quick_access.rs` and appended to the local defaults. When a remote machine
is active, `store.activeQuickAccess` replaces the list with that machine's home directory
plus its own pinned folders; switching back restores the local list. Remote pins are
stored separately in `%APPDATA%\wmfold\remote_quick_access.json` keyed by profile id
(`{ "<id>": ["/home/user/calc", ...] }`) via `src-tauri/src/quick_access.rs`; commands
`remote_quick_access` / `remote_add_quick_access` / `remote_remove_quick_access`
(`remotes.rs`) manage them, and deleting a profile also drops its pins. The right-click
「添加到快速访问」 and the bookmark hover × work in both local and remote mode.

Availability is refreshed with `paths_exist` (local `Path::is_dir`) and
`remote_paths_exist` (remote `test -d` over ssh); `store.unavailableQuickAccess` marks
missing items, `QuickAccessBar` renders them grey with a 不可用 badge and blocks
navigation (click shows `store.error`), while the hover × still removes them. The map is
keyed by `<machineId>::<path>`. Keep pinned folders out of `settings.json`: the frontend
saves the whole settings object on every column change and would overwrite them.

`store.lastPaths` remembers the last directory per machine (`'local'` or profile id); the
machine tabs restore it when switching back, falling back to home / 此电脑 if the
remembered path no longer loads. It is in-memory only (session scope).

### Settings persistence (program + remote)
`settings.json` also stores `terminal_tool` (`auto` / `wt` / `powershell` / `cmd`, default
`auto`); `sanitize` normalizes unknown values to `auto`. It is used by
`commands::open_in_terminal` when launching the system terminal.

Each `RemoteProfile` also has job-query settings: `job_query_enabled`,
`job_query_command` (default `squeue -u $USER`) and `job_query_interval` seconds
(clamped 5–600, default 10). The right panel (`RightPanel.vue`) always shows 预览 and
AI 对话 tabs, and adds a 作业查询 tab only when the active remote machine has job query
enabled; `store.rightPanelTab` (`preview` / `ai` / `jobs`) owns the selection so the
ribbon can open the AI tab. `JobPanel.vue`
runs `remote_job_query` on mount/tab activation and every interval, rendering
whitespace-separated output (e.g. squeue) as a table and anything else as raw
monospace text. Editing a machine in Settings calls `refreshRemoteProfiles`, which also
replaces `store.activeRemote` with the updated profile so the tab appears immediately.

### Submit job (提交作业)
Each `RemoteProfile` has `submit_command` (must contain `$PATH`; empty = this machine is
not offered), `submit_dir` (default `~/wmfold`), and `clear_chk_path`. Any file in the
local or remote file list has a right-click 「提交作业」 item (`ContextMenu.vue`); it
opens `SubmitJobDialog.vue`, lets the user pick one of `store.submitTargets` (profiles
whose command is non-empty), and calls `submit_job`. The target directory is the target
machine's currently open folder when that machine is the active tab
(`store.submitTargetDir`), otherwise `submit_dir`; the backend expands `~` using the
remote `$HOME` and creates the directory. `submit_job` supports a local source, a remote
source on the target machine, or a remote source on another machine (download + upload
through a local temp dir). It replaces literal `$PATH` with the shell-quoted remote file
path and runs `cd <dir> && bash -lc '<command>'`; the login shell loads the server's
`~/.profile` / `~/.bashrc` PATH so custom submit commands such as `subg16` resolve even
though plain non-interactive SSH does not load them. If a command still cannot be found,
configure its absolute path (e.g. `/opt/bin/subg16 $PATH`). Non-zero command exits are
returned in `SubmitJobResult` for display instead of being converted to an error. When
`clear_chk_path` is on, a temp copy of a `.gjf` has every `%chk=/path/name.chk` rewritten
to `%chk=name.chk` before upload (the original is never modified); submitting a file to
its own directory with that option writes `name.wmfold.gjf` to avoid overwriting it.
Settings for these fields live in Settings → 远程机器; old profiles load with the
defaults.

### Global keyboard shortcuts
`src/App.vue` (`onKeyDown`) skips shortcuts while focus is in `INPUT`/`TEXTAREA`/
`SELECT`, then maps: `Alt+←/→` back/forward, `Backspace` up, `F5` refresh, `F2` rename,
`Delete` delete, `Ctrl+C/X/V`, `Ctrl+N` new file, `Ctrl+Shift+N` new folder. The
new/delete/rename flows come from `composables/useFileActions.ts`, which the ribbon
buttons use too — change them in one place only.

### Windows path handling
Paths from the backend use backslashes. `src/utils/path.ts` normalizes them and treats
`::ThisPC` as a virtual "This PC" root (lists drives). Frontend never builds paths itself
— it always goes through those helpers.

### Preview splitter & external terminal
`App.vue` inserts a `.wm-splitter` between `.wmfold-main` and `ChemPanel`; dragging sets
`store.previewWidth` (220..70% of the window) and persists it in
`localStorage['wmfold.previewWidth']`. `ChemPanel` receives an inline `width` style, and
`.wm-chempanel` must stay `flex-shrink: 0`.

There is **no embedded terminal** anymore. `store.openTerminal()` calls the
`open_in_terminal` command (`src-tauri/src/commands.rs`), triggered by the ribbon
terminal button, the blank-area / folder context menu item, and `Ctrl+``. On Windows it
prefers `wt.exe` (found explicitly under `%LOCALAPPDATA%\Microsoft\WindowsApps`, since
the App Execution Alias is often missing from a GUI process's PATH) and falls back to a
new PowerShell window (`Set-Location`) and then `cmd.exe`; the tool is configurable in
**设置 → 程序设置** (`terminal_tool`: auto / wt / powershell / cmd). `wt.exe` is important
for remote sessions: it gives ssh a real ConPTY stdin, whereas a PowerShell/cmd process
started with `CREATE_NEW_CONSOLE` from the console-less GUI can leave ssh with no input
(the window shows the remote prompt but typing does nothing). After spawning,
`focus_terminal_window_soon` raises the new terminal window to the foreground (otherwise
the window can appear behind wmfold and keys keep going to the app). For a remote
machine it opens `ssh -tt` (interactive, so a password profile prompts for the password
in the terminal) and runs `cd <remote path> && exec $SHELL -l`. The current directory
comes from the store (`::ThisPC` → home). Do not reintroduce a webview/xterm terminal
panel.

### Remote connection reuse
`store.remoteStatusCache` and `remotePinsCache` remember each machine's `RemoteStatus`
and pinned folders keyed by `id|host|port|user` / profile id. Switching to a machine that
was already checked skips `remote_check`/`remote_install` (the expensive repeated
handshake) and uses the cached home/pins immediately; browsing still issues one ssh per
operation because Windows OpenSSH does not support ControlMaster multiplexing. Cache
entries are dropped if a switch fails. `lastPaths` restores the last directory per
machine on switch.

### Blocking commands must run off the UI thread
Tauri's default `#[tauri::command]` runs on the main (UI) thread. `#[tauri::command(async)]`
only moves a sync body onto the Tokio runtime — it still runs **on a Tokio worker**, not
a spare blocking thread. So:
- pure CPU / filesystem work that may take long (recursive scans, chemistry parsing,
  drive enumeration) must be wrapped in `tauri::async_runtime::spawn_blocking` (see
  `scan.rs`), or written as a true `async fn`;
- never create or drop a nested Tokio runtime on a worker: `reqwest::blocking` panics
  with “Cannot drop a runtime in a context where blocking is not allowed”. See
  `ai.rs` for the `spawn_blocking` wrapper pattern.
`commands::get_drives` additionally uses `GetLogicalDrives` instead of stat-ing every
drive letter with `Path::exists()`, because a disconnected mapped drive can block for
many seconds.

### Recursive scan (扫描)
`wmfold-core/src/fs.rs::scan_recursive` walks a folder iteratively with an explicit
stack and returns every **file** entry (directories are not included) plus `ScanStats`.
It skips symlinked directories (junction loops), optionally skips hidden entries
(`include_hidden`), throttles progress callbacks to ~80 ms, and stops at `max_entries`
(default 100 000). `src-tauri/src/scan.rs` wraps it: `scan_recursive(app, path, scan_id,
include_hidden, max_entries)` emits `scan-progress` payloads
(`scanId/dirs/files/current/elapsedMs/done/...`) and `cancel_scan(scan_id)` flips an
atomic checked by the walker. Only one scan may run at a time.

Frontend state (`recursiveMode`, `recursiveRoot`, `scanning`, `scanProgress`,
`scanStats`) lives in the Pinia store. `loadContent()` clears the recursive state (and
cancels an in-flight scan), so any navigation / machine switch exits the view; `F5`
re-scans. The UI label is 「扫描」 (the internal names still say recursive).

`FileList.vue` now uses Element Plus **TableV2** (`el-auto-resizer` + `el-table-v2`) for
virtual scrolling: all `store.sortedEntries` are passed as data, but only the visible
~20 rows are in the DOM. Columns/cells/header are rendered through the `header-cell` /
`cell` slots; row selection checkboxes, select-all, header sorting, row click/dblclick
and the custom context menu are implemented manually because TableV2 has no built-in
row-selection column. `tableKey` (path + filter + search + sort) remounts the table so
the scroll position returns to the top after a filter/search/sort change. Do **not**
reintroduce `el-table` or a row cap: the old eager table blocked for ~7 s at 1000 rows
× 13 columns on the test machine.

Because only visible rows matter, `FileList.onRowsRendered` (debounced 120 ms) calls
`store.ensureChemForRows(start, end)` for the cache/visible range plus a small overscan.
After `loadContent` / a completed scan, `startChemQueue()` queues **every** chemical file
in the current list, and `ensureChemForRows` moves visible paths into `chemPriority` so
they are parsed first. A single async worker (`kickChemWorker`) takes batches of 6 from
priority then the pending queue, yields between batches, updates `entries`, and keeps
`chemDone` / `chemTotal` for the status bar; `chemGeneration` invalidates the worker when
the folder or scan changes. Failed batches are removed from `chemAttempted` so a future
pass can retry. `resetChemQueue()` is called when navigation or scanning starts.

### AI chat (DeepSeek)
`src-tauri/src/ai.rs` calls DeepSeek's OpenAI-compatible API with `reqwest::blocking`,
always inside `tauri::async_runtime::spawn_blocking` (a `#[tauri::command(async)]` sync
body still runs on a Tokio worker, and reqwest's blocking client panics there with
“Cannot drop a runtime in a context where blocking is not allowed”). It uses
`rustls-no-provider` + the ring provider (`ensure_crypto_provider`), because Windows
Schannel can be unavailable; do not switch it to `default-tls`. The model runs a
function-calling loop
(`list_dir` / `read_file` / `search_files` / `file_info`, up to 12 rounds) and every
tool call is emitted as an `ai-tool` event (`requestId`, step with `id/label/status/
summary`) so the panel can show what was read. Identical tool name + arguments are
skipped with a synthetic tool result so a looping model cannot waste all rounds on the
same call. When the 12-round cap is reached, `chat_loop` adds a final instruction and
sends one more request with `tool_choice: "none"` so the user gets a summary of the
results already collected instead of a hard “超过 12 轮” error; only if that summary is
empty does the error surface. `search_files` content mode deliberately skips files over
512KB; the tool description and system prompt say so, because large chemistry logs must
be handled by filename search + `file_info`/`read_file`. Tool paths go through
`resolve_path`, which canonicalizes and rejects anything outside the current folder;
`read_file` is capped at 96 KB by default / 256 KB max, and binary files are refused.
`ai_chat` also receives the up-to-30 selected files and includes their relative paths
in the system prompt.

Config lives in `%APPDATA%\wmfold\ai.json` (`api_key` encrypted via `secret.rs` DPAPI,
`model`, `base_url`, `thinking`, `reasoning_effort`), **not** in `settings.json`.
Defaults follow the current DeepSeek API docs: model `deepseek-flash`, base URL
`https://api.deepseek.com` (OpenAI-compatible, no `/v1`), thinking off, effort `high`;
old `deepseek-chat` / `deepseek-reasoner` names are normalized to `deepseek-flash`.
Key precedence: saved key first, then `DEEPSEEK_APIKEY`; `ai_config` returns the source
(`saved` / `env` / `none`) plus a masked preview. `ai_save_config` with an empty key
clears the saved key. `ai_test` GETs `/models`.

Thinking mode is sent as `{"thinking":{"type":"enabled"|"disabled"}}`; when enabled,
`reasoning_effort` is one of `low|high|max`, and `temperature` is omitted (it has no
effect in thinking mode). The file tools require thinking-mode history to carry every
assistant `reasoning_content`; `AiChatMessage` / `AiReply` therefore have an optional
`reasoningContent`, which the frontend keeps on each assistant message and sends back.
If history predates this field, the backend falls back to non-thinking for that turn to
avoid a 400.

Tool calls also return `results: Vec<AiToolResult>` (`path` / `name` / `rel` / `isDir`)
for every file or folder the tool touched; `AiChatPanel.vue` renders them as clickable
chips (first 6, expandable). Clicking calls `store.revealAiPath`: in scan mode it keeps
the scanned entries, selects/previews a file and asks `FileList` to scroll to it, while
a folder narrows the scan list with the search box to that subdirectory; outside scan
mode a file navigates to its parent and selects it, and a folder navigates into it.
`revealRequest` (`{path, nonce}`) is the store-to-FileList signal; FileList uses the
TableV2 `scrollToRow` method (with a DOM instance fallback) after two ticks. When the
scan-list search/type filter is active (including the folder narrowing above),
`CommandRibbon.vue` shows a 「取消过滤」 button immediately after the scan button; it
calls `store.clearFilters()` to reset search and `filterType` to `all`. The scan button
keeps its native title even when disabled by overriding `pointer-events` for
`.wm-scan-btn.is-disabled`, so the tooltip is available at 此电脑 / remote roots.

AI scope is per request: `ai_chat` takes `recursive` (sent as `recursiveMode` at send
time). In normal mode `resolve_path` only allows the root and its direct children,
`list_dir`/`search_files` reject a non-root path, and search uses `fs::list_dir` rather
than a recursive scan; in scan mode the old recursive behavior applies. The system
prompt states the current scope and tells the model to ask the user to scan instead of
reading subfolders. `filteredTypeCounts` counts entries after search only, and FilterBar
renders `filtered / total` while any search/type filter is active.

`FileList.vue` separates row highlighting from checkbox selection: a plain click sets
`selectedPaths` to one row (highlight) and clears `checkedPaths`, so the row checkbox
stays unchecked; Shift+click selects the range from `activeRowPath` to the clicked row
and fills both `selectedPaths` and `checkedPaths`, so those rows are checked. Checkbox
clicks and select-all update `checkedPaths`; the header checkbox uses `checkedPaths`
(`allSelected` / `someSelected`) rather than the single-row highlight. The store watch
prunes `checkedPaths` whenever `selectedPaths` is cleared or shrunk.

`ContextMenu.vue` measures itself after opening and clamps `left/top` so the whole menu
stays inside the viewport (with `max-height` + scrolling for tall menus). Folder rows and
blank list areas expose 「在文件资源管理器中显示」 on local machines only; it invokes
`open_in_explorer(path)`, which spawns `explorer.exe <folder>`. Blank-area invocation
uses `store.currentPath`; remote / `::ThisPC` are rejected or hidden.

The Markdown renderer (`src/utils/markdown.ts`) also linkifies file paths with known
extensions and folder paths ending in `/` into `span.wm-ai-path` (`data-wm-path` /
`data-wm-dir`), skipping URLs and date-like slashes. `AiChatPanel` handles clicks by
event delegation, resolves the relative path against that message's `rootPath` (stored
when the question was sent, so old answers still point at their original folder), and
calls the same `revealAiPath`. Code blocks are intentionally not linkified.

Frontend types are `AiConfigView` / `AiReply` / `AiToolStep` in
`src/types/index.ts`; the chat panel is `AiChatPanel.vue` (store state `aiMessages`,
`aiSending`, `aiConfig`, `openAiPanel`, `sendAiMessage`, `onAiToolEvent`) and replies are
rendered by the escaping-only `src/utils/markdown.ts` (headings, lists, code fences,
inline code, bold, and GitHub-style tables with `| --- |` separators; table cells wrap
inside `.wm-ai-table`). AI chat is local-only; remote mode shows an error instead of
reading remote files.

### Single instance
`tauri-plugin-single-instance` is registered first in `lib.rs`; a second launch focuses
the existing `main` window instead of starting another process (multiple instances would
share the WebView2 user-data directory and the config files, and can leave a window
“未响应”).

### Auto update (Tauri 2 updater)
The desktop app uses the official Tauri 2 updater (`tauri-plugin-updater` +
`tauri-plugin-process`). `src-tauri/tauri.conf.json` sets
`bundle.createUpdaterArtifacts: true` and:

```
plugins.updater.endpoints = [
  "https://www.xiaofei911.top/statics/wmfold-updates/windows/latest-version-info.json"
]
plugins.updater.pubkey = <public key>
```

The endpoint JSON uses the Tauri static schema: `version`, `notes`, `pub_date`, and
`platforms["windows-x86_64"] = { signature, url }`. The signature is the contents of
`...-setup.exe.sig`; the URL points at the NSIS installer.

Signing keys are generated with `npm run tauri signer generate -w .tauri/wmfold.key`.
`.tauri/` is gitignored; `scripts/build-app.mjs` loads the private key via
`TAURI_SIGNING_PRIVATE_KEY`, runs the version bump + `tauri build`, then renames the
portable exe. `scripts/publish-update.mjs` copies the NSIS installer/signature, the
portable exe and the MSI into `windows/<version>/`, writes
`windows/latest-version-info.json`, uploads via `scp`+`ssh` to
`fly@47.115.214.219:/home/fly/share/programs/wmfold/`, and verifies the public URLs.
The official site's download buttons point at this directory (the page resolves the
current version from `latest-version-info.json`), so the site no longer stores installers.

Server layout (public through a symlink `/home/fly/statics/wmfold-updates` served by
Caddy's `/statics/*` handler):

```
/home/fly/share/programs/wmfold/windows/
├── latest-version-info.json
└── <version>/
    ├── wmfold_<version>_x64-setup.exe(.sig)   # installer + updater signature
    ├── wmfold_<version>.exe                   # official-site portable download
    └── wmfold_<version>_x64-en-US.msi         # official-site MSI download
```

Do not put update secrets or version installers in git; only `.tauri/` key on the build
machine. Auto-update only starts working from 0.3.12 (earlier versions have no updater);
0.3.12/0.3.13 had a Vue reactivity bug where the `Update` class instance was wrapped in a
deep `ref` and `downloadAndInstall` failed with “Cannot read private member…”. Keep
`availableUpdate` as a `shallowRef`. 0.3.14 fixed that; 0.3.47 is the current release.

The shared `src/composables/useUpdater.ts` owns update state (version, `Update` object,
checking/installing/progress/message) so the title bar and Settings dialog use one
check. `TitleBar.vue` auto-checks silently on mount, shows `wmfold v<version>` next to
the brand, and renders a 「升级」 button immediately before the settings gear when
`availableUpdate` is set; clicking it confirms via `ElMessageBox` and calls
`downloadAndInstall` + `relaunch`. Keep `availableUpdate` a `shallowRef`.

`ChemMeta` also has `job: Option<String>` for `gjf` and `log` files: `chem::read_job`
extracts the Gaussian route (text scan, no rswfn reader, so no panic/exit risk) and the
preview panel shows it as 「作业」. The file list also has a `job` chemical column for
`gjf`/`log` kinds (in `CHEM_DEFS`/`available_columns`, rendered monospace with ellipsis);
existing settings are migrated once by `localStorage['wmfold.jobColumnMigrated.v1']` so
the column appears without re-adding it after a user disables it. `ChemPanel` lazily calls
`read_chem` / `remote_info` when the list did not load chem metadata (chem columns
disabled), so the preview always has formula/energy/job for a selected chemistry file.

`ChemMeta.normal_end: Option<bool>` is set only for `log` files by
`chem::read_log_normal_end`, which calls rswfn's `LogReader::read_normalend` inside
`catch_unwind` (the method indexes the last line, so empty files would panic; the wrapper
returns `None`). Log files get a `normalEnd` chemical column (label 「正常结束」, values
是/否, default-added via `localStorage['wmfold.normalEndColumnMigrated.v1']`) and the
preview shows 「正常结束 / 未正常结束」. `wmfold-cli` 0.2.2 serializes the field for remote
`info`/`list`; rebuild `src-tauri/resources/wmfold-server` with `npm run build:server`
after changing it.

### 3D molecular preview (three.js)
`ChemPanel.vue` renders `MoleculeViewer.vue` at the **top** of the preview content for any
selected chem file. The viewer calls the async command `read_structure(path)`, which goes
through `wmfold_core::chem::read_structure` (same safe `read_atoms_basis` gateway as
metadata; never call rswfn readers directly). It returns
`MolStructure { ok, atoms, bonds }` with atom coordinates in Å and bond index pairs;
`MolAtom` is `{ element, x, y, z }`.

The Vue component builds a ball-and-stick model with `three` +
**`TrackballControls`** (left rotate, wheel zoom, right pan): spheres colored by element,
half-bonds colored by the endpoint atoms, directional/hemisphere lights, auto camera
fitting, resize observer, and disposal of geometries/materials on path change and
unmount. The stage height is a drag handle below the canvas (persisted in
`localStorage['wmfold.moleculeHeight']`, clamped 150–640 px). On any stage resize,
schedule `resize()` on the next animation frame, call `renderer.setSize(w, h, true)`,
update the camera aspect, and call `controls.handleResize()` so TrackballControls screen
coordinates stay correct when the preview splitter/width changes.

Remote files use the same viewer: `MoleculeViewer` receives the active `RemoteProfile`
and calls `remote_structure(profile, path)`, which runs
`wmfold -cli structure <path>` over SSH and returns `MolStructure` (the server parses
coordinates; the desktop only renders them). Adding the CLI `structure` command bumped
`wmfold-cli` to 0.2.2. `RemoteStatus.outdated` compares the installed server version
with `wmfold_cli::SERVER_VERSION`; `remote_install` re-uploads the bundled
`wmfold-server` when they differ, and the store retries `remoteCheck` before browsing.
Keep the bundled `src-tauri/resources/wmfold-server` rebuilt with
`npm run build:server` after touching `wmfold-cli` or core chemistry code.

`three` and `@types/three` are regular frontend dependencies; the bundle grows by a few
hundred KB (Vite may warn about chunk size). `MolStructure` / `MolAtom` types are
mirrored in `src/types/index.ts`; keep the serde camelCase shape in sync.

### Windows Explorer context menu
On Windows, `shell_menu::register_context_menu()` is called from the Tauri setup hook on
every launch. It writes the current-user key
`HKCU\Software\Classes\Directory\Background\shell\wmfold` with label
「通过 wmfold 打开」 and command `"<exe>" --open "%V"` (plus an `Icon`), so right-clicking
blank space inside an Explorer folder can open wmfold at that folder. The key is rewritten
on each start because the portable exe path can change; no admin rights are needed.
`main.rs` accepts `--open <DIR>` / `--open=<DIR>` (and a plain existing directory arg),
sets `WMFOLD_OPEN_PATH` for the first GUI process. On first launch `App.vue` calls
`take_launch_path` and `store.goLocal`; if an instance is already running, the
single-instance callback emits `open-path` and the main window navigates instead.
`winreg` is a Windows-only dependency. On Windows 11 this is a classic menu entry, so it
may appear under 「显示更多选项」.

### Custom title bar / window chrome
`tauri.conf.json` sets `decorations: false`, so the window has no native caption and the
window cannot be closed/minimized without the custom controls in `TitleBar.vue`. Window
operations need explicit capability permissions: `core:window:allow-start-dragging`,
`allow-minimize`, `allow-toggle-maximize`, `allow-is-maximized`, `allow-close` are listed
in `src-tauri/capabilities/default.json` — add more there before calling new window APIs.
The title bar height comes from the `--wm-titlebar-height` CSS variable (32px). Dragging
is implemented in `TitleBar.vue` with `appWindow.startDragging()` on mousedown over any
blank area that is not a button/tab/menu item; double-clicking blank title-bar space
toggles maximize, and edge resizing is handled by the OS. Do **not** rely on the old
`data-tauri-drag-region` 24px strip (it left most of the bar undraggable); if you add a
new interactive control to the title bar, make sure it matches the exclusion selector in
`onTitlebarMouseDown`. Keep `SettingsDialog` **outside** the `<header>` (and it uses
`append-to-body`): when it lived inside the header, the title-bar drag handler swallowed
mousedown on its tabs and 程序设置 / 远程机器 stopped switching. Keep the layout order: title bar (machine tabs + "+" menu +
settings) → bookmarks (quick access) → toolbar → ribbon → filter bar → file list →
status bar. Toolbar/ribbon buttons are icon-only
(`.wm-cmd .cmd-btn span { display: none }`); every command button must carry a `title`
attribute because that is the only hover label.

### Element Plus icons
Icons are registered globally in `src/main.ts`. They are referenced by **string name**
via `<component :is="'Folder'" />` (e.g., in `FileList.vue`/`QuickAccessBar.vue`/
`ChemPanel.vue`). Only use valid `@element-plus/icons-vue` component names. The set used
is checked at runtime; invalid names render blank (no build-time error).

### Chemistry metadata disk cache
`src-tauri/src/chem_cache.rs` backs `read_chem` / `read_chem_batch` (and the AI
`file_info` tool) with `%APPDATA%\wmfold\chem_cache.json`. A cache hit requires the same
normalized path, the same file size and mtime, and the current
`wmfold_core::chem::CHEM_CACHE_VERSION`. New entries are written periodically (every 128
entries or 5 seconds, temp file + replace); `flush_chem_cache` is called by the frontend
when the background queue finishes, so repeated opens, scans and restarts reuse the
cache. Bump `CHEM_CACHE_VERSION` whenever `ChemMeta`, the parser, or rswfn behavior
changes so all old entries are discarded. `read_structure` (3D geometry) is intentionally
not cached; remote `-cli` browsing still parses on the server. Deleting
`chem_cache.json` is safe and simply clears the cache.

After a folder load or completed scan, the backend parses every chemical file in the
current list in the background (not only visible rows). `StatusBar.vue` shows
`解析化学信息 已完成/总数` with a progress bar; the frontend queue is priority-first for
visible rows and runs in batches of 6 with a yield between batches.

### File listing performance
`list_dir` returns entries **without** chemistry metadata (fast). After listing, the
frontend calls `read_chem_batch` for chem files to fill the chemistry columns. Keep this
separation — do not parse chemistry inside `list_dir`.

## Development Conventions

- **No auto-dismissing popups**: do not use `ElMessage` / `ElNotification` for success,
  warning or error feedback. Operations should be silent when the result is visible, or
  set the persistent `store.error` bar (cleared by clicking it). `ElMessageBox` is still
  used only for user-initiated prompts/confirmations (rename, new file, delete, update)
  because those require an answer; never add an auto-closing toast.
- **State**: all explorer state lives in `src/stores/explorer.ts` (Pinia setup store).
  Components read/write store; avoid duplicating state locally.
- **Sorting**: the store always sorts directories-first; column sorting is handled via
  `setSort(prop, order)` and `sortedEntries`. `FileList.vue` uses TableV2, renders its
  own header cells, and calls `setSort(key, 'ascending'/'descending')` on header click
  (TableV2 itself has no row-selection/sort UI). Keep the sort indicator in the header
  slot in sync with `store.sortColumn` / `store.sortOrder`.
- **Backend commands**: return `Result<..., String>` on error; the frontend surfaces the
  message via the store `error` field / `ElMessage`.
- **Tests**: core unit tests live in `wmfold-core/src/{chem,fs,columns}.rs`
  (`cargo test` from `wmfold-core`); Tauri-only tests (settings, quick access, remotes)
  in `src-tauri/src/{settings,quick_access,remotes}.rs` (`cargo test --lib`); CLI protocol
  tests in `wmfold-cli/src/{lib,cli}.rs` (`cargo test` from `wmfold-cli`, or
  `npm run test:cli` from the repo root). They create temp files and assert parsed
  metadata.

## Sample Test Data

Sample files used by tests (XYZ/GJF/MOL/log-energy) are generated inline in the test
modules using `std::fs` in a temp dir. Note the XYZ parser requires coordinates with a
decimal point (e.g., `O 0.0 0.0 0.0`), and GJF must contain a `charge mult` line.

## Archived implementations

`legacy-gpui/` holds the pure-Rust GPUI Kit build (version 0.3.4). It is reference-only:
neither `npm run tauri build` nor `cargo build` at the root touches it, and it is not
covered by tests. It has its own `Cargo.toml` (`rswfn` path `../../rswfn`); build or run
it from inside that directory only when explicitly asked. Do not add features there —
all new work goes into the Tauri 2 project at the repository root.
