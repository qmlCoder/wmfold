//! Tauri 命令层：把前端 `invoke()` 调用转发给 `wmfold-core`。
//!
//! 这里只做参数/返回值适配与平台相关的事情（打开文件、快速访问、磁盘列表），
//! 具体逻辑在 `wmfold-core` 中，`-cli` 版也复用同一套。

use std::path::{Path, PathBuf};

use wmfold_core::models::{ChemMeta, DriveInfo, FileEntry, MolStructure, QuickAccess};
use wmfold_core::{chem, fs};

use crate::quick_access;
use crate::remotes::RemoteProfile;

// ---- 目录与文件 ----

/// 列出目录内容。
#[tauri::command(async)]
pub fn list_dir(path: String) -> Result<Vec<FileEntry>, String> {
    fs::list_dir(&path)
}

/// 当前用户主目录。
#[tauri::command(async)]
pub fn get_home_dir() -> String {
    fs::home_dir()
}

/// 启动参数里由资源管理器右键菜单传入的目录（只用于首次窗口初始化）。
#[tauri::command]
pub fn take_launch_path() -> Option<String> {
    std::env::var("WMFOLD_OPEN_PATH")
        .ok()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
}

/// 新建文件夹。
#[tauri::command(async)]
pub fn create_folder(path: String) -> Result<(), String> {
    fs::create_folder(&path)
}

/// 新建空文件。
#[tauri::command(async)]
pub fn create_file(path: String) -> Result<(), String> {
    fs::create_file(&path)
}

/// 重命名。
#[tauri::command(async)]
pub fn rename_path(path: String, new_name: String) -> Result<(), String> {
    fs::rename_path(&path, &new_name)
}

/// 删除（可回收站或永久）。
#[tauri::command(async)]
pub fn delete_paths(paths: Vec<String>, permanent: bool) -> Result<(), String> {
    fs::delete_paths(&paths, permanent)
}

/// 复制到目标目录。
#[tauri::command(async)]
pub fn copy_paths(paths: Vec<String>, dest_dir: String) -> Result<(), String> {
    fs::copy_paths(&paths, &dest_dir)
}

/// 移动到目标目录。
#[tauri::command(async)]
pub fn move_paths(paths: Vec<String>, dest_dir: String) -> Result<(), String> {
    fs::move_paths(&paths, &dest_dir)
}

/// 用系统默认程序打开路径（桌面版专有）。
#[tauri::command(async)]
pub fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

/// 在 Windows 文件资源管理器中打开某个文件夹（桌面版专有）。
#[tauri::command(async)]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let target = Path::new(&path);
        if !target.is_dir() {
            return Err("只能在文件资源管理器中打开文件夹".to_string());
        }
        std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("无法打开文件资源管理器：{e}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("仅 Windows 支持在文件资源管理器中打开".to_string())
    }
}

// ---- 化学元数据 ----

/// 读取单个化学文件元数据（优先使用磁盘缓存）。
#[tauri::command(async)]
pub fn read_chem(path: String) -> ChemMeta {
    crate::chem_cache::read_chem_cached(&path)
}

/// 读取分子三维结构（原子 + 成键），供预览面板的 three.js 查看器使用。
#[tauri::command(async)]
pub fn read_structure(path: String) -> MolStructure {
    chem::read_structure(&path)
}

/// 批量读取化学文件元数据（优先使用磁盘缓存）。
#[tauri::command(async)]
pub fn read_chem_batch(paths: Vec<String>) -> Vec<ChemMeta> {
    crate::chem_cache::read_chem_batch_cached(&paths)
}

/// 把后台解析累计的缓存立即写入磁盘。
#[tauri::command(async)]
pub fn flush_chem_cache() {
    crate::chem_cache::flush();
}

/// 检查本地路径是否仍然存在且是目录（快速访问可用性提示用）。
#[tauri::command(async)]
pub fn paths_exist(paths: Vec<String>) -> Vec<bool> {
    paths.iter().map(|p| Path::new(p).is_dir()).collect()
}

// ---- 桌面版专有：快速访问与磁盘 ----

