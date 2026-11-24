//! 远程服务器（SSH）支持。
//!
//! 通过系统自带的 OpenSSH 客户端（`ssh`）连接服务器：
//! - 检查服务器环境、是否已安装 `wmfold`；
//! - 把随包的 Linux 版 `wmfold` 上传到 `~/.local/bin/wmfold` 并 `chmod +x`；
//! - 调用远端 `wmfold -cli list/info`（JSON）取回文件信息。
//!
//! 认证只使用用户已有的 SSH 密钥 / ssh-agent（`BatchMode=yes`）；不处理密码输入。
//! 首次连接会用 `StrictHostKeyChecking=accept-new` 自动接受新主机的 host key。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use wmfold_core::models::{FileEntry, MolStructure, QuickAccess};

use crate::quick_access;
use crate::settings::config_dir;

/// 与 `wmfold -cli` 约定的 JSON 协议版本。
const PROTOCOL: u32 = 1;

/// Windows 下隐藏子进程的控制台窗口。
///
/// release 是 GUI 子系统；直接启动 ssh / plink / tar 这类 console 程序时，
/// 每次远程操作都会弹出一个终端窗口。所有非“在终端中打开”的子进程都必须隐藏。
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// 远程服务器配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProfile {
    /// 唯一标识（新增时留空，由后端生成）
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub port: u16,
    pub user: String,
    /// 私钥路径（可选；留空则使用 ssh-agent / ~/.ssh/config）
    #[serde(default)]
    pub identity_file: Option<String>,
    /// 密码（可选；填写后优先使用密码登录，通过随包的 plink 执行）
    #[serde(default)]
    pub password: Option<String>,
    /// 首次密码登录时自动探测并缓存的主机密钥指纹（SHA256:...）
    #[serde(default)]
    pub host_key: Option<String>,
    /// 是否在右侧面板显示远程作业查询
    #[serde(default)]
    pub job_query_enabled: bool,
    /// 作业查询命令，例如 `squeue -u $USER`
    #[serde(default)]
    pub job_query_command: String,
    /// 自动刷新间隔（秒）
    #[serde(default)]
    pub job_query_interval: u32,
    /// 提交作业命令模板；必须包含 `$PATH` 占位符，留空表示这台机器不参与提交
    #[serde(default)]
    pub submit_command: String,
    /// 默认提交目录（未打开目标机器文件夹时使用）
    #[serde(default = "default_submit_dir")]
    pub submit_dir: String,
    /// 提交 gjf 时是否把 `%chk=/path/name.chk` 改成 `%chk=name.chk`
    #[serde(default)]
    pub clear_chk_path: bool,
}

fn default_submit_dir() -> String {
    "~/wmfold".to_string()
}

impl RemoteProfile {
    fn normalized(mut self) -> Self {
        // 去掉输入框里可能带进来的首尾空白（含粘贴产生的不可见空格）
        self.name = self.name.trim().to_string();
        self.host = self.host.trim().to_string();
        self.user = self.user.trim().to_string();
        self.identity_file = self
            .identity_file
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        // 密码原样保留（首尾空格也可能是密码的一部分），只把空串归一为 None
        self.password = self.password.filter(|s| !s.is_empty());
        self.host_key = self
            .host_key
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self.job_query_command = self.job_query_command.trim().to_string();
        if self.job_query_command.is_empty() {
            self.job_query_command = "squeue -u $USER".to_string();
        }
        self.job_query_interval = if self.job_query_interval == 0 {
            10
        } else {
            self.job_query_interval.clamp(5, 600)
        };
        self.submit_command = self.submit_command.trim().to_string();
        self.submit_dir = self.submit_dir.trim().to_string();
        if self.submit_dir.is_empty() {
            self.submit_dir = default_submit_dir();
        }
        if self.port == 0 {
            self.port = 22;
        }
        if self.id.trim().is_empty() {
            self.id = format!("{}@{}:{}", self.user, self.host, self.port);
        }
        self
    }

    fn target(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }
}

/// 连接检查结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub os: String,
    pub home: String,
    pub installed: bool,
    pub wmfold_version: Option<String>,
    pub binary_path: Option<String>,
    /// 服务器上的 wmfold 版本是否低于随包版本（需要重新上传）
    pub outdated: bool,
}

/// `wmfold -cli list` 的返回（桌面端只关心 path + entries）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteListing {
    pub protocol: u32,
    pub path: String,
    pub entries: Vec<FileEntry>,
}

/// `wmfold -cli info` 的返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfoResponse {
    pub protocol: u32,
    pub entry: FileEntry,
}

/// `wmfold -cli structure` 的返回。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStructureResponse {
    pub protocol: u32,
    pub structure: MolStructure,
}

// ---- 配置持久化 ----

/// `%APPDATA%\wmfold\remotes.json`。
pub fn profiles_path() -> PathBuf {
    config_dir().join("remotes.json")
}

/// 校验 + 去重（纯函数，便于测试）。
pub fn sanitize(mut list: Vec<RemoteProfile>) -> Vec<RemoteProfile> {
    list.retain(|p| !p.host.trim().is_empty() && !p.user.trim().is_empty());
    let mut seen = std::collections::HashSet::new();
    list.into_iter()
        .map(RemoteProfile::normalized)
        .filter(|p| seen.insert(p.id.clone()))
        .collect()
}

// ---- 密码加密（Windows DPAPI，仅当前用户可解密） ----
// 具体实现放在 crate::secret，AI 的 API Key 也复用同一套加密。

use crate::secret::{protect as protect_password, unprotect as unprotect_password};

pub fn load_from(path: &Path) -> Vec<RemoteProfile> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mut list = serde_json::from_str::<Vec<RemoteProfile>>(&text)
                .map(sanitize)
                .unwrap_or_default();
            for profile in &mut list {
                if let Some(stored) = profile.password.take() {
                    profile.password = Some(unprotect_password(&stored));
                }
            }
            list
        }
        Err(_) => Vec::new(),
    }
}

