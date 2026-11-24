import { ref, shallowRef } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';

// 全应用共享的更新状态：标题栏升级按钮与 设置 → 程序设置 → 软件更新 使用同一份。
const appVersion = ref('');
const checking = ref(false);
const installing = ref(false);
// Update 带私有字段，必须使用 shallowRef，否则 Vue 代理会破坏 downloadAndInstall。
const availableUpdate = shallowRef<Update | null>(null);
const message = ref('');
const progress = ref('');

let versionPromise: Promise<void> | null = null;

function ensureVersion() {
  if (!versionPromise) {
    versionPromise = getVersion()
      .then((value) => {
        appVersion.value = value;
      })
      .catch(() => {
        /* 浏览器预览 / 非 Tauri 环境 */
      });
  }
  return versionPromise;
}

async function checkForUpdate(options: { silent?: boolean } = {}): Promise<Update | null> {
  if (checking.value || installing.value) return null;
  await ensureVersion();
  checking.value = true;
  if (!options.silent) message.value = '正在检查更新…';
  try {
    const update = await check();
    availableUpdate.value = update;
    if (update) {
      message.value = `发现新版本 ${update.version}（当前 ${update.currentVersion}）`;
    } else {
      message.value = options.silent ? '' : '当前已是最新版本';
    }
    return update;
  } catch (e) {
    availableUpdate.value = null;
    if (!options.silent) message.value = `检查更新失败：${String(e)}`;
    return null;
  } finally {
    checking.value = false;
  }
}

async function installUpdate(): Promise<boolean> {
  const update = availableUpdate.value;
  if (!update || installing.value) return false;
  installing.value = true;
  progress.value = '开始下载…';
  let downloaded = 0;
  try {
    await update.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        progress.value = '正在下载更新…';
      } else if (event.event === 'Progress') {
        downloaded += event.data.chunkLength;
        progress.value = `已下载 ${(downloaded / 1024 / 1024).toFixed(1)} MB`;
      } else if (event.event === 'Finished') {
        progress.value = '下载完成，正在安装…';
      }
    });
    progress.value = '安装完成，正在重启…';
    await relaunch();
    return true;
  } catch (e) {
    message.value = `更新失败：${String(e)}`;
    progress.value = '';
    installing.value = false;
    return false;
  }
}

/** 共享的 Tauri 2 自动更新状态与操作。 */
export function useUpdater() {
  void ensureVersion();
  return {
    appVersion,
    checking,
    installing,
    availableUpdate,
    message,
    progress,
    checkForUpdate,
    installUpdate,
  };
}
