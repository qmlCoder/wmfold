<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import type { TableInstance } from 'element-plus';
import { useExplorer } from '../stores/explorer';
import { useContextMenu } from '../composables/useContextMenu';
import type { FileEntry, SortOrder } from '../types';
import { chemColor, fileIcon, formatDate, formatSize, kindShort, typeLabel } from '../utils/format';
import { CHEM_DEFS } from '../utils/chemColumns';

const store = useExplorer();
const { open } = useContextMenu();

// 当前应展示的化学列：按所选类型 + 用户设置中勾选的列
const visibleChemCols = computed(() => {
  if (!store.chemColsVisible) return [];
  return store.columnsFor(store.filterType)
    .map((k) => ({ key: k, prop: k, ...CHEM_DEFS[k] }))
    .filter((c) => c && c.prop in CHEM_DEFS);
});

function onSortChange({ prop, order }: { prop: string | null; order: SortOrder }) {
  store.setSort(prop ?? 'name', order ?? 'ascending');
}

function onSelectionChange(rows: FileEntry[]) {
  store.selectedPaths = rows.map((r) => r.path);
}

function onRowDblclick(row: FileEntry) {
  store.openEntry(row);
}

function onRowClick(row: FileEntry) {
  // 单击文件即在右侧预览对应文件
  store.setPreview(row.path);
}

function onRowContextmenu(row: FileEntry, _column: unknown, event: MouseEvent) {
  if (!store.selectedPaths.includes(row.path)) {
    store.selectedPaths = [row.path];
  }
  event?.preventDefault();
  open(event.clientX, event.clientY, row);
}

function iconName(row: FileEntry): string {
  return fileIcon(row);
}

function chemText(row: FileEntry, key: string): string {
  const c = row.chem;
  if (!row.isChem || !c) return '—';
  switch (key) {
    case 'formula':
      return c.formula || '—';
    case 'nAtoms':
      return c.nAtoms ? String(c.nAtoms) : '—';
    case 'charge':
      return String(c.charge);
    case 'multip':
      return String(c.multip);
    case 'nBasis':
      return c.nBasis ? String(c.nBasis) : '—';
    case 'molW':
      return c.molW ? c.molW.toFixed(3) : '—';
    case 'energy':
      return c.energy != null ? c.energy.toFixed(6) : '—';
    default:
      return '—';
  }
}

// 切换文件类型后重排表格布局，让列宽自动适配
const tableRef = ref<TableInstance | null>(null);
watch(
  () => store.filterType,
  async () => {
    await nextTick();
    tableRef.value?.doLayout?.();
  },
);
</script>

<template>
  <div class="wm-table" style="display: flex; flex-direction: column; height: 100%">
    <el-table
      ref="tableRef"
      v-loading="store.loading"
      :data="store.sortedEntries"
      :default-sort="{ prop: 'name', order: 'ascending' }"
      row-key="path"
      height="100%"
      highlight-current-row
      border
      @sort-change="onSortChange"
      @selection-change="onSelectionChange"
      @row-dblclick="onRowDblclick"
      @row-click="onRowClick"
      @row-contextmenu="onRowContextmenu"
    >
      <el-table-column type="selection" width="34" />

      <el-table-column label="名称" prop="name" sortable="custom" min-width="200" show-overflow-tooltip>
        <template #default="{ row }">
          <div class="wm-file-name-cell">
            <span class="wm-file-icon" :class="{ folder: row.isDir }">
              <el-icon><component :is="row.isDir ? 'Folder' : iconName(row)" /></el-icon>
            </span>
            <span class="wm-file-name">{{ row.name }}</span>
          </div>
        </template>
      </el-table-column>

      <el-table-column label="类型" prop="type" sortable="custom" width="110">
        <template #default="{ row }">
          <span v-if="row.isChem" class="wm-chem-tag" :style="{ background: chemColor(row.kind) + '22', color: chemColor(row.kind) }">
            {{ kindShort(row.kind) }} · {{ typeLabel(row) }}
          </span>
          <span v-else>{{ typeLabel(row) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="大小" prop="size" sortable="custom" width="90" align="right">
        <template #default="{ row }">
          <span class="wm-chem-num">{{ row.isDir ? '—' : formatSize(row.size) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="修改日期" prop="modified" sortable="custom" width="140">
        <template #default="{ row }">
          <span class="wm-chem-num">{{ formatDate(row.modified) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="创建日期" prop="created" sortable="custom" width="130">
        <template #default="{ row }">
          <span class="wm-chem-num">{{ formatDate(row.created) }}</span>
        </template>
      </el-table-column>

      <!-- 化学信息列：按所选类型动态展示，可收缩宽度以尽量适配窗口 -->
      <el-table-column
        v-for="c in visibleChemCols"
        :key="c.prop"
        :prop="c.prop"
        :label="c.label"
        :min-width="c.minWidth"
        :align="c.align || 'left'"
        :sortable="c.sortable ? 'custom' : false"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span :class="c.prop === 'formula' ? 'wm-chem-formula' : 'wm-chem-num'">
            {{ chemText(row, c.prop) }}
          </span>
        </template>
      </el-table-column>

      <template #empty>
        <div class="wm-empty">
          <el-icon><FolderOpened /></el-icon>
          <span>此文件夹为空</span>
        </div>
      </template>
    </el-table>
  </div>
</template>
