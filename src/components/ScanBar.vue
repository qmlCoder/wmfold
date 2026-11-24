<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();

const current = computed(() => {
  const path = store.scanProgress.current;
  if (!path) return '';
  return store.recursiveRoot ? store.relativePath(path) : path;
});

const elapsed = computed(() => {
  const ms = store.scanning ? store.scanProgress.elapsedMs : (store.scanStats?.elapsedMs ?? 0);
  const seconds = ms / 1000;
  return seconds >= 10 ? `${seconds.toFixed(0)} 秒` : `${seconds.toFixed(1)} 秒`;
});

function action() {
  if (store.scanning) void store.cancelRecursiveScan();
  else void store.stopRecursive();
}
</script>

<template>
  <div v-if="store.scanning || store.recursiveMode" class="wm-scanbar">
    <el-icon class="wm-scanbar-icon" :class="{ 'is-spinning': store.scanning }">
      <Loading v-if="store.scanning" />
      <Files v-else />
    </el-icon>

    <span v-if="store.scanning" class="wm-scanbar-text">
      正在扫描：已扫描
      <strong>{{ store.scanProgress.dirs }}</strong> 个文件夹 /
      <strong>{{ store.scanProgress.files }}</strong> 个文件
    </span>
    <span v-else class="wm-scanbar-text">
      扫描完成：<strong>{{ store.scanStats?.files ?? 0 }}</strong> 个文件 ·
      <strong>{{ store.scanStats?.dirs ?? 0 }}</strong> 个文件夹
      <span v-if="store.scanStats?.cancelled" class="wm-scanbar-note">（已取消，部分结果）</span>
      <span v-else-if="store.scanStats?.truncated" class="wm-scanbar-note">（已达上限）</span>
      <span class="wm-scanbar-note">· {{ elapsed }}</span>
    </span>

    <span class="wm-scanbar-current" :title="store.scanProgress.current">{{ current }}</span>

    <div class="wm-scanbar-track" :class="{ 'is-active': store.scanning }">
      <div class="wm-scanbar-fill" />
    </div>

    <button class="wm-scanbar-btn" @click="action">
      {{ store.scanning ? '取消' : '退出扫描' }}
    </button>
  </div>
</template>
