// 交叉编译服务器（Linux x86_64-musl）用的 wmfold-cli 二进制，并放入 Tauri 资源目录。
// 用法：npm run build:server
//
// 前置：rustup target add x86_64-unknown-linux-musl
// 说明：用 rust-lld 做静态链接；rswfn 及其依赖均为纯 Rust，因此不需要 Linux 工具链。
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const cliDir = path.join(root, 'wmfold-cli');
const target = 'x86_64-unknown-linux-musl';

const build = spawnSync(
  'cargo',
  ['build', '--release', '--target', target],
  {
    cwd: cliDir,
    stdio: 'inherit',
    shell: true,
    env: {
      ...process.env,
      RUSTFLAGS: '-C linker=rust-lld -C link-self-contained=yes',
    },
  },
);
if (build.status !== 0) {
  console.error('[server] 交叉编译失败（先执行 rustup target add ' + target + '）');
  process.exit(build.status ?? 1);
}

const src = path.join(cliDir, 'target', target, 'release', 'wmfold-cli');
const destDir = path.join(root, 'src-tauri', 'resources');
const dest = path.join(destDir, 'wmfold-server');
fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(src, dest);
console.log(`[server] 已更新 ${dest}`);