pub fn save_to(path: &Path, list: &[RemoteProfile]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // 密码用 Windows DPAPI 加密后再落盘（只有当前用户能解密）
    let stored: Vec<RemoteProfile> = list
        .iter()
        .cloned()
        .map(|mut profile| {
            if let Some(password) = profile.password.take() {
                let encrypted = protect_password(&password);
                profile.password = if encrypted.is_empty() {
                    None
                } else {
                    Some(encrypted)
                };
            }
            profile
        })
        .collect();
    let text = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

pub fn load() -> Vec<RemoteProfile> {
    load_from(&profiles_path())
}

pub fn save(list: &[RemoteProfile]) -> Result<(), String> {
    save_to(&profiles_path(), list)
}

pub fn upsert(profile: RemoteProfile) -> Result<Vec<RemoteProfile>, String> {
    if profile.host.trim().is_empty() || profile.user.trim().is_empty() {
        return Err("host 和 user 不能为空".to_string());
    }
    let profile = profile.normalized();
    let mut list = load();
    match list.iter_mut().find(|p| p.id == profile.id) {
        Some(existing) => {
            // 前端不回填密码/主机密钥时表示“保持不变”，避免编辑其它字段把密码清掉
            let mut next = profile;
            if next.password.is_none() {
                next.password = existing.password.clone();
            }
            if next.host_key.is_none() {
                next.host_key = existing.host_key.clone();
            }
            // 主机/端口变了，旧指纹不再适用，下次连接重新探测
            if next.host != existing.host || next.port != existing.port {
                next.host_key = None;
            }
            *existing = next;
        }
        None => list.push(profile),
    }
    let list = sanitize(list);
    save(&list)?;
    Ok(list)
}

pub fn remove(id: &str) -> Result<Vec<RemoteProfile>, String> {
    let mut list = load();
    list.retain(|p| p.id != id);
    save(&list)?;
    Ok(list)
}

// ---- SSH 执行 ----

/// 随包的 plink（PuTTY）用于密码登录；Windows 自带 OpenSSH 无法非交互传密码。
const PLINK_BIN: &[u8] = include_bytes!("../resources/plink.exe");
static PASSWORD_FILE_SEQ: AtomicU64 = AtomicU64::new(1);

fn plink_path() -> Result<PathBuf, String> {
    #[cfg(not(windows))]
    {
        Err("密码登录目前仅支持 Windows 桌面版".to_string())
    }
    #[cfg(windows)]
    {
        let path = config_dir().join("plink.exe");
        let stale = match std::fs::metadata(&path) {
            Ok(meta) => meta.len() != PLINK_BIN.len() as u64,
            Err(_) => true,
        };
        if stale {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("创建 plink 目录失败: {e}"))?;
            }
            std::fs::write(&path, PLINK_BIN)
                .map_err(|e| format!("写入 plink 失败: {e}"))?;
        }
        Ok(path)
    }
}

fn write_password_file(password: &str) -> Result<PathBuf, String> {
    let seq = PASSWORD_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "wmfold-pw-{}-{seq}.txt",
        std::process::id()
    ));
    std::fs::write(&path, password.as_bytes())
        .map_err(|e| format!("写入临时密码文件失败: {e}"))?;
    Ok(path)
}

