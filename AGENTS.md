# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, Codex, etc.) working
in this repository. Read it before making changes.

## Project Overview

**WMFold** is a Windows-style file explorer desktop app built for **chemistry /
wavefunction analysis**. It behaves like the built-in Windows Explorer but adds
specialized columns that show molecular information (formula, atom count, charge,
multiplicity, basis-set size, molecular weight, energy) parsed from common
chemistry files.

- **Stack**: Tauri 2 (Rust backend) + Vue 3 + TypeScript + Element Plus + Pinia.
- **Chemistry parsing**: reuses the user's own wavefunction library **`rswfn`**
  (located at `E:\code\rswfn`, outside this repo) as a path dependency.
- **Platform**: Windows only in practice (uses `cargo-tauri`, MSVC toolchain,
  WebView2). Linear algebra inside `rswfn` uses the pure-Rust `faer` crate — **no
  MKL / BLAS / LAPACK** dependency.

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
cargo build --release               # build exe (target/release/wmfold.exe)
cargo test --lib                    # run backend unit tests
```

### Build with auto version bump (recommended)
```bash
npm run build:app
# 1) bumps the last version number (补丁号) +1
# 2) runs `npm run tauri build`
# 3) copies `target/release/wmfold.exe` to `target/release/WMFold_<version>.exe`
# outputs:
#   src-tauri/target/release/WMFold_<version>.exe            (带版本号的程序)
#   src-tauri/target/release/bundle/msi/WMFold_<version>_x64_en-US.msi
#   src-tauri/target/release/bundle/nsis/WMFold_<version>_x64-setup.exe
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
- **生成的程序名包含版本号**：`scripts/rename-exe.mjs` 会把构建产物此外
  `target/release/wmfold.exe` 复制为 `target/release/WMFold_<version>.exe`；安装包文件名
  本身也带版本号（`WMFold_<version>_x64-setup.exe` 与 `..._x64_en-US.msi`）。
- **注意**：tauri CLI 在运行 `beforeBuildCommand` **之前**就读取版本号，因此不要把版本递增
  放进 `beforeBuildCommand`——否则捆绑包仍会用旧版本。版本递增必须在 `npm run tauri build`
  **之前**执行（`build:app` 脚本已按此顺序编排）。
- 三个版本声明文件必须保持一致（Cargo.toml 与 tauri.conf.json 不一致会导致打包失败/警告）。

## Architecture / Layout

```
src/                    # Vue frontend
  api/tauri.ts          # invoke() wrappers for every backend command
  components/           # Toolbar, CommandRibbon, FilterBar, Sidebar, FileList, StatusBar, ChemPanel, ContextMenu, SettingsDialog
  composables/useContextMenu.ts  # global right-click menu state (reactive)
  stores/explorer.ts    # Pinia store: path history, sorting, clipboard, chem metadata, per-type column settings
  styles/windows.css    # Windows 11 look; styling overrides for Element Plus
  types/index.ts        # TS types mirroring Rust Serialize structs
  utils/format.ts       # size/date formatting, icon + type labels, chem colors
  utils/path.ts         # Windows path helpers (split/join, THIS_PC virtual root)
  utils/chemColumns.ts  # chem column defs + per-kind available columns (shared by FileList/FilterBar/SettingsDialog)
  App.vue               # layout + global keyboard shortcuts
  main.ts               # mounts Vue + Pinia + ElementPlus; registers all icons globally

src-tauri/              # Rust backend
  Cargo.toml            # crate `wmfold`, lib name `wmfold_lib`
  src/main.rs           # entry; calls wmfold_lib::run()
  src/lib.rs            # tauri Builder + invoke_handler (registers commands)
  src/chem.rs           # chemistry metadata via rswfn (SAFETY-CRITICAL — see below)
  src/fs_cmds.rs        # directory listing, file ops, drives, quick access
  src/models.rs         # Serialize structs (#[serde(rename_all="camelCase")])
  tauri.conf.json       # window config, bundle config, devUrl http://localhost:1420
```

## Tauri Commands

Registered in `src-tauri/src/lib.rs`; called from frontend via `src/api/tauri.ts`.

