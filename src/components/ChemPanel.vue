<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';
import { chemColor, chemIcon, formatSize, kindShort, typeLabel } from '../utils/format';

const store = useExplorer();

// 预览的文件：优先取最近点击预览的文件，其次取勾选选中的第一个
const entry = computed(() => {
  if (store.previewPath) {
    return store.entries.find((e) => e.path === store.previewPath) ?? null;
  }
  return store.selectedEntries[0] ?? null;
});

const props = computed(() => {
  const e = entry.value;
  if (!e) return [];
  if (e.isDir) {
    return [
      { k: '名称', v: e.name },
      { k: '类型', v: '文件夹' },
      { k: '路径', v: e.path },
    ];
  }
  const rows: { k: string; v: string }[] = [
    { k: '名称', v: e.name },
    { k: '类型', v: typeLabel(e) },
    { k: '大小', v: formatSize(e.size) },
    { k: '路径', v: e.path },
  ];
  if (e.isChem && e.chem) {
    const c = e.chem;
    rows.push(
      { k: '化学式', v: c.formula || '—' },
      { k: '原子数', v: String(c.nAtoms) },
      { k: '总电荷', v: String(c.charge) },
      { k: '多重度', v: String(c.multip) },
      { k: 'α/β 电子', v: `${c.nAlpha}/${c.nBeta}` },
      { k: '基组函数', v: c.nBasis ? String(c.nBasis) : '—' },
      { k: '分子量', v: c.molW ? c.molW.toFixed(3) + ' g/mol' : '—' },
      { k: '能量', v: c.energy != null ? c.energy.toFixed(6) + ' a.u.' : '—' },
    );
  }
  return rows;
});
</script>

<template>
  <div class="wm-chempanel">
    <div class="wm-chempanel-header">
      预览
      <el-button link size="small" @click="store.toggleChemPanel()">关闭</el-button>
    </div>

    <template v-if="entry">
      <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 12px">
        <el-icon :size="26" :color="entry.isDir ? '#e8a33d' : '#7a7a7a'">
          <component :is="entry.isDir ? 'Folder' : chemIcon(entry.kind)" />
        </el-icon>
        <div style="flex: 1; min-width: 0">
          <div style="font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
            {{ entry.name }}
          </div>
          <div v-if="entry.isChem" class="wm-chem-tag" :style="{ background: chemColor(entry.kind) + '22', color: chemColor(entry.kind) }">
            {{ kindShort(entry.kind) }}
          </div>
        </div>
      </div>

      <div v-for="p in props" :key="p.k" class="prop-row">
        <span class="k">{{ p.k }}</span>
        <span class="v" :title="p.v">{{ p.v }}</span>
      </div>
    </template>

    <el-empty v-else description="选择要预览的文件" :image-size="60" />
  </div>
</template>
