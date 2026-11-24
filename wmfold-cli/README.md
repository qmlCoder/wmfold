# wmfold-cli · 机器接口（JSON）

`wmfold-cli` 是 wmfold 的**机器接口**：它只输出 JSON，不进入任何交互界面。
桌面端的远程机器模式（通过 SSH）与脚本调用共用这一套协议。

它由两部分组成：

- **库 `wmfold_cli`**：合并启动器 `wmfold` 以 `wmfold -cli <命令>` 的形式调用；
- **独立二进制 `wmfold-cli`**：在没有 Tauri / WebView / Node 的服务器（Linux）上单独编译，
  参数与 `wmfold -cli` 完全一致，也可以改名成 `wmfold` 放到 `~/.local/bin/` 供桌面端识别。

依赖只有 `wmfold-core`（化学解析 / 目录列表 / 数据模型）、`serde` 与 `serde_json`；
不依赖 ratatui / crossterm / chrono，也不进入全屏或 alternate screen。

## 用法

```bash
# 通过合并启动器（桌面端程序）
wmfold -cli list [PATH] [--chem]
wmfold -cli info <PATH> [--chem]
wmfold -cli structure <PATH>
wmfold -cli version

# 独立二进制，参数相同
wmfold-cli list [PATH] [--chem]
wmfold-cli info <PATH> [--chem]
wmfold-cli structure <PATH>
wmfold-cli version

# 帮助 / 版本（纯文本，不是 JSON）
wmfold -cli --help
wmfold -cli -V
```

- `list [PATH]`：列出目录，`PATH` 省略时用当前目录 `.`。返回的 `path` 与每个条目的
  `path` 都是绝对路径。
- `info <PATH>`：返回单个文件 / 目录的条目。
- `structure <PATH>`：读取分子三维结构，返回 `structure.ok`、`structure.atoms`
  （元素符号 + Å 坐标）与 `structure.bonds`（原子索引对）；解析失败时返回 `ok: false`，
  不会让 CLI 报错退出。桌面端远程三维预览用的就是这条命令。
- `version`：返回协议版本、程序名与版本号（JSON）。
- `--chem`（或 `-c`）：为化学文件解析化学元数据（不传时 `chem` 为 `null`，列目录更快）。

## JSON 结构

协议版本 `PROTOCOL = 1`，定义在 `src/cli.rs`。**字段有不兼容变化时必须递增该版本号**，
并同步修改桌面端 `src-tauri/src/remotes.rs` 里的 `PROTOCOL` 与
`RemoteListing` / `RemoteInfoResponse` 结构体（两端不匹配时桌面端会提示重新安装服务器端）。

`list` 返回：

```json
{
  "protocol": 1,
  "path": "/home/user/calc",
  "entries": [
    {
      "name": "water.xyz",
      "path": "/home/user/calc/water.xyz",
      "isDir": false,
      "size": 412,
      "created": 1730000000000,
      "modified": 1730000123000,
      "extension": "xyz",
      "kind": "xyz",
      "isChem": true,
      "isHidden": false,
      "isSymlink": false,
      "chem": {
        "fileType": "xyz",
        "fileKind": "xyz",
        "formula": "H2O",
        "nAtoms": 3,
        "charge": 0,
        "multip": 1,
        "nAlpha": 5,
        "nBeta": 5,
        "nBasis": 0,
        "molW": 18.015,
        "energy": null,
        "job": "p b3lyp/6-31g(d) opt freq",
        "ok": true
      }
    }
  ]
}
```

`structure` 返回：

```json
{
  "protocol": 1,
  "path": "/home/user/calc/water.xyz",
  "structure": {
    "ok": true,
    "atoms": [
      { "element": "O", "x": 0.0, "y": 0.0, "z": 0.1068 },
      { "element": "H", "x": 0.0, "y": 0.7852, "z": -0.4273 }
    ],
    "bonds": [[0, 1], [0, 2]]
  }
}
```

坐标单位为 **Å**；桌面端 `src-tauri/src/remotes.rs` 用 `RemoteStructureResponse` 反序列化。

`info` 返回：

```json
{
  "protocol": 1,
  "entry": { "name": "water.xyz", "path": "...", "isDir": false, "...": "..." }
}
```

`version` 返回：

```json
{ "protocol": 1, "name": "wmfold-cli", "version": "0.1.0" }
```

说明：

- 字段名一律 **camelCase**（`wmfold-core` 的 `#[serde(rename_all = "camelCase")]`）。
- `entries` / `entry` 的形状与桌面端文件表条目一致（`wmfold-core::models::FileEntry`）。
- 目录条目的 `size` 为 0、`chem` 为 `null`；`created` / `modified` 是 Unix 时间戳（毫秒），
  取不到时为 `null`。
- 解析失败时化学字段回退为默认值（`ok: false`），不会中断整个列表。

## 错误处理

- 正常输出时 **stdout 始终只有一行 JSON**，错误信息走 stderr，退出码非 0。
- 化学解析遇到坏文件会 panic（`wmfold-core` 已用 `catch_unwind` 兜住）；JSON 输出期间
  panic 回溯被静默记录到临时目录下的 `wmfold-cli-panics.log`
  （Windows `%TEMP%`，Linux `/tmp`），不会污染 stdout；结束时 stderr 只打印一行提示
  与日志路径。
- 用法错误（未知子命令 / 选项、多个 PATH）会打印用法并返回非 0。

## 服务器端构建

桌面端远程模式首次连接时，会把随包的 Linux x86_64-musl 静态二进制
`src-tauri/resources/wmfold-server` 上传到远端 `~/.local/bin/wmfold`（`chmod 755`），
之后调用 `wmfold -cli list/info`。

```bash
# 1) 在 Linux 服务器上直接编译
cd wmfold-cli
cargo build --release                 # 产物 target/release/wmfold-cli
# 拷贝到 ~/.local/bin/wmfold 即可

# 2) 在 Windows 上交叉编译随包静态版本（刷新 Tauri 资源）
npm run build:server                  # 需先 rustup target add x86_64-unknown-linux-musl
# 流程见 scripts/build-server.mjs：
#   cd wmfold-cli && cargo build --release --target x86_64-unknown-linux-musl
#   target/x86_64-unknown-linux-musl/release/wmfold-cli
#     -> src-tauri/resources/wmfold-server
```

## 测试

```bash
cargo test                            # 在 wmfold-cli/ 内运行
npm run test:cli                      # 或在仓库根目录运行（等价）
```

测试覆盖 `list` / `info` 的 JSON 序列化（协议版本、`entries` / `entry`、camelCase 字段）
以及参数解析（`--chem`、默认 `.`、非法选项）。
