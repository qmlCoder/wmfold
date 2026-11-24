// 构建发布版：递增版本号 → 带 updater 签名构建 → 生成带版本号的 exe。
//
// 私钥默认在 .tauri/wmfold.key（已被 .gitignore 忽略），可通过
// TAURI_SIGNING_PRIVATE_KEY_PATH 覆盖；密码默认空。
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';

function run(command, args, env = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, ...env },
    shell: process.platform === 'win32',
  });
  if (result.error) {
    console.error(`[build] 启动 ${command} 失败：${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`[build] ${command} ${args.join(' ')} 退出码 ${result.status}`);
    process.exit(result.status ?? 1);
  }
}

const keyPath =
  process.env.TAURI_SIGNING_PRIVATE_KEY_PATH || path.join(root, '.tauri', 'wmfold.key');
if (!fs.existsSync(keyPath)) {
  console.error(
    `[build] 找不到 updater 私钥：${keyPath}\n` +
      '首次构建请先生成密钥：npm run tauri signer generate -w .tauri/wmfold.key',
  );
  process.exit(1);
}

// 1) 版本号递增（package.json / Cargo.toml / tauri.conf.json 三处同步）
run(process.execPath, ['scripts/bump-version.mjs']);

// 2) tauri build（createUpdaterArtifacts + 签名）
run(npm, ['run', 'tauri', 'build'], {
  TAURI_SIGNING_PRIVATE_KEY: keyPath,
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD:
    process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD || '',
});

// 3) 生成 wmfold_<version>.exe
run(process.execPath, ['scripts/rename-exe.mjs']);

const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
console.log(`\n[build] 完成：wmfold ${pkg.version}`);
console.log(
  `[build] 发布更新：npm run publish:update -- "更新说明"`,
);
