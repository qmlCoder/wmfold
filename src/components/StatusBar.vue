<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();

const total = computed(() => store.entries.length);
const selected = computed(() => store.selectedPaths.length);
</script>

<template>
  <div class="wm-statusbar">
    <span>{{ total }} 个项目</span>
    <span v-if="selected > 0">{{ selected }} 个已选择</span>
    <span v-if="store.chemLoading" style="color: #888">
      <el-icon style="vertical-align: middle"><Loading /></el-icon> 读取化学信息…
    </span>
    <span class="spacer" />
    <span style="color: #999">{{ store.currentPath }}</span>
  </div>
</template>
