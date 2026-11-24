<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { useExplorer } from '../stores/explorer';
import { THIS_PC } from '../utils/path';

const store = useExplorer();

const canBack = computed(() => store.historyIndex > 0);
const canForward = computed(() => store.historyIndex < store.history.length - 1);
const canUp = computed(() =>
  store.isRemote ? store.currentPath.replace(/\/+$/, '') !== '' : store.currentPath !== THIS_PC,
);

const editMode = ref(false);
const editValue = ref('');
const addrInput = ref<InstanceType<typeof HTMLInputElement> | null>(null);

function enterEdit() {
  editValue.value = store.currentPath;
  editMode.value = true;
  nextTick(() => (addrInput.value as any)?.focus?.());
}

function submitEdit() {
  const v = editValue.value.trim();
  editMode.value = false;
  if (v) store.navigate(v);
}

function cancelEdit() {
  editMode.value = false;
}
</script>

<template>
  <div class="wm-cmd" style="padding-bottom: 2px">
    <button class="cmd-btn" :class="{ 'is-disabled': !canBack }" title="后退" @click="store.goBack()">
      <el-icon><DArrowLeft /></el-icon>
      <span>后退</span>
    </button>
    <button class="cmd-btn" :class="{ 'is-disabled': !canForward }" title="前进" @click="store.goForward()">
      <el-icon><DArrowRight /></el-icon>
      <span>前进</span>
    </button>
    <button class="cmd-btn" :class="{ 'is-disabled': !canUp }" title="向上" @click="store.goUp()">
      <el-icon><Top /></el-icon>
      <span>向上</span>
    </button>
    <button class="cmd-btn" title="刷新" @click="store.refresh()">
      <el-icon><Refresh /></el-icon>
      <span>刷新</span>
    </button>

    <div style="flex: 1" />

    <!-- 地址栏 -->
    <div class="wm-addrbar" @dblclick="enterEdit">
      <template v-if="!editMode">
        <div class="wm-crumbs">
          <template v-for="(c, i) in store.crumbPaths" :key="c.path">
            <span class="wm-crumb" @click="store.navigate(c.path)">{{ c.label }}</span>
            <span v-if="i < store.crumbPaths.length - 1" class="wm-crumb-sep"> › </span>
          </template>
        </div>
      </template>
      <template v-else>
        <input
          ref="addrInput"
          v-model="editValue"
          class="wm-addrbar-input"
          style="width: 100%; border: none; outline: none; font-size: 13px"
          @keydown.enter="submitEdit"
          @keydown.esc="cancelEdit"
          @blur="cancelEdit"
        />
      </template>
    </div>

    <el-input
      v-model="store.search"
      class="wm-search"
      placeholder="搜索当前目录"
      clearable
    >
      <template #prefix>
        <el-icon><Search /></el-icon>
      </template>
    </el-input>
  </div>
</template>
