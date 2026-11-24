//! WMFold 机器接口（`wmfold -cli`）。
//!
//! 只输出 JSON，不进入任何交互界面；两个入口共用同一套实现：
//! - 桌面端合并启动器 `wmfold`：`wmfold -cli list|info|version ...`；
//! - 服务器 / Linux 上单独编译的静态二进制 `wmfold-cli`
//!   （桌面端远程模式会把它的产物作为 `wmfold-server` 上传到 `~/.local/bin/wmfold`）。
//!
//! JSON 协议见 [`cli::PROTOCOL`]，桌面端会校验它；字段变化时要同步升级版本号。

mod cli;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 随桌面端发布的服务器二进制版本（远程机器据此判断是否需要重新上传）。
pub const SERVER_VERSION: &str = VERSION;

/// 独立的 `wmfold-cli` 二进制入口。
pub fn cli_main(args: Vec<String>) -> i32 {
    cli_main_named("wmfold-cli", args)
}

/// 带程序名的入口：帮助 / 错误信息里的命令名由调用方决定。
///
/// 合并启动器传入 `"wmfold"`，并把参数里的 `-cli` / `--cli` 原样交给这里剥离。
pub fn cli_main_named(program: &str, mut args: Vec<String>) -> i32 {
    let flagged = match args.iter().position(|a| a == "-cli" || a == "--cli") {
        Some(pos) => {
            args.remove(pos);
            true
        }
        None => false,
    };
    let display = if flagged {
        format!("{program} -cli")
    } else {
        program.to_string()
    };

    if args.first().map(|a| a == "-h" || a == "--help").unwrap_or(false) {
        println!("{}", cli::usage(&display));
        return 0;
    }
    if args
        .first()
        .map(|a| a == "-V" || a == "--version")
        .unwrap_or(false)
    {
        println!("{display} {VERSION}");
        return 0;
    }

    // 坏文件的 panic 回溯不能混进 stdout 的 JSON，静默期间改写到临时日志
    install_panic_hook();
    QUIET_PANICS.store(true, Ordering::Relaxed);
    let result = cli::run(&display, args);
    QUIET_PANICS.store(false, Ordering::Relaxed);
    report_panics();

    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{display}: {err}");
            1
        }
    }
}

/// JSON 输出期间是否静默 panic。
///
/// 化学解析遇到坏文件时会 panic（`wmfold-core` 用 `catch_unwind` 兜住），默认 hook
/// 会把回溯写到 stderr；静默期间改为追加写入临时日志，保证 stdout 只有一行 JSON。
static QUIET_PANICS: AtomicBool = AtomicBool::new(false);

/// 本次进程内被静默记录的 panic 次数（用于结束后提示）。
static PANIC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// panic 日志文件（临时目录下）。
fn panic_log_path() -> PathBuf {
    std::env::temp_dir().join("wmfold-cli-panics.log")
}

/// 安装 panic hook：静默期间记录到文件，其余情况保持默认行为。
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if QUIET_PANICS.load(Ordering::Relaxed) {
            PANIC_COUNT.fetch_add(1, Ordering::Relaxed);
            use std::io::Write as _;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(panic_log_path())
            {
                let _ = writeln!(file, "{info}");
            }
        } else {
            default_hook(info);
        }
    }));
}

/// 若有被静默记录的解析失败，退出前打印一条简短提示（stderr）。
fn report_panics() {
    let count = PANIC_COUNT.swap(0, Ordering::Relaxed);
    if count > 0 {
        eprintln!(
            "wmfold-cli: warning: {count} chemistry file(s) could not be parsed (details: {})",
            panic_log_path().display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_flag_prints_without_json() {
        // 只验证常量存在，避免真的退出进程
        assert!(!VERSION.is_empty());
    }
}
