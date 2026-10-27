<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue';
import { useExplorer } from './stores/explorer';
import { useFileActions } from './composables/useFileActions';
import Toolbar from './components/Toolbar.vue';
import CommandRibbon from './components/CommandRibbon.vue';
import FilterBar from './components/FilterBar.vue';
import Sidebar from './components/Sidebar.vue';
import FileList from './components/FileList.vue';
import StatusBar from './components/StatusBar.vue';
import ChemPanel from './components/ChemPanel.vue';
import ContextMenu from './components/ContextMenu.vue';

const store = useExplorer();
const { newFolder, newFile, deleteSelected, rename } = useFileActions();

function onKeyDown(e: KeyboardEvent) {
  // 输入框 / 文本域 / 下拉框内不拦截快捷键
  const tag = (e.target as HTMLElement).tagName;
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

onMounted(async () => {
  await store.init();
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
});
</script>

<template>
  <div class="wmfold">
    <Toolbar />
    <CommandRibbon />
    <FilterBar />
    <div v-if="store.error" class="wm-errorbar" title="点击关闭" @click="store.error = ''">
      {{ store.error }}
    </div>
    <div class="wmfold-body">
      <Sidebar />
      <div class="wmfold-content">
        <div class="wmfold-main">
          <FileList />
        </div>
        <ChemPanel v-if="store.showChemPanel" />
      </div>
    </div>
    <StatusBar />
    <ContextMenu />
  </div>
</template>