/// 默认快速访问项：
/// - Windows：此电脑 + 用户目录；
/// - 其他平台：仅用户目录。
fn default_quick_access(home: &str, windows: bool) -> Vec<QuickAccess> {
    let mut items = Vec::new();
    if windows {
        items.push(QuickAccess {
            name: "此电脑".to_string(),
            path: "::ThisPC".to_string(),
            icon: "computer".to_string(),
            pinned: false,
        });
    }
    if !home.trim().is_empty() {
        items.push(QuickAccess {
            name: "用户目录".to_string(),
            path: home.to_string(),
            icon: "home".to_string(),
            pinned: false,
        });
    }
    items
}

/// 快速访问项（默认项 + 用户右键固定的目录）。
#[tauri::command(async)]
pub fn get_quick_access() -> Vec<QuickAccess> {
    let home = fs::home_dir();
    let mut items = default_quick_access(&home, cfg!(windows));

    // 用户右键固定的目录（持久化在配置目录的 quick_access.json）。
    // 与默认项重复时以默认项为准，避免侧边栏出现两条同名路径。
    let existing: Vec<String> = items.iter().map(|q| quick_access::path_key(&q.path)).collect();
    for item in quick_access::pinned_items(&quick_access::load_pins()) {
        if existing.contains(&quick_access::path_key(&item.path)) {
            continue;
        }
        items.push(item);
    }

    items
}

/// 把文件夹添加到快速访问（已在内置列表或已固定时返回 false）。
#[tauri::command(async)]
pub fn add_quick_access(path: String) -> Result<bool, String> {
    let key = quick_access::path_key(&path);
    if get_quick_access()
        .iter()
        .any(|q| quick_access::path_key(&q.path) == key)
    {
        return Ok(false);
    }
    quick_access::add_pin(&path)
}

/// 从快速访问中取消固定（内置项不可移除，调用时静默成功）。
#[tauri::command(async)]
pub fn remove_quick_access(path: String) -> Result<(), String> {
    quick_access::remove_pin(&path)
}

/// 列出本地磁盘驱动器（Windows）。
///
/// 用 `GetLogicalDrives` 的位掩码枚举，**不要**对每个盘符做 `Path::exists()`：
/// 后者会对断开的网络盘 / 休眠的移动硬盘发起 I/O，可能卡十几秒，导致整个
/// 文件资源管理器“未响应”。
#[tauri::command(async)]
pub fn get_drives() -> Vec<DriveInfo> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
        let mask = unsafe { GetLogicalDrives() };
        let mut drives = Vec::new();
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = (b'A' + i as u8) as char;
            drives.push(DriveInfo {
                letter: letter.to_string(),
                label: format!("本地磁盘 ({letter})"),
                total: None,
                free: None,
            });
        }
        return drives;
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

// ---- 外部终端 ----

/// 用系统终端打开当前目录（本机）或远端目录（SSH 登录后 cd 到该目录）。
///
/// 终端工具来自 设置 → 程序设置（auto / wt / powershell / cmd）。
#[tauri::command(async)]
pub fn open_in_terminal(path: String, remote: Option<RemoteProfile>) -> Result<(), String> {
    let tool = crate::settings::load_settings_inner().terminal_tool;
    let local_cwd = fs::home_dir();
    match remote {
        None => {
            let target = if path.trim().is_empty() || path == "::ThisPC" {
                local_cwd
            } else {
                path
            };
            if !Path::new(&target).is_dir() {
                return Err(format!("目录不存在：{target}"));
            }
            open_local_terminal(&target, &tool)
        }
        Some(profile) => {
            let remote_path = if path.trim().is_empty() { "~".to_string() } else { path };
            open_remote_terminal(&profile, &remote_path, &local_cwd, &tool)
        }
    }
}

fn local_shell_script(cwd: &str) -> String {
    format!("Set-Location -LiteralPath {}", ps_quote(cwd))
}

fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn tool_error(tool: &str) -> String {
    let name = match tool {
        "wt" => "Windows Terminal (wt.exe)",
        "powershell" => "PowerShell",
        "cmd" => "命令提示符 (cmd.exe)",
        _ => "系统终端",
    };
    format!("无法启动 {name}。可在 设置 → 程序设置 中更换终端工具。")
}

