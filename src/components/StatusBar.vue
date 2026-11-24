<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();

const total = computed(() => store.visibleEntries.length);
const selected = computed(() => store.selectedPaths.length);
const machine = computed(() => store.activeRemote?.name || store.activeRemote?.host || '');
const scanInfo = computed(() => {
  if (store.scanning) {
    return `扫描中 ${store.scanProgress.dirs} 个文件夹 / ${store.scanProgress.files} 个文件`;
  }
  if (store.recursiveMode) {
    return `扫描视图 ${store.scanStats?.files ?? 0} 个文件`;
  }
  return '';
});

const chemPercent = computed(() =>
  store.chemTotal > 0
    ? Math.min(100, Math.round((store.chemDone / store.chemTotal) * 100))
    : 0,
);
</script>

<template>
  <div class="wm-statusbar">
    <span>{{ total }} 个项目</span>
    <span v-if="selected > 0">{{ selected }} 个已选择</span>
    <span v-if="scanInfo" class="wm-status-recursive">
      <el-icon style="vertical-align: middle"><Files /></el-icon>
      {{ scanInfo }}
    </span>
    <span v-if="store.chemLoading" class="wm-status-chem">
      <el-icon class="is-loading" style="vertical-align: middle"><Loading /></el-icon>
      <template v-if="store.chemTotal > 0">
        <span>解析化学信息</span>
        <strong>{{ store.chemDone }}/{{ store.chemTotal }}</strong>
        <span class="wm-status-chem-track">
          <span class="wm-status-chem-fill" :style="{ width: chemPercent + '%' }" />
        </span>
      </template>
      <template v-else>
        <span>读取化学信息…</span>
      </template>
    </span>
    <span class="spacer" />
    <span class="wm-status-machine">
      <el-icon style="vertical-align: middle">
        <Connection v-if="store.isRemote" />
        <Monitor v-else />
      </el-icon>
      {{ store.isRemote ? machine : '本机' }}
    </span>
    <span style="color: #999">{{ store.currentPath }}</span>
  </div>
</template>
