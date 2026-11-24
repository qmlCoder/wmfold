<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { api } from '../api/tauri';
import { useExplorer } from '../stores/explorer';
import { chemColor, chemIcon, formatSize, kindShort, typeLabel } from '../utils/format';
import type { ChemMeta } from '../types';
import MoleculeViewer from './MoleculeViewer.vue';

const store = useExplorer();

// 预览的文件：优先取最近点击预览的文件，其次取勾选选中的第一个
const entry = computed(() => {
  if (store.previewPath) {
    return store.entries.find((e) => e.path === store.previewPath) ?? null;
  }
  return store.selectedEntries[0] ?? null;
});

// 列表未开启化学列时 entry.chem 为空；预览面板仍应展示化学信息（含 gjf/log 的作业）。
const previewChem = ref<ChemMeta | null>(null);
let chemRequestId = 0;

watch(
  () => [entry.value?.path, entry.value?.chem] as const,
  async () => {
    const e = entry.value;
    const id = ++chemRequestId;
    if (!e || !e.isChem) {
      previewChem.value = null;
      return;
    }
    if (e.chem) {
      previewChem.value = e.chem;
      return;
    }
    previewChem.value = null;
    try {
      let meta: ChemMeta | null = null;
      if (store.isRemote) {
        if (store.activeRemote) {
          const info = await api.remoteInfo(store.activeRemote, e.path);
          meta = info.chem ?? null;
        }
      } else {
        meta = await api.readChem(e.path);
      }
      if (id === chemRequestId) previewChem.value = meta;
    } catch {
      /* 预览解析失败时保留基本信息即可 */
    }
  },
  { immediate: true },
);

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
  const c = e.chem ?? previewChem.value;
  if (e.isChem && c) {
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
    if (e.kind === 'gjf' || e.kind === 'log') {
      rows.push({ k: '作业', v: c.job || '—' });
    }
    if (e.kind === 'log') {
      rows.push({
        k: '正常结束',
        v: c.normalEnd == null ? '—' : c.normalEnd ? '正常结束' : '未正常结束',
      });
    }
  }
  return rows;
});
</script>

<template>
  <div class="wm-chempanel-content">
    <template v-if="entry">
      <MoleculeViewer
        v-if="entry.isChem"
        :path="entry.path"
        :profile="store.activeRemote"
      />

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

      <div
        v-for="p in props"
        :key="p.k"
        class="prop-row"
        :class="{ 'wm-job-row': p.k === '作业' }"
      >
        <span class="k">{{ p.k }}</span>
        <span class="v" :title="p.v">{{ p.v }}</span>
      </div>
    </template>

    <el-empty v-else description="选择要预览的文件" :image-size="60" />
  </div>
</template>
