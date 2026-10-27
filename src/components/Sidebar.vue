<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();

type Q = { name: string; path: string; icon: string };

const thisPcItems = computed(() => store.quickAccess.filter((q: Q) => q.path === '::ThisPC'));
const folderItems = computed(() => store.quickAccess.filter((q: Q) => q.path !== '::ThisPC'));

function isActive(path: string): boolean {
  if (path === '::ThisPC') return store.isThisPc;
  return store.currentPath === path;
}
</script>

<template>
  <aside class="wm-sidebar">
    <div class="wm-sidebar-section">快速访问</div>
    <div
      v-for="q in thisPcItems"
      :key="q.path"
      class="wm-side-item"
      :class="{ 'is-active': isActive(q.path) }"
      @click="store.navigate(q.path)"
    >
      <el-icon><Monitor /></el-icon>
      <span>{{ q.name }}</span>
    </div>
    <div
      v-for="q in folderItems"
      :key="q.path"
      class="wm-side-item"
      :class="{ 'is-active': isActive(q.path) }"
      @click="store.navigate(q.path)"
    >
      <el-icon><Folder /></el-icon>
      <span>{{ q.name }}</span>
    </div>

    <div class="wm-sidebar-section">此电脑</div>
    <div
      v-for="d in store.drives"
      :key="d.letter"
      class="wm-side-item"
      :class="{ 'is-active': isActive(`${d.letter}:\\`) }"
      @click="store.navigate(`${d.letter}:\\`)"
    >
      <el-icon><Coin /></el-icon>
      <span>本地磁盘 ({{ d.letter }}:)</span>
    </div>
  </aside>
</template>
