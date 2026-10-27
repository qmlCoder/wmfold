<script setup lang="ts">
import { useExplorer } from '../stores/explorer';
import { useFileActions } from '../composables/useFileActions';
import SettingsDialog from './SettingsDialog.vue';

const store = useExplorer();
// 与全局快捷键共用同一套新建 / 删除 / 重命名逻辑
const { newFolder, newFile, deleteSelected, rename } = useFileActions();
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

    <button class="cmd-btn" :class="{ 'is-disabled': !store.hasSelection }" @click="deleteSelected(true)">
      <el-icon><Delete /></el-icon>
      <span>删除</span>
    </button>
    <button class="cmd-btn" :class="{ 'is-disabled': store.selectedEntries.length !== 1 }" @click="rename(true)">
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
