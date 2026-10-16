// 构建后将 wmfold.exe 复制为带版本号的程序名，例如 WMFold_0.3.4.exe。
// 用法：node scripts/rename-exe.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const m = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!m) throw new Error('Cargo.toml 中未找到 version');
const version = m[1];

const releaseDir = path.join(root, 'target', 'release');
const srcExe = path.join(releaseDir, 'wmfold.exe');

if (!fs.existsSync(srcExe)) {
  console.error(`[rename] 未找到 ${srcExe}，请先执行 cargo build --release`);
  process.exit(1);
}

const destExe = path.join(releaseDir, `WMFold_${version}.exe`);
fs.copyFileSync(srcExe, destExe);
console.log(`[rename] 已生成带版本号的程序：${destExe}`);