/// 解析 plink 首次连接时打印的主机密钥指纹。
fn extract_host_key(stderr: &str) -> Option<String> {
    let start = stderr.find("SHA256:")?;
    let rest = &stderr[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\r' || c == '\n')
        .unwrap_or(rest.len());
    let fingerprint = &rest[..end];
    if fingerprint.len() > "SHA256:".len() {
        Some(fingerprint.to_string())
    } else {
        None
    }
}

/// 第一次密码登录时 plink 会拒绝未知主机密钥，但会把指纹打印到 stderr；
/// 取到指纹后缓存进 profile，后续命令用 `-hostkey` 接受。
fn probe_host_key(profile: &RemoteProfile) -> Result<String, String> {
    let plink = plink_path()?;
    let password = profile
        .password
        .as_deref()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "密码为空".to_string())?;
    let pw_file = write_password_file(password)?;
    let port = if profile.port == 0 { 22 } else { profile.port };
    let output = {
        let mut cmd = Command::new(&plink);
        cmd.arg("-batch")
            .arg("-ssh")
            .arg("-P")
            .arg(port.to_string())
            .arg("-l")
            .arg(profile.user.trim())
            .arg("-pwfile")
            .arg(&pw_file)
            .arg(profile.host.trim())
            .arg("exit");
        hide_console(&mut cmd);
        cmd.output()
    };
    let _ = std::fs::remove_file(&pw_file);
    let output = output.map_err(|e| format!("无法启动 plink: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(fingerprint) = extract_host_key(&stderr) {
        return Ok(fingerprint);
    }
    Err(format!(
        "无法获取服务器主机密钥: {}",
        stderr.trim()
    ))
}

fn remember_host_key(profile: &RemoteProfile, fingerprint: &str) {
    let mut list = load();
    if let Some(existing) = list.iter_mut().find(|p| p.id == profile.id) {
        existing.host_key = Some(fingerprint.to_string());
        let _ = save(&list);
    }
}

/// 一个待执行的 SSH 命令；密码模式下临时密码文件随对象销毁自动删除。
struct RemoteCommand {
    cmd: Command,
    pw_file: Option<PathBuf>,
}

impl Drop for RemoteCommand {
    fn drop(&mut self) {
        if let Some(path) = self.pw_file.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl std::ops::Deref for RemoteCommand {
    type Target = Command;
    fn deref(&self) -> &Self::Target {
        &self.cmd
    }
}

impl std::ops::DerefMut for RemoteCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cmd
    }
}

struct Ssh {
    profile: RemoteProfile,
}

impl Ssh {
    fn new(profile: RemoteProfile) -> Result<Self, String> {
        let mut profile = profile.normalized();
        let uses_password = profile
            .password
            .as_deref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        if uses_password && profile.host_key.is_none() {
            // 前端手里的 profile 可能还是首次连接前的旧对象，先看磁盘上是否已缓存指纹
            if let Some(saved) = load().into_iter().find(|p| !p.id.is_empty() && p.id == profile.id)
            {
                profile.host_key = saved.host_key;
            }
        }
        if uses_password && profile.host_key.is_none() {
            let fingerprint = probe_host_key(&profile)?;
            remember_host_key(&profile, &fingerprint);
            profile.host_key = Some(fingerprint);
        }
        Ok(Self { profile })
    }

    fn uses_password(&self) -> bool {
        self.profile
            .password
            .as_deref()
            .map(|p| !p.is_empty())
            .unwrap_or(false)
    }

    /// 构造 SSH 命令（已经带上目标主机与远端命令）。
    fn build(&self, remote_cmd: &str) -> Result<RemoteCommand, String> {
        let port = if self.profile.port == 0 {
            22
        } else {
            self.profile.port
        };
        if self.uses_password() {
            #[cfg(not(windows))]
            {
                let _ = (port, remote_cmd);
                return Err("密码登录目前仅支持 Windows 桌面版".to_string());
            }
            #[cfg(windows)]
            {
                let password = self.profile.password.as_deref().unwrap_or_default();
                let pw_file = write_password_file(password)?;
                let mut cmd = Command::new(plink_path()?);
                cmd.arg("-batch");
                cmd.arg("-ssh");
                cmd.arg("-P");
                cmd.arg(port.to_string());
                cmd.arg("-l");
                cmd.arg(self.profile.user.trim());
                if let Some(fingerprint) = self.profile.host_key.as_deref() {
                    if !fingerprint.is_empty() {
                        cmd.arg("-hostkey");
                        cmd.arg(fingerprint);
                    }
                }
                cmd.arg("-pwfile");
                cmd.arg(&pw_file);
                cmd.arg(self.profile.host.trim());
                cmd.arg(remote_cmd);
                hide_console(&mut cmd);
                Ok(RemoteCommand {
                    cmd,
                    pw_file: Some(pw_file),
                })
            }
        } else {
            let mut cmd = Command::new("ssh");
            cmd.arg("-o");
            cmd.arg("BatchMode=yes");
            cmd.arg("-o");
            cmd.arg("ConnectTimeout=10");
            cmd.arg("-o");
            cmd.arg("StrictHostKeyChecking=accept-new");
            if let Some(id) = self
                .profile
                .identity_file
                .as_ref()
                .filter(|s| !s.trim().is_empty())
            {
                cmd.arg("-i");
                cmd.arg(id.trim());
            }
            cmd.arg("-p");
            cmd.arg(port.to_string());
            cmd.arg(self.profile.target());
            cmd.arg(remote_cmd);
            hide_console(&mut cmd);
            Ok(RemoteCommand { cmd, pw_file: None })
        }
    }

    /// 在服务器上执行一条 shell 命令，返回 stdout（已 trim）。
    fn run(&self, remote_cmd: &str) -> Result<String, String> {
        let output = self
            .build(remote_cmd)?
            .output()
            .map_err(|e| format!("无法启动 SSH 客户端: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr.trim();
            return Err(if msg.is_empty() {
                format!("SSH 退出码 {:?}", output.status.code())
            } else {
                msg.to_string()
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// 通过 SSH 管道把本地文件写入远端固定位置。
    ///
    /// 不用 `scp`：Windows 版 scp 对本地路径 / 目标参数的解析在部分环境下会报
    /// “hostname contains invalid characters”，ssh + `cat` 更稳定。
    fn upload(&self, local: &Path) -> Result<(), String> {
        use std::io::Write as _;
        use std::process::Stdio;

        let data = std::fs::read(local).map_err(|e| {
            format!(
                "读取本地服务器二进制失败（{}）: {e}",
                local.display()
            )
        })?;

        let script = "mkdir -p \"$HOME/.local/bin\" && \
                      cat > \"$HOME/.local/bin/wmfold\" && \
                      chmod 755 \"$HOME/.local/bin/wmfold\"";
        let mut command = self.build(script)?;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("无法启动 SSH 上传: {e}"))?;

        {
            let mut stdin = child.stdin.take().ok_or("无法打开 SSH stdin")?;
            stdin
                .write_all(&data)
                .map_err(|e| format!("写入 SSH 上传失败: {e}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("SSH 上传失败: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "上传 wmfold 到服务器失败: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    /// 执行远端命令并返回原始输出（不因非零退出码报错）。
    fn run_raw(&self, remote_cmd: &str) -> Result<std::process::Output, String> {
        self.build(remote_cmd)?
            .output()
            .map_err(|e| format!("无法启动 SSH 客户端: {e}"))
    }

    /// 通过 SSH 管道把本地文件写到远端（stdin），用于 tar 流上传。
    fn ssh_with_stdin_file(&self, remote_cmd: &str, file: &Path) -> Result<(), String> {
        use std::process::Stdio;
        let input = std::fs::File::open(file)
            .map_err(|e| format!("打开本地临时文件失败: {e}"))?;
        let mut command = self.build(remote_cmd)?;
        let child = command
            .stdin(Stdio::from(input))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("无法启动 SSH 上传: {e}"))?;
        let output = child
            .wait_with_output()
            .map_err(|e| format!("SSH 上传失败: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "远端写入失败: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    /// 把远端命令的 stdout（tar 流）保存到本地临时文件。
    fn ssh_to_file(&self, remote_cmd: &str, out: &Path) -> Result<(), String> {
        use std::process::Stdio;
        let file =
            std::fs::File::create(out).map_err(|e| format!("创建本地临时文件失败: {e}"))?;
        let mut command = self.build(remote_cmd)?;
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::from(file))
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("无法启动 SSH 下载: {e}"))?;
        let output = child
            .wait_with_output()
            .map_err(|e| format!("SSH 下载失败: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "远端读取失败: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    /// 把一个本地文件 / 目录（tar 打包后）上传到远端目录。
    fn upload_path(&self, local: &Path, remote_dest: &str) -> Result<(), String> {
        let parent = local
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = local
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "无法确定本地文件名".to_string())?;
        let archive = temp_archive("up");
        let dest = sh_quote(remote_dest);
        let script = format!("mkdir -p {dest} && tar -xf - -C {dest}");
        let result = (|| {
            tar_create(&archive, parent, name)?;
            self.ssh_with_stdin_file(&script, &archive)
        })();
        let _ = std::fs::remove_file(&archive);
        result
    }

    /// 把远端文件 / 目录（tar 流）下载并解压到本地目录。
    fn download_path(&self, remote: &str, local_dest: &str) -> Result<(), String> {
        let parent = posix_parent(remote);
        let name = posix_name(remote)?;
        let archive = temp_archive("down");
        let script = format!(
            "tar -cf - -C {} -- {}",
            sh_quote(&parent),
            sh_quote(&name)
        );
        let result = (|| {
            self.ssh_to_file(&script, &archive)?;
            tar_extract(&archive, local_dest)
        })();
        let _ = std::fs::remove_file(&archive);
        result
    }

    /// 同一台远端机器内部复制 / 移动。
    fn copy_within(&self, paths: &[String], dest: &str, move_mode: bool) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        let dest = sh_quote(dest);
        let list: Vec<String> = paths.iter().map(|p| sh_quote(p)).collect();
        let cmd = if move_mode { "mv" } else { "cp -a" };
        self.run(&format!(
            "mkdir -p {dest} && {cmd} -- {} {dest}",
            list.join(" ")
        ))?;
        Ok(())
    }

    /// 删除远端路径（用于跨设备“剪切”后的源清理）。
    fn delete_paths(&self, paths: &[String]) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        if paths.iter().any(|p| p.trim().is_empty() || p.trim() == "/") {
            return Err("拒绝删除远端根目录".to_string());
        }
        let list: Vec<String> = paths.iter().map(|p| sh_quote(p)).collect();
        self.run(&format!("rm -rf -- {}", list.join(" ")))?;
        Ok(())
    }
}

/// POSIX 路径的父目录。
fn posix_parent(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => trimmed[..i].to_string(),
        None => ".".to_string(),
    }
}

/// POSIX 路径的最后一段（不允许根目录）。
fn posix_name(path: &str) -> Result<String, String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("不能传输根目录".to_string());
    }
    let name = trimmed.rsplit('/').next().unwrap_or("");
    if name.is_empty() {
        Err("无法确定远端文件名".to_string())
    } else {
        Ok(name.to_string())
    }
}

/// 唯一临时 tar 文件路径。
fn temp_archive(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("wmfold-{tag}-{}-{id}.tar", std::process::id()))
}

/// 唯一临时目录路径。
fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("wmfold-{tag}-{}-{id}", std::process::id()))
}

/// 用系统 tar 打包单个文件 / 目录（Windows 10+ 自带 bsdtar）。
fn tar_create(archive: &Path, base: &Path, name: &str) -> Result<(), String> {
    let mut cmd = Command::new("tar");
    cmd.arg("-cf")
        .arg(archive)
        .arg("-C")
        .arg(base)
        .arg("--")
        .arg(name);
    hide_console(&mut cmd);
    let status = cmd
        .status()
        .map_err(|e| format!("无法启动 tar（Windows 10+ 自带）: {e}"))?;
    if !status.success() {
        return Err(format!("tar 打包失败（{}）", name));
    }
    Ok(())
}

/// 用系统 tar 解压到目标目录。
fn tar_extract(archive: &Path, dest: &str) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("创建目标目录失败: {e}"))?;
    let mut cmd = Command::new("tar");
    cmd.arg("-xf").arg(archive).arg("-C").arg(dest);
    hide_console(&mut cmd);
    let status = cmd
        .status()
        .map_err(|e| format!("无法启动 tar（Windows 10+ 自带）: {e}"))?;
    if !status.success() {
        return Err("tar 解压失败".to_string());
    }
    Ok(())
}

/// 远端调用 wmfold 的 shell 前缀：优先用 PATH 中的，其次用安装位置。
fn remote_wmfold_prefix() -> &'static str {
    "WM=$(command -v wmfold 2>/dev/null || echo \"$HOME/.local/bin/wmfold\"); \
     if [ ! -x \"$WM\" ] && [ ! -f \"$WM\" ]; then echo 'wmfold is not installed on the server' >&2; exit 127; fi; "
}

