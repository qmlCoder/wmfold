mod chem;
mod fs_cmds;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            fs_cmds::list_dir,
            fs_cmds::get_home_dir,
            fs_cmds::get_quick_access,
            fs_cmds::get_drives,
            fs_cmds::create_folder,
            fs_cmds::create_file,
            fs_cmds::rename_path,
            fs_cmds::delete_paths,
            fs_cmds::copy_paths,
            fs_cmds::move_paths,
            fs_cmds::open_path,
            chem::read_chem,
            chem::read_chem_batch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
