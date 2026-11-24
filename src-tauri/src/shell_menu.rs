//! Windows 资源管理器右键菜单集成。
//!
//! 在「文件夹空白处右键」菜单里注册「通过 wmfold 打开」，写入
//! `HKCU\Software\Classes\Directory\Background\shell\wmfold`（当前用户，无需管理员）。
//! 每次启动都按当前 exe 路径重写，便携版移动位置后也能自动更新。

#[cfg(windows)]
pub fn register_context_menu() -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let exe = std::env::current_exe().map_err(|e| format!("无法获取 wmfold 路径: {e}"))?;
    let exe = exe.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Classes\Directory\Background\shell\wmfold")
        .map_err(|e| format!("创建右键菜单注册表项失败: {e}"))?;
    key.set_value("", &"通过 wmfold 打开")
        .map_err(|e| format!("写入右键菜单名称失败: {e}"))?;
    key.set_value("Icon", &format!("{exe},0"))
        .map_err(|e| format!("写入右键菜单图标失败: {e}"))?;
    let (command, _) = key
        .create_subkey("command")
        .map_err(|e| format!("创建右键菜单命令项失败: {e}"))?;
    command
        .set_value("", &format!("\"{exe}\" --open \"%V\""))
        .map_err(|e| format!("写入右键菜单命令失败: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn register_context_menu() -> Result<(), String> {
    Ok(())
}
