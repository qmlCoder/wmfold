<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { api } from './api/tauri';
import type { AiToolEvent, ScanProgressEvent } from './types';
import { useExplorer } from './stores/explorer';
import { useFileActions } from './composables/useFileActions';
import Toolbar from './components/Toolbar.vue';
import CommandRibbon from './components/CommandRibbon.vue';
import FilterBar from './components/FilterBar.vue';
import FileList from './components/FileList.vue';
import ScanBar from './components/ScanBar.vue';
import StatusBar from './components/StatusBar.vue';
import RightPanel from './components/RightPanel.vue';
import ContextMenu from './components/ContextMenu.vue';
import SubmitJobDialog from './components/SubmitJobDialog.vue';
import TitleBar from './components/TitleBar.vue';
import QuickAccessBar from './components/QuickAccessBar.vue';

const store = useExplorer();
const { newFolder, newFile, deleteSelected, rename } = useFileActions();

/** 拖动预览窗格左侧分隔条，调整文件列表 / 预览宽度。 */
function startPreviewResize(e: MouseEvent) {
  e.preventDefault();
  const onMove = (ev: MouseEvent) => {
    const max = Math.max(260, Math.round(window.innerWidth * 0.7));
    const width = window.innerWidth - ev.clientX;
    store.setPreviewWidth(Math.min(Math.max(Math.round(width), 220), max));
  };
  const onUp = () => {
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
  };
  window.addEventListener('mousemove', onMove);
  window.addEventListener('mouseup', onUp);
}

function onKeyDown(e: KeyboardEvent) {
  // Ctrl+` 用系统终端打开当前目录（远程机器则新开 ssh 窗口）
  if ((e.ctrlKey || e.metaKey) && e.key === '`') {
    e.preventDefault();
    void store.openTerminal();
    return;
  }

  // 输入框 / 文本域 / 下拉框内不拦截快捷键
  const target = e.target as HTMLElement;
  const tag = target.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

  const mod = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();

  if (e.altKey && e.key === 'ArrowLeft') {
    e.preventDefault();
    store.goBack();
  } else if (e.altKey && e.key === 'ArrowRight') {
    e.preventDefault();
    store.goForward();
  } else if (e.key === 'Backspace') {
    e.preventDefault();
    store.goUp();
  } else if (e.key === 'F5') {
    e.preventDefault();
    store.refresh();
  } else if (mod && e.shiftKey && key === 'n') {
    // Ctrl+Shift+N：新建文件夹
    e.preventDefault();
    newFolder();
  } else if (mod && key === 'n') {
    // Ctrl+N：新建文件
    e.preventDefault();
    newFile();
  } else if (mod && key === 'c') {
    store.copySelected('copy');
  } else if (mod && key === 'x') {
    store.copySelected('cut');
  } else if (mod && key === 'v') {
    store.paste();
  } else if (e.key === 'Delete') {
    e.preventDefault();
    deleteSelected();
  } else if (e.key === 'F2') {
    e.preventDefault();
    rename();
  }
}

let unlistenAttach: (() => void) | undefined;
let unlistenOpenPath: (() => void) | undefined;
let unlistenScan: (() => void) | undefined;
let unlistenAiTool: (() => void) | undefined;

onMounted(async () => {
  unlistenAttach = await listen<{ machine: string; path: string }>('machine-attach', (event) => {
    store.attachMachine(event.payload.machine, event.payload.path);
    if (store.windowLabel !== 'main') {
      void api.registerMachineWindow(
        store.windowLabel,
        event.payload.machine,
        event.payload.path,
      );
    }
  });
  unlistenOpenPath = await listen<string>('open-path', (event) => {
    // 只让主窗口响应资源管理器右键菜单，避免独立机器窗口被带到本机目录。
    if (store.windowLabel === 'main' && event.payload) {
      void store.goLocal(event.payload);
    }
  });
  // 递归读取进度 / AI 工具调用进度
  unlistenScan = await listen<ScanProgressEvent>('scan-progress', (event) => {
    store.onScanProgress(event.payload);
  });
  unlistenAiTool = await listen<AiToolEvent>('ai-tool', (event) => {
    store.onAiToolEvent(event.payload);
  });
  await store.init();
  // 首次由资源管理器右键菜单启动时，直接打开对应目录。
  if (store.windowLabel === 'main') {
    const launchPath = await api.takeLaunchPath().catch(() => null);
    if (launchPath) await store.goLocal(launchPath);
  }
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  unlistenAttach?.();
  unlistenOpenPath?.();
  unlistenScan?.();
  unlistenAiTool?.();
  window.removeEventListener('keydown', onKeyDown);
});
</script>

<template>
  <div class="wmfold">
    <TitleBar />
    <QuickAccessBar />
    <Toolbar />
    <CommandRibbon />
    <FilterBar />
    <ScanBar />
    <div v-if="store.error" class="wm-errorbar" title="点击关闭" @click="store.error = ''">
      {{ store.error }}
    </div>
    <div class="wmfold-body">
      <div class="wmfold-content">
        <div class="wmfold-main">
          <div class="wmfold-filearea">
            <FileList />
          </div>
        </div>
        <div
          v-if="store.showChemPanel"
          class="wm-splitter"
          title="拖动调整预览宽度"
          @mousedown="startPreviewResize"
        />
        <RightPanel
          v-if="store.showChemPanel"
          :style="{ width: `${store.previewWidth}px` }"
        />
      </div>
    </div>
    <StatusBar />
    <ContextMenu />
    <SubmitJobDialog />
  </div>
</template>
