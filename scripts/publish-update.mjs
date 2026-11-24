// 发布 Tauri 2 自动更新：
//   1. 把当前版本的 NSIS 安装包和 .sig 签名放到 windows/<version>/
//   2. 生成 windows/latest-version-info.json
//   3. 上传到 /home/fly/share/programs/wmfold/（通过 statics 符号链接公开）
//
// 用法：npm run publish:update -- "本次更新说明"
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
const version = pkg.version;
const notes = process.argv.slice(2).join(' ').trim() || `wmfold ${version}`;

const remoteHost = 'fly@47.115.214.219';
const remoteBase = '/home/fly/share/programs/wmfold';
const publicBase = 'https://www.xiaofei911.top/statics/wmfold-updates';

const releaseDir = path.join(root, 'src-tauri', 'target', 'release');
const nsisDir = path.join(releaseDir, 'bundle', 'nsis');
const setupName = `wmfold_${version}_x64-setup.exe`;
const setupPath = path.join(nsisDir, setupName);
const sigPath = `${setupPath}.sig`;
// 官网下载区还提供便携版与 MSI，一起发布到同一个版本目录
const portableName = `wmfold_${version}.exe`;
const portablePath = path.join(releaseDir, portableName);
const msiName = `wmfold_${version}_x64_en-US.msi`;
const msiPath = path.join(releaseDir, 'bundle', 'msi', msiName);

if (!fs.existsSync(setupPath)) {
  console.error(`[publish] 找不到安装包：${setupPath}\n请先运行 npm run build:app`);
  process.exit(1);
}
if (!fs.existsSync(sigPath)) {
  console.error(
    `[publish] 找不到签名文件：${sigPath}\n` +
      '请确认 updater 私钥存在，并通过 npm run build:app 重新构建。',
  );
  process.exit(1);
}
for (const [label, file, hint] of [
  ['便携版', portablePath, 'npm run build:app 会通过 rename:exe 生成'],
  ['MSI', msiPath, 'npm run build:app 会同时产出 MSI'],
]) {
  if (!fs.existsSync(file)) {
    console.error(`[publish] 找不到${label}：${file}\n（${hint}）`);
    process.exit(1);
  }
}

const staging = path.join(root, 'update-staging');
const windowsDir = path.join(staging, 'windows');
const versionDir = path.join(windowsDir, version);
fs.rmSync(staging, { recursive: true, force: true });
fs.mkdirSync(versionDir, { recursive: true });

fs.copyFileSync(setupPath, path.join(versionDir, setupName));
fs.copyFileSync(sigPath, path.join(versionDir, `${setupName}.sig`));
fs.copyFileSync(portablePath, path.join(versionDir, portableName));
fs.copyFileSync(msiPath, path.join(versionDir, msiName));

const signature = fs.readFileSync(sigPath, 'utf8').trim();
const installerUrl = `${publicBase}/windows/${version}/${setupName}`;
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: installerUrl,
    },
  },
};
const manifestPath = path.join(windowsDir, 'latest-version-info.json');
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
console.log(`[publish] 已生成 ${manifestPath}`);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: 'inherit',
    ...options,
  });
  if (result.error || result.status !== 0) {
    console.error(
      `[publish] 命令失败：${command} ${args.join(' ')} ${result.error?.message ?? ''}`,
    );
    process.exit(result.status ?? 1);
  }
}

// 打包后上传；tar 解压是增量覆盖，旧版本文件夹会保留
const archive = path.join(staging, `wmfold-update-${version}.tar.gz`);
run('tar', ['-czf', archive, '-C', staging, 'windows']);
const remoteArchive = `/tmp/wmfold-update-${version}.tar.gz`;
run('scp', [archive, `${remoteHost}:${remoteArchive}`]);
run('ssh', [
  remoteHost,
  `mkdir -p ${remoteBase} && tar -xzf ${remoteArchive} -C ${remoteBase} && ` +
    `ln -sfn ${remoteBase} /home/fly/statics/wmfold-updates && ` +
    `chmod -R a+rX ${remoteBase} && rm -f ${remoteArchive}`,
]);

// 校验公网可访问
const manifestUrl = `${publicBase}/windows/latest-version-info.json`;
const check = await fetch(manifestUrl, { cache: 'no-store' });
if (!check.ok) {
  console.error(`[publish] 清单不可访问：${manifestUrl} (${check.status})`);
  process.exit(1);
}
const remoteManifest = await check.json();
if (remoteManifest.version !== version) {
  console.error(
    `[publish] 清单版本不匹配：期望 ${version}，实际 ${remoteManifest.version}`,
  );
  process.exit(1);
}
for (const [label, url] of [
  ['安装包', installerUrl],
  ['便携版', `${publicBase}/windows/${version}/${portableName}`],
  ['MSI', `${publicBase}/windows/${version}/${msiName}`],
]) {
  const checkFile = await fetch(url, { method: 'HEAD' });
  if (!checkFile.ok) {
    console.error(`[publish] ${label}不可访问：${url} (${checkFile.status})`);
    process.exit(1);
  }
}

console.log(
  `[publish] 已发布 wmfold ${version}\n` +
    `  清单：${manifestUrl}\n` +
    `  安装包：${installerUrl}\n` +
    `  便携版：${publicBase}/windows/${version}/${portableName}\n` +
    `  MSI：${publicBase}/windows/${version}/${msiName}`,
);
