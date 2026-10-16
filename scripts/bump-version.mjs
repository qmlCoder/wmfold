// 每次编译时：补丁号（最后一个数）+1，并同步写回版本声明文件。
// 用法：node scripts/bump-version.mjs
//
// 版本以根目录 Cargo.toml 为准（package.json 保持同步，便于 npm 脚本读取）。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const read = (p) => fs.readFileSync(p, 'utf8');
const write = (p, s) => fs.writeFileSync(p, s, 'utf8');

const cargoPath = path.join(root, 'Cargo.toml');
const cargo = read(cargoPath);
const m = cargo.match(/^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"/m);
if (!m) throw new Error('Cargo.toml 中未找到 version');

const nextVersion = `${m[1]}.${m[2]}.${Number(m[3]) + 1}`;
write(cargoPath, cargo.replace(/^version\s*=\s*"[^"]+"/m, `version = "${nextVersion}"`));

const pkgPath = path.join(root, 'package.json');
if (fs.existsSync(pkgPath)) {
  const pkg = JSON.parse(read(pkgPath));
  pkg.version = nextVersion;
  write(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
}

console.log(`[version] 版本号已更新为 ${nextVersion}`);
