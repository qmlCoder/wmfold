<script setup lang="ts">
import { ElMessage, ElMessageBox } from 'element-plus';
import { useExplorer } from '../stores/explorer';
import SettingsDialog from './SettingsDialog.vue';

const store = useExplorer();

async function newFolder() {
  try {
    const { value } = await ElMessageBox.prompt('请输入文件夹名称', '新建文件夹', {
      confirmButtonText: '创建',
      cancelButtonText: '取消',
      inputValue: '新建文件夹',
      inputPlaceholder: '文件夹名称',
    });
    if (value) {
      await store.createFolder(value);
      ElMessage.success('文件夹已创建');
    }
  } catch {
    /* 取消 */
  }
}

async function newFile() {
  try {
    const { value } = await ElMessageBox.prompt('请输入文件名（含扩展名）', '新建文件', {
      confirmButtonText: '创建',
      cancelButtonText: '取消',
      inputValue: '新建化学文件.xyz',
      inputPlaceholder: '文件名',
    });
    if (value) {
      await store.createFile(value);
      ElMessage.success('文件已创建');
    }
  } catch {
    /* 取消 */
  }
}

async function deleteSelected() {
  if (!store.hasSelection) {
    ElMessage.warning('请先选择要删除的项目');
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定要删除选中的 ${store.selectedEntries.length} 个项目吗？`,
      '删除',
      { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' },
    );
    await store.deleteSelected(false);
    ElMessage.success('已删除');
  } catch {
    /* 取消 */
  }
}

async function rename() {
  const sel = store.selectedEntries[0];
  if (!sel) {
    ElMessage.warning('请先选择一个项目');
    return;
  }
  try {
    const { value } = await ElMessageBox.prompt('请输入新名称', '重命名', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      inputValue: sel.name,
    });
    if (value) {
      await store.renameEntry(sel, value);
      ElMessage.success('已重命名');
    }
  } catch {
    /* 取消 */
  }
}
</script>

<template>
  <div class="wm-cmd" style="border-bottom: 1px solid var(--border)">
    <button class="cmd-btn" @click="newFolder">
      <el-icon><FolderAdd /></el-icon>
      <span>新建文件夹</span>
    </button>
    <button class="cmd-btn" @click="newFile">
      <el-icon><DocumentAdd /></el-icon>
      <span>新建文件</span>
    </button>

    <div class="cmd-divider" />

    <button class="cmd-btn" :class="{ 'is-disabled': !store.hasSelection }" @click="deleteSelected">
      <el-icon><Delete /></el-icon>
      <span>删除</span>
    </button>
    <button class="cmd-btn" :class="{ 'is-disabled': store.selectedEntries.length !== 1 }" @click="rename">
      <el-icon><EditPen /></el-icon>
      <span>重命名</span>
    </button>

    <div class="cmd-divider" />

    <button class="cmd-btn" :class="{ 'is-disabled': !store.hasSelection }" @click="store.copySelected('copy')">
      <el-icon><CopyDocument /></el-icon>
      <span>复制</span>
    </button>
    <button class="cmd-btn" :class="{ 'is-disabled': !store.hasSelection }" @click="store.copySelected('cut')">
      <el-icon><Scissor /></el-icon>
      <span>剪切</span>
    </button>
    <button class="cmd-btn" :class="{ 'is-disabled': !store.clipboard }" @click="store.paste()">
      <el-icon><DocumentCopy /></el-icon>
      <span>粘贴</span>
    </button>

    <div class="cmd-divider" />

    <div style="flex: 1" />

    <SettingsDialog />

    <button class="cmd-btn" :class="{ 'is-active': store.showChemPanel }" @click="store.toggleChemPanel()">
      <el-icon><View /></el-icon>
      <span>预览</span>
    </button>
  </div>
</template>
