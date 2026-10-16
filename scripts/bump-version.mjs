// 每次编译时：小版本号（次版本号，中间位）+1，并同步写回三个版本声明文件。
// 用法：node scripts/bump-version.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const read = (p) => fs.readFileSync(p, 'utf8');
const write = (p, s) => fs.writeFileSync(p, s, 'utf8');

// 以 package.json 为版本源
const pkgPath = path.join(root, 'package.json');
const pkg = JSON.parse(read(pkgPath));
const [major, minor, patch] = pkg.version.split('.').map((n) => Number(n) || 0);

const nextVersion = `${major}.${minor}.${patch + 1}`; // 每次编译：最后一个数（补丁号）+1
pkg.version = nextVersion;
write(pkgPath, JSON.stringify(pkg, null, 2) + '\n');

// Cargo.toml
const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml');
let cargo = read(cargoPath);
if (!/^version\s*=\s*"[^"]+"/m.test(cargo)) throw new Error('Cargo.toml 中未找到 version');
cargo = cargo.replace(/^version\s*=\s*"[^"]+"/m, `version = "${nextVersion}"`);
write(cargoPath, cargo);

// tauri.conf.json
const confPath = path.join(root, 'src-tauri', 'tauri.conf.json');
const conf = JSON.parse(read(confPath));
conf.version = nextVersion;
write(confPath, JSON.stringify(conf, null, 2) + '\n');

console.log(`[version] 版本号已更新为 ${nextVersion}`);
