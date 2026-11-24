//! `wmfold-cli` 独立可执行入口：在服务器 / Linux 上提供 `-cli` 的 JSON 接口。
//!
//! 桌面端合并启动器 `wmfold` 复用同一个库（`wmfold -cli ...`），两者的输出完全一致。

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::ExitCode::from(wmfold_cli::cli_main(args) as u8)
}