/// POSIX shell 单引号转义。
pub(crate) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 连接检查：OS、home、是否已安装 wmfold 及其版本。
#[tauri::command(async)]
pub fn remote_check(profile: RemoteProfile) -> Result<RemoteStatus, String> {
    let ssh = Ssh::new(profile)?;
    let script = "printf 'OS=%s\\n' \"$(uname -sr 2>/dev/null || echo unknown)\"; \
                  printf 'HOME=%s\\n' \"$HOME\"; \
                  WM=$(command -v wmfold 2>/dev/null || echo \"$HOME/.local/bin/wmfold\"); \
                  if [ -x \"$WM\" ] || [ -f \"$WM\" ]; then printf 'WM=%s\\n' \"$WM\"; \"$WM\" -cli version 2>/dev/null || true; fi";
    let output = ssh.run(script)?;

    let mut os = String::new();
    let mut home = String::new();
    let mut binary_path: Option<String> = None;
    let mut wmfold_version: Option<String> = None;
    for line in output.lines() {
        if let Some(v) = line.strip_prefix("OS=") {
            os = v.to_string();
        } else if let Some(v) = line.strip_prefix("HOME=") {
            home = v.to_string();
        } else if let Some(v) = line.strip_prefix("WM=") {
            binary_path = Some(v.to_string());
        } else if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("protocol").and_then(|p| p.as_u64()) == Some(PROTOCOL as u64) {
                    wmfold_version = v
                        .get("version")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                }
            }
        }
    }

    Ok(RemoteStatus {
        os,
        home,
        installed: wmfold_version.is_some() || binary_path.is_some(),
        outdated: wmfold_version.as_deref() != Some(wmfold_cli::SERVER_VERSION),
        wmfold_version,
        binary_path,
    })
}

