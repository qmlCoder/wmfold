<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ElMessageBox } from 'element-plus';
import { useExplorer } from '../stores/explorer';
import { useUpdater } from '../composables/useUpdater';
import MachineTabs from './MachineTabs.vue';
import SettingsDialog from './SettingsDialog.vue';

const store = useExplorer();
const appWindow = getCurrentWindow();

const {
  appVersion,
  availableUpdate,
  installing: updateInstalling,
  progress: updateProgress,
  message: updateMessage,
  checkForUpdate,
  installUpdate,
} = useUpdater();

async function upgrade() {
  const update = availableUpdate.value;
  if (!update || updateInstalling.value) return;
  try {
    await ElMessageBox.confirm(
      `发现新版本 ${update.version}（当前 ${update.currentVersion}），是否立即下载并安装？`,
      '软件更新',
      {
        confirmButtonText: '立即升级',
        cancelButtonText: '稍后',
        type: 'info',
      },
    );
  } catch {
    return;
  }
  const ok = await installUpdate();
  if (!ok && updateMessage.value) {
    store.error = updateMessage.value;
  }
}

const maximized = ref(false);
let unlisten: (() => void) | undefined;
let lastTitlebarDown = 0;

async function syncMaximized() {
  try {
    maximized.value = await appWindow.isMaximized();
  } catch {
    /* 非 Tauri 环境（纯浏览器预览）忽略 */
  }
}

async function minimize() {
  await appWindow.minimize();
}

async function toggleMaximize() {
  await appWindow.toggleMaximize();
  await syncMaximized();
}

async function close() {
  await appWindow.close();
}

/**
 * 自定义标题栏的拖动。
 *
 * 之前只有 tab 右侧一条 24px 的 `data-tauri-drag-region` 能拖，标题栏其余
 * 区域都拖不动。这里改为在标题栏空白区域按下鼠标时直接调用 startDragging，
 * 并排除所有交互元素（窗口按钮、机器 tab、加号菜单）。
 */
async function onTitlebarMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement | null;
  if (!target) return;
  if (
    target.closest(
      'button, input, textarea, .wm-machine-tab, .wm-titlebar-control, .wm-tab-menu, .wm-machine-pop',
    )
  ) {
    return;
  }

  // 双击标题栏空白处：最大化 / 还原
  const now = Date.now();
  if (now - lastTitlebarDown < 350) {
    lastTitlebarDown = 0;
    await toggleMaximize();
    return;
  }
  lastTitlebarDown = now;

  try {
    await appWindow.startDragging();
  } catch {
    /* 非 Tauri 环境忽略 */
  }
}

onMounted(async () => {
  await syncMaximized();
  unlisten = await appWindow.onResized(() => void syncMaximized());
  // 启动后静默检查一次更新；有新版本时在设置按钮左侧显示升级按钮。
  void checkForUpdate({ silent: true });
});

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <header class="wm-titlebar" @mousedown="onTitlebarMouseDown">
    <div class="wm-titlebar-left">
      <img class="wm-titlebar-icon" src="/app-icon.png" alt="" />
      <span class="wm-titlebar-brand">wmfold</span>
      <span v-if="appVersion" class="wm-titlebar-version">v{{ appVersion }}</span>
    </div>

    <div class="wm-titlebar-tabs">
      <MachineTabs />
      <div class="wm-titlebar-drag" />
    </div>

    <div class="wm-titlebar-controls">
      <button
        v-if="availableUpdate"
        class="wm-titlebar-control wm-upgrade-button"
        :title="
          updateInstalling
            ? updateProgress || '正在升级…'
            : `发现新版本 ${availableUpdate.version}，点击升级`
        "
        :disabled="updateInstalling"
        @click="upgrade"
      >
        <el-icon><Download /></el-icon>
        <span>{{ updateInstalling ? '升级中' : '升级' }}</span>
      </button>
      <button class="wm-titlebar-control" title="设置" @click="store.openSettings('display')">
        <el-icon><Setting /></el-icon>
      </button>
      <button class="wm-titlebar-control" title="最小化" @click="minimize">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 5h10" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button
        class="wm-titlebar-control"
        :title="maximized ? '还原' : '最大化'"
        @click="toggleMaximize"
      >
        <svg v-if="!maximized" width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
        <svg v-else width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <rect x="0.5" y="2.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
          <path d="M2.5 2.5v-2h7v7h-2" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button class="wm-titlebar-control is-close" title="关闭" @click="close">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
    </div>
  </header>

  <!-- 设置对话框必须放在 header 外面：否则它的 tab 点击会被标题栏拖动逻辑吞掉 -->
  <SettingsDialog :show-button="false" />
</template>

<style scoped>
.wm-titlebar {
  display: flex;
  align-items: stretch;
  height: var(--wm-titlebar-height, 32px);
  flex-shrink: 0;
  background: #ffffff;
  border-bottom: 1px solid var(--border, #e0e0e0);
  user-select: none;
  position: relative;
  z-index: 1000;
}

.wm-titlebar-left {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-left: 10px;
  padding-right: 6px;
  min-width: 0;
  flex-shrink: 0;
}

.wm-titlebar-icon {
  width: 16px;
  height: 16px;
  object-fit: contain;
  -webkit-user-drag: none;
  pointer-events: none;
}

.wm-titlebar-brand {
  font-size: 12px;
  font-weight: 600;
  color: #303133;
  white-space: nowrap;
  pointer-events: none;
}

.wm-titlebar-version {
  font-size: 11px;
  color: #8a8f98;
  white-space: nowrap;
  pointer-events: none;
  font-variant-numeric: tabular-nums;
}

.wm-titlebar-tabs {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: stretch;
}

.wm-titlebar-drag {
  flex: 1;
  min-width: 12px;
}

.wm-titlebar-controls {
  display: flex;
  align-items: stretch;
}

.wm-titlebar-control {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  border: none;
  background: transparent;
  color: #303133;
  cursor: pointer;
}

.wm-titlebar-control:hover {
  background: #f0f0f0;
}

.wm-titlebar-control.is-close:hover {
  background: #e81123;
  color: #ffffff;
}

.wm-upgrade-button {
  width: auto;
  gap: 4px;
  padding: 0 12px;
  font-size: 12px;
  color: var(--accent, #0067c0);
}

.wm-upgrade-button:hover {
  background: #eef5ff;
}

.wm-upgrade-button:disabled {
  opacity: 0.65;
  cursor: default;
}
</style>