| Command | Rust signature | JS args |
| --- | --- | --- |
| `list_dir` | `(path: String)` | `{ path }` |
| `get_home_dir` | `() -> String` | `{}` |
| `get_quick_access` | `() -> Vec<QuickAccess>` | `{}` |
| `get_drives` | `() -> Vec<DriveInfo>` | `{}` |
| `create_folder` | `(path: String)` | `{ path }` |
| `create_file` | `(path: String)` | `{ path }` |
| `rename_path` | `(path, newName)` | `{ path, newName }` |
| `delete_paths` | `(paths: Vec<String>, permanent: bool)` | `{ paths, permanent }` |
| `copy_paths` | `(paths: Vec<String>, dest_dir: String)` | `{ paths, destDir }` |
| `move_paths` | `(paths: Vec<String>, dest_dir: String)` | `{ paths, destDir }` |
| `open_path` | `(app: AppHandle, path: String)` | `{ path }` |
| `read_chem` | `(path: String) -> ChemMeta` | `{ path }` |
| `read_chem_batch` | `(paths: Vec<String>) -> Vec<ChemMeta>` | `{ paths }` |

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
through the hardened entry points in `src-tauri/src/chem.rs`:

- **`read_atoms_basis(path, kind)`** — the only safe gateway that (1) pre-checks
  gjf / log files to avoid `process::exit`, (2) wraps reader calls in
  `catch_unwind`, (3) only calls `get_atoms()` and `get_basis()` — **never `get_coefs()`**.
- **`parse_chem(path)`** / commands `read_chem` / `read_chem_batch` — use
  `read_atoms_basis` internally and fall back to safe defaults on failure.
- **`kind_from_ext(ext)`** — maps an extension to a chem kind. Use this instead of
  `rswfn::reader::get_fileType` (which `panic!`s on unknown types).

When adding a new format, add it to `kind_from_ext`, `type_label`, and the `match`
inside `read_atoms_basis`, plus the frontend labels in `src/utils/format.ts`.

## Important Constraints / Gotchas

### rswfn path dependency
`src-tauri/Cargo.toml` has `rswfn = { path = "../../rswfn" }`. This requires the
project to live at `E:\code\wmfold` with `E:\code\rswfn` present. Moving the project
breaks the build; update the path if relocated.

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

### Serde camelCase
Rust models use `#[serde(rename_all = "camelCase")]`, so fields serialize as camelCase
(e.g., `n_atoms` → `nAtoms`, `mol_w` → `molW`). The matching TS types live in
`src/types/index.ts`. Keep both in sync.

### Windows path handling
Paths from the backend use backslashes. `src/utils/path.ts` normalizes them and treats
`::ThisPC` as a virtual "This PC" root (lists drives). Frontend never builds paths itself
— it always goes through those helpers.

### Element Plus icons
Icons are registered globally in `src/main.ts`. They are referenced by **string name**
via `<component :is="'Folder'" />` (e.g., in `FileList.vue`/`Sidebar.vue`/`ChemPanel.vue`).
Only use valid `@element-plus/icons-vue` component names. The set used is checked at
runtime; invalid names render blank (no build-time error).

### File listing performance
`list_dir` returns entries **without** chemistry metadata (fast). After listing, the
frontend calls `read_chem_batch` for chem files to fill the chemistry columns. Keep this
separation — do not parse chemistry inside `list_dir`.

## Development Conventions

- **State**: all explorer state lives in `src/stores/explorer.ts` (Pinia setup store).
  Components read/write store; avoid duplicating state locally.
- **Sorting**: the store always sorts directories-first; column sorting is handled via
  `setSort(prop, order)` and `sortedEntries`. Table columns use `sortable="custom"` and
  emit `sort-change` → `setSort`.
- **Backend commands**: return `Result<..., String>` on error; the frontend surfaces the
  message via the store `error` field / `ElMessage`.
- **Tests**: unit tests live in `src-tauri/src/chem.rs` and `src-tauri/src/fs_cmds.rs`
  (`#[cfg(test)] mod tests`). They create temp files and assert parsed metadata. Run with
  `cargo test --lib` from `src-tauri`.

## Sample Test Data

Sample files used by tests (XYZ/GJF/MOL/log-energy) are generated inline in the test
modules using `std::fs` in a temp dir. Note the XYZ parser requires coordinates with a
decimal point (e.g., `O 0.0 0.0 0.0`), and GJF must contain a `charge mult` line.
