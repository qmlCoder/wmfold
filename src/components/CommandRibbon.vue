<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';
import { useFileActions } from '../composables/useFileActions';

const store = useExplorer();
// 与全局快捷键共用同一套新建 / 删除 / 重命名逻辑
const { newFolder, newFile, deleteSelected, rename } = useFileActions();

const hasFilter = computed(
  () => store.search.trim().length > 0 || store.filterType !== 'all',
);

const scanTitle = computed(() =>
  store.scanning
    ? '取消扫描'
    : store.recursiveMode
      ? '退出扫描视图，回到普通列表'
      : '扫描当前文件夹及所有子文件夹中的文件',
);

/** 扫描：再次点击取消扫描；已在扫描视图时退出并回到普通列表。 */
function toggleRecursive() {
  if (store.scanning) {
    void store.cancelRecursiveScan();
  } else if (store.recursiveMode) {
    void store.stopRecursive();
  } else {
    void store.startRecursive();
  }
}

/** 预览按钮：先切到预览 tab；已经显示预览时再点则关闭右侧面板。 */
function togglePreview() {
  if (store.showChemPanel && store.rightPanelTab === 'preview') {
    store.toggleChemPanel();
    return;
  }
  store.showChemPanel = true;
  store.rightPanelTab = 'preview';
}
</script>

<template>
  <div class="wm-cmd" style="border-bottom: 1px solid var(--border)">
    <button class="cmd-btn" title="新建文件夹" @click="newFolder">
      <el-icon><FolderAdd /></el-icon>
      <span>新建文件夹</span>
    </button>
    <button class="cmd-btn" title="新建文件" @click="newFile">
      <el-icon><DocumentAdd /></el-icon>
      <span>新建文件</span>
    </button>

    <div class="cmd-divider" />

    <button
      class="cmd-btn"
      title="删除"
      :class="{ 'is-disabled': !store.hasSelection }"
      @click="deleteSelected(true)"
    >
      <el-icon><Delete /></el-icon>
      <span>删除</span>
    </button>
    <button
      class="cmd-btn"
      title="重命名"
      :class="{ 'is-disabled': store.selectedEntries.length !== 1 }"
      @click="rename(true)"
    >
      <el-icon><EditPen /></el-icon>
      <span>重命名</span>
    </button>

    <div class="cmd-divider" />

    <button
      class="cmd-btn"
      title="复制"
      :class="{ 'is-disabled': !store.hasSelection }"
      @click="store.copySelected('copy')"
    >
      <el-icon><CopyDocument /></el-icon>
      <span>复制</span>
    </button>
    <button
      class="cmd-btn"
      title="剪切"
      :class="{ 'is-disabled': !store.hasSelection }"
      @click="store.copySelected('cut')"
    >
      <el-icon><Scissor /></el-icon>
      <span>剪切</span>
    </button>
    <button
      class="cmd-btn"
      title="粘贴"
      :class="{ 'is-disabled': !store.clipboard }"
      @click="store.paste()"
    >
      <el-icon><DocumentCopy /></el-icon>
      <span>粘贴</span>
    </button>

    <div class="cmd-divider" />

    <button
      class="cmd-btn wm-scan-btn"
      :class="{
        'is-active': store.recursiveMode || store.scanning,
        'is-disabled': store.isRemote || store.isThisPc,
      }"
      :title="scanTitle"
      :aria-label="scanTitle"
      @click="toggleRecursive"
    >
      <el-icon><Files /></el-icon>
      <span>扫描</span>
    </button>

    <button
      v-if="hasFilter"
      class="cmd-btn"
      title="取消搜索和类型过滤，恢复完整列表"
      @click="store.clearFilters()"
    >
      <el-icon><Close /></el-icon>
      <span>取消过滤</span>
    </button>

    <div style="flex: 1" />

    <button class="cmd-btn" title="AI 对话（DeepSeek）" @click="store.openAiPanel()">
      <el-icon><ChatDotRound /></el-icon>
      <span>AI 对话</span>
    </button>

    <button
      class="cmd-btn"
      title="在系统终端中打开当前文件夹"
      @click="store.openTerminal()"
    >
      <el-icon><Platform /></el-icon>
    </button>

    <button
      class="cmd-btn"
      title="预览"
      :class="{ 'is-active': store.showChemPanel && store.rightPanelTab === 'preview' }"
      @click="togglePreview"
    >
      <el-icon><View /></el-icon>
      <span>预览</span>
    </button>
  </div>
</template>
