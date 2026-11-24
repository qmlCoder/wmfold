//! WMFold 共享核心。
//!
//! 这里放与界面无关的逻辑，供两个前端复用：
//! - `src-tauri/`：Windows 桌面版（Tauri 2 + Vue）
//! - `wmfold-cli/`：机器接口（JSON，桌面端远程浏览与脚本共用）
//!
//! 模块：
//! - [`models`]：`FileEntry` / `ChemMeta` 等数据结构
//! - [`chem`]：化学文件解析（复用 `rswfn`，含安全防护，SAFETY-CRITICAL）
//! - [`fs`]：目录列表与常规文件操作
//! - [`columns`]：化学列定义与各文件类型可用列（后端设置校验共用）
//!
//! 注意：本 crate **不能**依赖 Tauri、GPUI 等界面框架，也不应引入任何平台专属
//! 的 GUI 依赖，否则服务器（无 WebView / 无桌面环境）上无法编译。

pub mod chem;
pub mod columns;
pub mod fs;
pub mod models;

pub use models::{ChemMeta, DriveInfo, FileEntry, MolAtom, MolStructure, QuickAccess};
