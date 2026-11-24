mod ai;
mod chem_cache;
mod commands;
mod quick_access;
mod remotes;
mod scan;
mod secret;
mod settings;
mod shell_menu;
mod windows;

use tauri::{Emitter, Manager};

/// 从命令行参数中提取「通过 wmfold 打开」的目录。
///
/// 资源管理器右键菜单使用 `wmfold.exe --open "%V"`；也兼容直接把目录作为参数。
pub fn open_path_from_args(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--open" {
            return iter.next().cloned().filter(|p| !p.trim().is_empty());
        }
        if let Some(path) = arg.strip_prefix("--open=") {
            return Some(path.to_string()).filter(|p| !p.trim().is_empty());
        }
    }
    // 兼容 `wmfold.exe D:\some\folder`（只接受真实存在的目录）
    args.iter()
        .find(|arg| {
            !arg.starts_with('-')
                && std::path::Path::new(arg).is_dir()
        })
        .cloned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例：第二次启动时聚焦已有窗口，避免两个进程同时读写 WebView2 / 配置
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // 从资源管理器右键菜单再次打开时，让已有窗口导航到目标目录
            if let Some(path) = open_path_from_args(&argv) {
                let _ = app.emit("open-path", path);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        // Tauri 2 自动更新：检查 / 下载 / 安装，安装后由 process 插件重启
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|_app| {
            #[cfg(windows)]
            if let Err(err) = shell_menu::register_context_menu() {
                eprintln!("wmfold: 注册资源管理器右键菜单失败: {err}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_dir,
            commands::get_home_dir,
            commands::take_launch_path,
            commands::get_quick_access,
            commands::paths_exist,
            commands::add_quick_access,
            commands::remove_quick_access,
            commands::get_drives,
            commands::create_folder,
            commands::create_file,
            commands::rename_path,
            commands::delete_paths,
            commands::copy_paths,
            commands::move_paths,
            commands::open_path,
            commands::open_in_explorer,
            commands::read_chem,
            commands::read_chem_batch,
            commands::flush_chem_cache,
            commands::read_structure,
            commands::open_in_terminal,
            scan::scan_recursive,
            scan::cancel_scan,
            ai::ai_config,
            ai::ai_save_config,
            ai::ai_test,
            ai::ai_chat,
            settings::load_settings,
            settings::save_settings,
            remotes::remote_profiles,
            remotes::remote_save_profile,
            remotes::remote_delete_profile,
            remotes::remote_check,
            remotes::remote_install,
            remotes::remote_list,
            remotes::remote_info,
            remotes::remote_structure,
            remotes::remote_home,
            remotes::remote_quick_access,
            remotes::remote_add_quick_access,
            remotes::remote_remove_quick_access,
            remotes::remote_paths_exist,
            remotes::remote_upload,
            remotes::remote_download,
            remotes::remote_copy_within,
            remotes::remote_delete,
            remotes::remote_transfer,
            remotes::submit_job,
            remotes::remote_job_query,
            windows::window_at_point,
            windows::merge_machine_window,
            windows::register_machine_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
