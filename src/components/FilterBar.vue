<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';
import { FILTER_KINDS, KIND_LABELS } from '../utils/chemColumns';
import { chemColor } from '../utils/format';

const store = useExplorer();
const kinds = FILTER_KINDS;

const showRatio = computed(
  () => store.search.trim().length > 0 || store.filterType !== 'all',
);

function countText(kind: string): string {
  if (!showRatio.value) return String(store.typeCounts[kind] ?? 0);
  return `${store.filteredTypeCounts[kind] ?? 0} / ${store.typeCounts[kind] ?? 0}`;
}

function isActive(v: string) {
  return store.filterType === v;
}
</script>

<template>
  <div class="wm-filters">
    <button class="wm-filter" :class="{ 'is-active': isActive('all') }" @click="store.setFilterType('all')">
      <el-icon><Files /></el-icon>
      <span>全部文件</span>
      <span class="wm-filter-count" :class="{ 'is-ratio': showRatio }">{{ countText('all') }}</span>
    </button>
    <button
      v-for="k in kinds"
      :key="k"
      class="wm-filter"
      :class="{ 'is-active': isActive(k) }"
      @click="store.setFilterType(k)"
    >
      <span class="wm-filter-dot" :style="{ background: chemColor(k) }" />
      <span>{{ KIND_LABELS[k] }}</span>
      <span class="wm-filter-count" :class="{ 'is-ratio': showRatio }">{{ countText(k) }}</span>
    </button>
  </div>
</template>