/// 在服务器上安装 wmfold（上传随包的 Linux 二进制到 `~/.local/bin/wmfold`）。
#[tauri::command(async)]
pub fn remote_install(app: tauri::AppHandle, profile: RemoteProfile) -> Result<String, String> {
    let ssh = Ssh::new(profile.clone())?;

    // 已安装且版本与随包版本一致才复用；旧版本需要重新上传（例如新增 structure 命令）
    if let Ok(status) = remote_check(profile) {
        if status.wmfold_version.as_deref() == Some(wmfold_cli::SERVER_VERSION) {
            return Ok(format!(
                "服务器上已安装 wmfold {}",
                status.wmfold_version.unwrap_or_default()
            ));
        }
    }

    let local = server_binary(&app).ok_or_else(|| {
        "找不到随包的 Linux 版 wmfold 二进制。请在服务器上自行构建（cargo build --release）\
         并放到 ~/.local/bin/wmfold，或设置环境变量 WMFOLD_SERVER_BIN 指向本地 Linux 二进制。"
            .to_string()
    })?;

    ssh.run("mkdir -p \"$HOME/.local/bin\"")?;
    ssh.upload(&local)?;
    ssh.run("chmod 755 \"$HOME/.local/bin/wmfold\"")?;

    let out = ssh.run(&format!(
        "{}\"$WM\" -cli version",
        remote_wmfold_prefix()
    ))?;
    let version = serde_json::from_str::<serde_json::Value>(&out)
        .ok()
        .and_then(|v| v.get("version").and_then(|s| s.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    Ok(format!("已安装 wmfold {version} 到 ~/.local/bin/wmfold"))
}

fn server_binary(app: &tauri::AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("WMFOLD_SERVER_BIN") {
        if !p.trim().is_empty() {
            candidates.push(PathBuf::from(p));
        }
    }
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("wmfold-server"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("wmfold-server"));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("resources").join("wmfold-server"));
    candidates.push(
        manifest
            .parent()
            .unwrap_or(&manifest)
            .join("wmfold-cli")
            .join("target")
            .join("x86_64-unknown-linux-musl")
            .join("release")
            .join("wmfold-cli"),
    );
    candidates.into_iter().find(|p| p.is_file())
}

/// 列出远端目录（JSON 来自 `wmfold -cli list`）。
#[tauri::command(async)]
pub fn remote_list(
    profile: RemoteProfile,
    path: String,
    chem: bool,
) -> Result<RemoteListing, String> {
    let ssh = Ssh::new(profile)?;
    let chem_flag = if chem { " --chem" } else { "" };
    let script = format!(
        "{}\"$WM\" -cli list {}{}",
        remote_wmfold_prefix(),
        sh_quote(&path),
        chem_flag
    );
    let output = ssh.run(&script)?;
    let listing: RemoteListing = serde_json::from_str(&output)
        .map_err(|e| format!("解析远端 wmfold 输出失败: {e}"))?;
    if listing.protocol != PROTOCOL {
        return Err(format!(
            "远端 wmfold 协议版本 {} 与本地 {PROTOCOL} 不兼容，请重新安装/更新服务器上的 wmfold",
            listing.protocol
        ));
    }
    Ok(listing)
}

/// 远端单个文件信息（含化学元数据）。
#[tauri::command(async)]
pub fn remote_info(profile: RemoteProfile, path: String) -> Result<FileEntry, String> {
    let ssh = Ssh::new(profile)?;
    let script = format!(
        "{}\"$WM\" -cli info {} --chem",
        remote_wmfold_prefix(),
        sh_quote(&path)
    );
    let output = ssh.run(&script)?;
    let response: RemoteInfoResponse = serde_json::from_str(&output)
        .map_err(|e| format!("解析远端 wmfold 输出失败: {e}"))?;
    if response.protocol != PROTOCOL {
        return Err(format!(
            "远端 wmfold 协议版本 {} 与本地 {PROTOCOL} 不兼容，请重新安装/更新服务器上的 wmfold",
            response.protocol
        ));
    }
    Ok(response.entry)
}

/// 远端分子三维结构（`wmfold -cli structure`，原子坐标由服务器端解析后传回）。
#[tauri::command(async)]
pub fn remote_structure(
    profile: RemoteProfile,
    path: String,
) -> Result<MolStructure, String> {
    let ssh = Ssh::new(profile)?;
    let script = format!(
        "{}\"$WM\" -cli structure {}",
        remote_wmfold_prefix(),
        sh_quote(&path)
    );
    let output = ssh.run(&script)?;
    let response: RemoteStructureResponse = serde_json::from_str(&output)
        .map_err(|e| format!("解析远端 wmfold structure 输出失败: {e}"))?;
    if response.protocol != PROTOCOL {
        return Err(format!(
            "远端 wmfold 协议版本 {} 与本地 {PROTOCOL} 不兼容，请重新安装/更新服务器上的 wmfold",
            response.protocol
        ));
    }
    Ok(response.structure)
}

/// 某台远程机器已固定的目录（侧边栏「快速访问」用）。
#[tauri::command(async)]
pub fn remote_quick_access(machine: String) -> Vec<QuickAccess> {
    quick_access::remote_pinned_items(&quick_access::load_remote_pins(&machine))
}

/// 把远端目录添加到该机器的快速访问。
#[tauri::command(async)]
pub fn remote_add_quick_access(machine: String, path: String) -> Result<bool, String> {
    quick_access::add_remote_pin(&machine, &path)
}

/// 从该机器的快速访问中移除远端目录。
#[tauri::command(async)]
pub fn remote_remove_quick_access(machine: String, path: String) -> Result<(), String> {
    quick_access::remove_remote_pin(&machine, &path)
}

/// 远端用户主目录。
#[tauri::command(async)]
pub fn remote_home(profile: RemoteProfile) -> Result<String, String> {
    Ssh::new(profile)?.run("printf '%s' \"$HOME\"")
}

// ---- Tauri 命令：跨设备复制 / 剪切 ----

/// 本地 → 远程：把本地文件 / 目录（tar 流）上传到远端目录。
#[tauri::command(async)]
pub fn remote_upload(
    profile: RemoteProfile,
    sources: Vec<String>,
    dest_dir: String,
) -> Result<(), String> {
    if sources.is_empty() {
        return Ok(());
    }
    let ssh = Ssh::new(profile)?;
    for source in sources {
        ssh.upload_path(Path::new(&source), &dest_dir)
            .map_err(|e| format!("上传 {source} 失败: {e}"))?;
    }
    Ok(())
}

/// 远程 → 本地：把远端文件 / 目录下载到本地目录。
#[tauri::command(async)]
pub fn remote_download(
    profile: RemoteProfile,
    sources: Vec<String>,
    dest_dir: String,
) -> Result<(), String> {
    if sources.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("创建本地目标目录失败: {e}"))?;
    let ssh = Ssh::new(profile)?;
    for source in sources {
        ssh.download_path(&source, &dest_dir)
            .map_err(|e| format!("下载 {source} 失败: {e}"))?;
    }
    Ok(())
}