/// 远端机器用系统 ssh 打开一个新的终端窗口；密码机器会在窗口里提示输入密码。
fn open_remote_terminal(
    profile: &RemoteProfile,
    remote_path: &str,
    local_cwd: &str,
    tool: &str,
) -> Result<(), String> {
    let port = if profile.port == 0 { 22 } else { profile.port };
    let remote_cmd = format!(
        "cd {} && exec ${{SHELL:-bash}} -l",
        sh_quote_posix(remote_path)
    );
    let target = format!("{}@{}", profile.user.trim(), profile.host.trim());
    #[cfg(windows)]
    {
        let wt_args: Vec<String> = vec![
            "-d".into(),
            local_cwd.into(),
            "ssh".into(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            "-tt".into(),
            "-p".into(),
            port.to_string(),
            target,
            remote_cmd.clone(),
        ];
        // 有 Windows Terminal 就用它承载 ssh：它给子进程真正的 ConPTY stdin，
        // 否则 PowerShell 的 CREATE_NEW_CONSOLE 可能拿不到输入，表现为“键盘没反应”。
        if spawn_wt(&wt_args) {
            return Ok(());
        }
        let ssh_args = format!(
            "-o StrictHostKeyChecking=accept-new -tt -p {} {} {}",
            port,
            format!("{}@{}", profile.user.trim(), profile.host.trim()),
            ps_quote(&remote_cmd)
        );
        let ps_script = format!(
            "Set-Location -LiteralPath {}; ssh {}",
            ps_quote(local_cwd),
            ssh_args
        );
        let cmd_line = format!("cd /d {} && ssh {}", ps_cmd_quote(local_cwd), ssh_args);
        match tool {
            "wt" => {
                if spawn_wt(&wt_args) {
                    Ok(())
                } else {
                    Err(tool_error("wt"))
                }
            }
            "powershell" => {
                if spawn_new_console(
                    "powershell.exe",
                    &["-NoLogo", "-NoExit", "-Command", &ps_script],
                ) {
                    Ok(())
                } else {
                    Err(tool_error("powershell"))
                }
            }
            "cmd" => {
                if spawn_new_console("cmd.exe", &["/K", &cmd_line]) {
                    Ok(())
                } else {
                    Err(tool_error("cmd"))
                }
            }
            _ => {
                if spawn_wt(&wt_args) {
                    return Ok(());
                }
                if spawn_new_console(
                    "powershell.exe",
                    &["-NoLogo", "-NoExit", "-Command", &ps_script],
                ) {
                    return Ok(());
                }
                if spawn_new_console("cmd.exe", &["/K", &cmd_line]) {
                    return Ok(());
                }
                Err(tool_error("auto"))
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (target, remote_cmd, local_cwd, tool);
        Err("当前平台不支持从资源管理器打开终端".to_string())
    }
}

fn open_local_terminal(cwd: &str, tool: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let wt_args = ["-d".to_string(), cwd.to_string()];
        let ps_script = local_shell_script(cwd);
        let cmd_line = format!("cd /d {}", ps_cmd_quote(cwd));
        // 有 Windows Terminal 时总是用它做终端宿主（保留所选的 shell），
        // 这样 stdin/stdout 是真正的 ConPTY，输入不会丢。
        let wt_args: Vec<String> = match tool {
            "powershell" => vec![
                "-d".into(),
                cwd.into(),
                "powershell.exe".into(),
                "-NoLogo".into(),
                "-NoExit".into(),
                "-Command".into(),
                ps_script.clone(),
            ],
            "cmd" => vec![
                "-d".into(),
                cwd.into(),
                "cmd.exe".into(),
                "/K".into(),
                cmd_line.clone(),
            ],
            _ => wt_args.to_vec(),
        };
        if spawn_wt(&wt_args) {
            return Ok(());
        }
        match tool {
            "wt" => {
                if spawn_wt(&wt_args) {
                    Ok(())
                } else {
                    Err(tool_error("wt"))
                }
            }
            "powershell" => {
                if spawn_new_console(
                    "powershell.exe",
                    &["-NoLogo", "-NoExit", "-Command", &ps_script],
                ) {
                    Ok(())
                } else {
                    Err(tool_error("powershell"))
                }
            }
            "cmd" => {
                if spawn_new_console("cmd.exe", &["/K", &cmd_line]) {
                    Ok(())
                } else {
                    Err(tool_error("cmd"))
                }
            }
            _ => {
                if spawn_wt(&wt_args) {
                    return Ok(());
                }
                if spawn_new_console(
                    "powershell.exe",
                    &["-NoLogo", "-NoExit", "-Command", &ps_script],
                ) {
                    return Ok(());
                }
                if spawn_new_console("cmd.exe", &["/K", &cmd_line]) {
                    return Ok(());
                }
                Err(tool_error("auto"))
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (cwd, tool);
        Err("当前平台不支持从资源管理器打开终端".to_string())
    }
}

fn ps_cmd_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

fn sh_quote_posix(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 用 Windows Terminal 打开：`wt.exe -d <cwd> [command...]`。
#[cfg(windows)]
fn find_wt() -> PathBuf {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let base = PathBuf::from(local).join("Microsoft").join("WindowsApps");
        let candidates = [
            base.join("wt.exe"),
            base.join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
                .join("wt.exe"),
        ];
        for candidate in candidates {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("wt.exe")
}

#[cfg(windows)]
fn spawn_wt(args: &[String]) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let spawned = std::process::Command::new(find_wt())
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok();
    if spawned {
        focus_terminal_window_soon();
    }
    spawned
}

#[cfg(windows)]
fn spawn_new_console(program: &str, args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;
    let spawned = std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .is_ok();
    if spawned {
        focus_terminal_window_soon();
    }
    spawned
}

/// 新建终端窗口后，Windows 有时把它放在后台；这里主动把它提到前台，
/// 否则用户会看到窗口却以为键盘没反应（实际焦点还在 wmfold）。
#[cfg(windows)]
fn focus_terminal_window_soon() {
    std::thread::spawn(|| {
        for _ in 0..4 {
            std::thread::sleep(std::time::Duration::from_millis(700));
            if unsafe { focus_terminal_window_once() } {
                return;
            }
        }
    });
}

#[cfg(windows)]
unsafe fn process_image_name(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if handle.is_null() {
        return None;
    }
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size);
    CloseHandle(handle);
    if ok == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..size as usize]))
}

#[cfg(windows)]
unsafe fn is_terminal_window(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let Some(path) = process_image_name(pid) else {
        return false;
    };
    let name = path
        .rsplit(|c| c == '\\' || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "windowsterminal.exe"
            | "openconsole.exe"
            | "conhost.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "cmd.exe"
    )
}

#[cfg(windows)]
unsafe fn focus_terminal_window_once() -> bool {
    use std::ptr;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT, BOOL};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowRect, IsWindowVisible, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    };

    let foreground = GetForegroundWindow();
    if is_terminal_window(foreground) {
        return true;
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return 1;
        }
        if rect.right - rect.left < 200 || rect.bottom - rect.top < 120 {
            return 1;
        }
        if is_terminal_window(hwnd) {
            *(lparam as *mut HWND) = hwnd;
            return 0;
        }
        1
    }

    let mut found: HWND = ptr::null_mut();
    EnumWindows(Some(enum_proc), &mut found as *mut HWND as LPARAM);
    if found.is_null() {
        return false;
    }
    ShowWindow(found, SW_RESTORE);
    SetForegroundWindow(found);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_defaults_are_this_pc_and_home() {
        let items = default_quick_access("C:\\Users\\me", true);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].path, "::ThisPC");
        assert_eq!(items[1].name, "用户目录");
        assert_eq!(items[1].path, "C:\\Users\\me");
        assert_eq!(items[1].icon, "home");
    }

    #[test]
    fn linux_defaults_only_home() {
        let items = default_quick_access("/home/me", false);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "用户目录");
        assert_eq!(items[0].path, "/home/me");
    }

    #[test]
    fn empty_home_is_skipped() {
        let items = default_quick_access("   ", true);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, "::ThisPC");
    }
}
