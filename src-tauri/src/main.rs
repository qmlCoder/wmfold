//! WMFold 合并启动器。
//!
//! - `wmfold`       启动桌面端（Tauri 2 + Vue）
//! - `wmfold -cli`  机器接口：输出 JSON（脚本 / 远程调用）
//!
//! Release 构建使用 **GUI 子系统**：从资源管理器双击启动时不会创建控制台窗口。
//! `-cli` / `--help` / `--version` 等终端模式会在启动时附加到父控制台（或新开一个），
//! 因此命令行使用仍然正常；debug 构建保留 console 子系统，方便 `tauri dev` 看日志。

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // 终端模式需要控制台：GUI 子系统的 release 进程默认没有控制台。
    #[cfg(all(windows, not(debug_assertions)))]
    if args.iter().any(|a| {
        matches!(
            a.as_str(),
            "-cli" | "--cli" | "-h" | "--help" | "-V" | "--version" | "-tui" | "--tui"
        )
    }) {
        attach_console();
    }

    // 兼容旧版启动参数：桌面模式直接跑 Tauri
    if args.first().map(|a| a == "--gui-child").unwrap_or(false) {
        wmfold_lib::run();
        return;
    }

    // ---- 机器接口：`wmfold -cli ...` 输出 JSON（脚本 / 远程调用）
    if args.iter().any(|a| matches!(a.as_str(), "-cli" | "--cli")) {
        std::process::exit(wmfold_cli::cli_main_named("wmfold", args));
    }

    // ---- -tui 已被移除：给出明确提示，避免误启动桌面窗口 ----
    if args.iter().any(|a| matches!(a.as_str(), "-tui" | "--tui")) {
        eprintln!("wmfold: the terminal UI (-tui) has been removed; use wmfold -cli ... for JSON output.");
        std::process::exit(2);
    }

    // ---- wmfold --help / --version：不启动窗口，直接打印 ----
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_launcher_help();
        return;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("wmfold {VERSION}");
        return;
    }

    // ---- 默认：桌面端 ----
    // 资源管理器右键菜单会传 `--open <DIR>`；通过环境变量带给首次创建的 WebView。
    let open_path = wmfold_lib::open_path_from_args(&args);
    if let Some(path) = open_path.as_deref() {
        std::env::set_var("WMFOLD_OPEN_PATH", path);
    }
    wmfold_lib::run();
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GUI 子系统的 release 进程默认没有控制台；终端模式下附加到父控制台，
/// 找不到父控制台时（例如从资源管理器直接带 `-cli` 参数运行）新建一个。
#[cfg(all(windows, not(debug_assertions)))]
fn attach_console() {
    use std::ptr;
    use windows_sys::Win32::Foundation::{
        GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetFileType, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_DISK,
        FILE_TYPE_PIPE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        AllocConsole, AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS,
        STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    // 已经被重定向到文件/管道时不要 attach 控制台，否则会把 stdout 换成 CONOUT$，
    // 脚本里的 `wmfold -cli ... > out.json` / 管道捕获会拿到空内容。
    let stdout = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if !stdout.is_null() && stdout != INVALID_HANDLE_VALUE {
        let kind = unsafe { GetFileType(stdout) };
        if kind == FILE_TYPE_DISK || kind == FILE_TYPE_PIPE {
            return;
        }
    }

    let mut attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0;
    if !attached {
        attached = unsafe { AllocConsole() } != 0;
    }
    if !attached {
        return;
    }

    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    unsafe {
        let out = CreateFileW(
            wide("CONOUT$").as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if out != INVALID_HANDLE_VALUE {
            SetStdHandle(STD_OUTPUT_HANDLE, out);
            SetStdHandle(STD_ERROR_HANDLE, out);
        }
        let input = CreateFileW(
            wide("CONIN$").as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if input != INVALID_HANDLE_VALUE {
            SetStdHandle(STD_INPUT_HANDLE, input);
        }
    }
}

fn print_launcher_help() {
    println!(
        "wmfold {VERSION} — WMFold 合并启动器

USAGE:
  wmfold                       启动桌面端（Tauri 2 + Vue）
  wmfold --open <DIR>          在桌面端打开指定目录（资源管理器右键菜单）
  wmfold -cli <COMMAND> ...    机器接口：输出 JSON（脚本 / 远程调用）

OPTIONS:
  -h, --help                   显示本帮助
  -V, --version                显示版本
  --open <DIR>                 启动后直接打开目录
  -cli, --cli                  机器可读模式（list / info / version，输出 JSON）

CLI 用法（`wmfold -cli --help`）:
  wmfold -cli list [PATH] [--chem]   目录列表 JSON
  wmfold -cli info <PATH> [--chem]   单个文件/目录 JSON
  wmfold -cli version                CLI 协议版本
"
    );
}