/// 同一台远程机器内复制 / 移动。
#[tauri::command(async)]
pub fn remote_copy_within(
    profile: RemoteProfile,
    sources: Vec<String>,
    dest_dir: String,
    move_mode: bool,
) -> Result<(), String> {
    Ssh::new(profile)?.copy_within(&sources, &dest_dir, move_mode)
}

/// 检查远端路径是否仍然存在且是目录（远程快速访问可用性提示用）。
#[tauri::command(async)]
pub fn remote_paths_exist(
    profile: RemoteProfile,
    paths: Vec<String>,
) -> Result<Vec<bool>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let checks: Vec<String> = paths
        .iter()
        .map(|p| format!("if [ -d {} ]; then echo 1; else echo 0; fi", sh_quote(p)))
        .collect();
    let output = Ssh::new(profile)?.run(&checks.join("; "))?;
    let flags: Vec<bool> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim() == "1")
        .collect();
    if flags.len() != paths.len() {
        return Err("检查远端目录失败".to_string());
    }
    Ok(flags)
}

/// 删除远端路径（跨设备“剪切”后的源清理）。
#[tauri::command(async)]
pub fn remote_delete(profile: RemoteProfile, paths: Vec<String>) -> Result<(), String> {
    Ssh::new(profile)?.delete_paths(&paths)
}

