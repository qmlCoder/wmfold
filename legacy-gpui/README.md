# legacy-gpui —— 纯 Rust / GPUI Kit 实现（已归档）

本目录保存 WMFold 在迁回 **Tauri 2 + Vue** 之前的那一版实现：单一 Rust
crate，界面基于 `gpui-kit`（GPUI + GPUI Base + GPUI Component），不依赖
Node.js / WebView2。

- **不参与当前构建**，仅作参考与对照。
- 版本停留在 `0.3.4`（Tauri 版从同一版本号继续）。
- 依赖 `rswfn = { path = "../../rswfn" }`，即 `E:\code\rswfn`。
- 若要单独构建，在本目录执行 `cargo run` / `cargo build --release`；
  产物为 `legacy-gpui/target/release/wmfold.exe`。

当前开发的技术栈是 Tauri 2 + Vue 3（仓库根目录），见根目录
[README.md](../README.md) 与 [AGENTS.md](../AGENTS.md)。
