<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue';
import { ElMessageBox } from 'element-plus';
import { useExplorer } from './stores/explorer';
import Toolbar from './components/Toolbar.vue';
import CommandRibbon from './components/CommandRibbon.vue';
import FilterBar from './components/FilterBar.vue';
import Sidebar from './components/Sidebar.vue';
import FileList from './components/FileList.vue';
import StatusBar from './components/StatusBar.vue';
import ChemPanel from './components/ChemPanel.vue';
import ContextMenu from './components/ContextMenu.vue';

const store = useExplorer();

async function deleteSelected() {
  if (!store.hasSelection) return;
  try {
    await ElMessageBox.confirm(`确定删除选中的 ${store.selectedEntries.length} 项吗？`, '删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
    await store.deleteSelected(false);
  } catch {
    /* 取消 */
  }
}

async function rename() {
  const t = store.selectedEntries[0];
  if (!t) return;
  try {
    const { value } = await ElMessageBox.prompt('请输入新名称', '重命名', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      inputValue: t.name,
    });
    if (value) await store.renameEntry(t, value);
  } catch {
    /* 取消 */
  }
}

function onKeyDown(e: KeyboardEvent) {
  const tag = (e.target as HTMLElement).tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

  if (e.altKey && e.key === 'ArrowLeft') {
    e.preventDefault();
    store.goBack();
  } else if (e.altKey && e.key === 'ArrowRight') {
    e.preventDefault();
    store.goForward();
  } else if (e.key === 'Backspace') {
    e.preventDefault();
    store.goUp();
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
    store.copySelected('copy');
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'x') {
    store.copySelected('cut');
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v') {
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