/// 远程 A → 远程 B：经本机临时目录中转（tar 流），支持剪切。
#[tauri::command(async)]
pub fn remote_transfer(
    from: RemoteProfile,
    to: RemoteProfile,
    sources: Vec<String>,
    dest_dir: String,
    move_mode: bool,
) -> Result<(), String> {
    if sources.is_empty() {
        return Ok(());
    }
    let src = Ssh::new(from)?;
    let dst = Ssh::new(to)?;
    let staging = temp_dir("xfer");
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;

    let result = (|| {
        for (i, source) in sources.iter().enumerate() {
            let dir = staging.join(i.to_string());
            std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
            let dir_str = dir
                .to_str()
                .ok_or_else(|| "临时目录路径包含非法字符".to_string())?;
            src.download_path(source, dir_str)?;
            let local = dir.join(posix_name(source)?);
            dst.upload_path(&local, &dest_dir)?;
        }
        if move_mode {
            src.delete_paths(&sources)?;
        }
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&staging);
    result
}

// ---- Tauri 命令：提交作业 ----

/// 提交作业的结果（命令即使退出码非 0 也返回，由前端展示输出）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitJobResult {
    pub ok: bool,
    pub command: String,
    pub remote_path: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

fn remote_basename(path: &str) -> String {
    path.trim_end_matches(|c| c == '/' || c == '\\')
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or("")
        .to_string()
}

/// 把 gjf 文本里 `%chk=/some/path/name.chk` 的路径去掉，只保留文件名。
fn strip_chk_paths(content: &str) -> (String, bool) {
    let mut out = String::with_capacity(content.len());
    let mut changed = false;
    for line in content.split_inclusive('\n') {
        let (body, ending) = if let Some(body) = line.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = line.strip_suffix('\n') {
            (body, "\n")
        } else {
            (line, "")
        };
        let trimmed = body.trim_start();
        if trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("%chk=") {
            let start = body.len() - trimmed.len();
            let value = &body[start + 5..];
            let name = value
                .rsplit(|c| c == '/' || c == '\\')
                .next()
                .unwrap_or(value);
            if name != value {
                changed = true;
            }
            out.push_str(&body[..start + 5]);
            out.push_str(name);
        } else {
            out.push_str(body);
        }
        out.push_str(ending);
    }
    (out, changed)
}

fn rewrite_gjf_chk(path: &Path) -> Result<(), String> {
    let is_gjf = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gjf"))
        .unwrap_or(false);
    if !is_gjf {
        return Ok(());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("读取 gjf 文件失败（{}）: {e}", path.display()))?;
    let (rewritten, changed) = strip_chk_paths(&content);
    if changed {
        std::fs::write(path, rewritten)
            .map_err(|e| format!("写回 gjf 临时文件失败: {e}"))?;
    }
    Ok(())
}

/// 同一目录下避免覆盖原文件时使用：`a.gjf` → `a.wmfold.gjf`。
fn submit_filename(name: &str) -> String {
    match name.rfind('.') {
        Some(i) => format!("{}.wmfold{}", &name[..i], &name[i..]),
        None => format!("{name}.wmfold"),
    }
}

fn join_remote(dir: &str, name: &str) -> String {
    if dir == "/" {
        format!("/{name}")
    } else {
        format!("{}/{}", dir.trim_end_matches('/'), name)
    }
}

/// 解析默认提交目录；支持 `~` / `~/...`（用远端 `$HOME` 展开）。
fn resolve_remote_dir(ssh: &Ssh, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    let raw = if raw.is_empty() {
        default_submit_dir()
    } else {
        raw.to_string()
    };
    if raw == "~" || raw.starts_with("~/") {
        let home = ssh.run("printf '%s' \"$HOME\"")?;
        let resolved = if raw == "~" {
            home
        } else {
            format!(
                "{}/{}",
                home.trim_end_matches('/'),
                raw.trim_start_matches("~/")
            )
        };
        return Ok(normalize_remote_dir(&resolved));
    }
    Ok(normalize_remote_dir(&raw))
}

fn normalize_remote_dir(dir: &str) -> String {
    let trimmed = dir.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn run_submit_command(
    ssh: &Ssh,
    dest: &str,
    template: &str,
    remote_path: &str,
) -> Result<SubmitJobResult, String> {
    let command = template.replace("$PATH", &sh_quote(remote_path));
    // 登录 shell 才会加载 ~/.profile / ~/.bashrc 里的 PATH（subg16 等命令通常
    // 在 /opt/bin、~/script 等非默认目录）；普通 ssh 非交互 shell 不会加载这些。
    let script = format!(
        "cd {} && bash -lc {}",
        sh_quote(dest),
        sh_quote(&command)
    );
    let output = ssh.run_raw(&script)?;
    Ok(SubmitJobResult {
        ok: output.status.success(),
        command,
        remote_path: remote_path.to_string(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        exit_code: output.status.code(),
    })
}

/// 把源文件提交到目标机器：
/// - 源可以是本机文件，也可以是另一台远程机器上的文件；
/// - 目标目录由前端传入（目标机器当前浏览的文件夹），否则用目标机器的默认提交目录；
/// - 只有目标机器自己的 `clear_chk_path` 打开时才改写 gjf 的 `%chk=`；
/// - 改写只针对上传的临时副本，不修改源文件。
#[tauri::command(async)]
pub fn submit_job(
    source_profile: Option<RemoteProfile>,
    source_path: String,
    target_profile: RemoteProfile,
    target_dir: Option<String>,
) -> Result<SubmitJobResult, String> {
    let source_path = source_path.trim().to_string();
    if source_path.is_empty() {
        return Err("源文件路径为空".to_string());
    }
    let target = target_profile.normalized();
    let template = target.submit_command.trim().to_string();
    if template.is_empty() {
        return Err("目标机器没有配置提交作业命令".to_string());
    }
    if !template.contains("$PATH") {
        return Err("提交作业命令必须包含 $PATH 占位符".to_string());
    }

    let target_ssh = Ssh::new(target.clone())?;
    let raw_dest = target_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| target.submit_dir.clone());
    let dest = resolve_remote_dir(&target_ssh, &raw_dest)?;
    target_ssh.run(&format!("mkdir -p {}", sh_quote(&dest)))?;

    let source_name = remote_basename(&source_path);
    if source_name.is_empty() {
        return Err("无法识别源文件名".to_string());
    }

    // 同一台机器、同一个目录、且不改 chk：直接在原文件上提交，避免复制覆盖。
    let same_machine_same_dir = source_profile
        .as_ref()
        .map(|p| p.id == target.id && posix_parent(&source_path) == dest)
        .unwrap_or(false);
    if same_machine_same_dir && !target.clear_chk_path {
        return run_submit_command(&target_ssh, &dest, &template, &source_path);
    }

    let staging = temp_dir("submit");
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建提交临时目录失败: {e}"))?;
    let result = (|| {
        let suffix = same_machine_same_dir && target.clear_chk_path;
        let filename = if suffix {
            submit_filename(&source_name)
        } else {
            source_name.clone()
        };
        let staged_file = staging.join(&filename);

        if let Some(profile) = source_profile.as_ref() {
            let source_ssh = Ssh::new(profile.clone())?;
            let staging_str = staging
                .to_str()
                .ok_or_else(|| "临时目录路径包含非法字符".to_string())?;
            source_ssh.download_path(&source_path, staging_str)?;
            let downloaded = staging.join(&source_name);
            if downloaded != staged_file {
                std::fs::rename(&downloaded, &staged_file)
                    .map_err(|e| format!("准备提交临时文件失败: {e}"))?;
            }
        } else {
            let source_local = PathBuf::from(&source_path);
            if !source_local.is_file() {
                return Err(format!("源文件不存在或不是文件：{source_path}"));
            }
            if target.clear_chk_path {
                std::fs::copy(&source_local, &staged_file)
                    .map_err(|e| format!("复制 gjf 到临时目录失败: {e}"))?;
            }
        }

        if target.clear_chk_path {
            rewrite_gjf_chk(&staged_file)?;
        }

        let upload_file = if source_profile.is_some() || target.clear_chk_path {
            &staged_file
        } else {
            Path::new(&source_path)
        };
        target_ssh.upload_path(upload_file, &dest)?;
        let remote_path = join_remote(&dest, &filename);
        run_submit_command(&target_ssh, &dest, &template, &remote_path)
    })();

    let _ = std::fs::remove_dir_all(&staging);
    result
}

// ---- Tauri 命令：配置管理 ----

/// 远程作业查询：在服务器上执行配置的命令（例如 squeue），返回 stdout。
#[tauri::command(async)]
pub fn remote_job_query(profile: RemoteProfile, command: String) -> Result<String, String> {
    let command = command.trim();
    let command = if command.is_empty() {
        "squeue -u $USER"
    } else {
        command
    };
    Ssh::new(profile)?.run(command)
}

#[tauri::command(async)]
pub fn remote_profiles() -> Vec<RemoteProfile> {
    load()
}

#[tauri::command(async)]
pub fn remote_save_profile(profile: RemoteProfile) -> Result<Vec<RemoteProfile>, String> {
    upsert(profile)
}

#[tauri::command(async)]
pub fn remote_delete_profile(id: String) -> Result<Vec<RemoteProfile>, String> {
    // 同时清理这台机器保存的远程快速访问项
    quick_access::remove_remote_machine(&id)?;
    remove(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str) -> RemoteProfile {
        RemoteProfile {
            id: id.to_string(),
            name: "dev".into(),
            host: "example.com".into(),
            port: 22,
            user: "user".into(),
            identity_file: None,
            password: None,
            host_key: None,
            job_query_enabled: false,
            job_query_command: String::new(),
            job_query_interval: 0,
            submit_command: String::new(),
            submit_dir: String::new(),
            clear_chk_path: false,
        }
    }

    #[test]
    fn sanitize_fills_defaults_and_dedupes() {
        let mut a = profile("");
        a.port = 0;
        let mut b = profile("");
        b.host = "other".into();
        b.user = "".into(); // 非法，应被丢弃
        let list = sanitize(vec![a.clone(), a, b]);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].port, 22);
        assert_eq!(list[0].id, "user@example.com:22");
    }

    #[test]
    fn extracts_host_key_fingerprint() {
        let stderr = "The server's ssh-ed25519 key fingerprint is:\n  \
                      ssh-ed25519 255 SHA256:WS1WprD0qnW+0tI1Y19qBizyzjEVWWpNpBPF8W7DekU\n\
                      Connection abandoned.";
        assert_eq!(
            extract_host_key(stderr).as_deref(),
            Some("SHA256:WS1WprD0qnW+0tI1Y19qBizyzjEVWWpNpBPF8W7DekU")
        );
    }

    #[cfg(windows)]
    #[test]
    fn password_is_encrypted_on_disk_and_restored() {
        let dir = std::env::temp_dir().join(format!("wmfold_remotes_pw_{}", std::process::id()));
        let path = dir.join("remotes.json");
        let mut item = profile("pw");
        item.password = Some("p@ss word".to_string());
        save_to(&path, &[item]).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("p@ss word"));
        assert!(raw.contains("dpapi:"));
        let loaded = load_from(&path);
        assert_eq!(loaded[0].password.as_deref(), Some("p@ss word"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn job_query_defaults_are_normalized() {
        let mut item = profile("job");
        item.job_query_command = "  ".to_string();
        item.job_query_interval = 0;
        let item = item.normalized();
        assert_eq!(item.job_query_command, "squeue -u $USER");
        assert_eq!(item.job_query_interval, 10);
        assert_eq!(item.submit_dir, "~/wmfold");
        assert_eq!(item.submit_command, "");

        let mut item = profile("job");
        item.job_query_interval = 9999;
        assert_eq!(item.normalized().job_query_interval, 600);
    }

    #[test]
    fn strips_chk_paths_and_keeps_filename() {
        let input = "%chk=/home/u/calc/a.chk\n%mem=4GB\n%chk=C:\\calc\\b.chk\r\n#p b3lyp\n";
        let (out, changed) = strip_chk_paths(input);
        assert!(changed);
        assert_eq!(
            out,
            "%chk=a.chk\n%mem=4GB\n%chk=b.chk\r\n#p b3lyp\n"
        );

        let unchanged = "%mem=4GB\n%chk=local.chk\n";
        let (out, changed) = strip_chk_paths(unchanged);
        assert!(!changed);
        assert_eq!(out, unchanged);
    }

    #[test]
    fn submit_filename_avoids_overwrite() {
        assert_eq!(submit_filename("a.gjf"), "a.wmfold.gjf");
        assert_eq!(submit_filename("a"), "a.wmfold");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("wmfold_remotes_{}", std::process::id()));
        let path = dir.join("remotes.json");
        let list = vec![profile("a")];
        save_to(&path, &list).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].host, "example.com");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
        assert_eq!(sh_quote(".local/bin/wmfold"), "'.local/bin/wmfold'");
    }

    #[test]
    fn posix_path_helpers() {
        assert_eq!(posix_parent("/home/u/a"), "/home/u");
        assert_eq!(posix_parent("/a"), "/");
        assert_eq!(posix_parent("/home/u/"), "/home");
        assert_eq!(posix_parent("a"), ".");
        assert_eq!(posix_name("/home/u/a/").unwrap(), "a");
        assert!(posix_name("/").is_err());
        assert!(posix_name("").is_err());
    }

    #[test]
    fn tar_roundtrip_copies_directory() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir().join(format!(
            "wmfold-tar-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let src = base.join("src");
        let sub = src.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.txt"), "hello").unwrap();

        let archive = base.join("a.tar");
        tar_create(&archive, &src, "sub").unwrap();
        let out = base.join("out");
        tar_extract(&archive, out.to_str().unwrap()).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("sub").join("a.txt")).unwrap(),
            "hello"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    }
