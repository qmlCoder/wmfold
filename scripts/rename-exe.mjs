// 构建后将 wmfold.exe 复制为带版本号的程序名，例如 wmfold_0.3.5.exe。
// 用法：node scripts/rename-exe.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const pkgPath = path.join(root, 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
const version = pkg.version;

const releaseDir = path.join(root, 'src-tauri', 'target', 'release');
const srcExe = path.join(releaseDir, 'wmfold.exe');

if (!fs.existsSync(srcExe)) {
  console.error(`[rename] 未找到 ${srcExe}，请先执行 npm run tauri build`);
  process.exit(1);
}

const destExe = path.join(releaseDir, `wmfold_${version}.exe`);
fs.copyFileSync(srcExe, destExe);
console.log(`[rename] 已生成带版本号的程序：${destExe}`);
