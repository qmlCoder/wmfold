<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useExplorer } from '../stores/explorer';
import { useContextMenu } from '../composables/useContextMenu';
import type { FileEntry, SortOrder } from '../types';
import { chemColor, fileIcon, formatDate, formatSize, kindShort, typeLabel } from '../utils/format';
import { CHEM_DEFS } from '../utils/chemColumns';

const store = useExplorer();
const { open } = useContextMenu();

/** 虚拟表格的列定义（TableV2 的列只需 width/title/key + 可选 cell 渲染）。 */
interface VColumn {
  key: string;
  title: string;
  width: number;
  flexGrow?: number;
  align?: 'left' | 'right' | 'center';
  canSort?: boolean;
}

// 当前应展示的化学列：按所选类型 + 用户设置中勾选的列
const visibleChemCols = computed(() => {
  if (!store.chemColsVisible) return [];
  return store.columnsFor(store.filterType)
    .map((k) => ({ key: k, prop: k, ...CHEM_DEFS[k] }))
    .filter((c) => c && c.prop in CHEM_DEFS);
});

const columns = computed<VColumn[]>(() => {
  const cols: VColumn[] = [
    { key: '__check__', title: '', width: 46 },
    { key: 'name', title: '名称', width: 260, flexGrow: 1, canSort: true },
  ];
  if (store.recursiveMode) {
    cols.push({ key: 'relativeDir', title: '所在文件夹', width: 240 });
  }
  cols.push(
    { key: 'type', title: '类型', width: 130, canSort: true },
    { key: 'size', title: '大小', width: 100, align: 'right', canSort: true },
    { key: 'modified', title: '修改日期', width: 155, canSort: true },
    { key: 'created', title: '创建日期', width: 145, canSort: true },
  );
  for (const c of visibleChemCols.value) {
    cols.push({
      key: c.prop,
      title: c.label,
      width: Math.max(70, c.minWidth + 24),
      align: c.align,
      canSort: c.sortable,
    });
  }
  return cols;
});

// ---- 选择 ----
const selectedSet = computed(() => new Set(store.selectedPaths));
const checkedSet = computed(() => new Set(store.checkedPaths));

const allSelected = computed(() => {
  const rows = store.sortedEntries;
  return rows.length > 0 && rows.every((row) => checkedSet.value.has(row.path));
});

const someSelected = computed(() => {
  if (allSelected.value) return false;
  return store.sortedEntries.some((row) => checkedSet.value.has(row.path));
});

/** 多选 / 勾选：selectedPaths 与 checkedPaths 保持一致。 */
function setCheckedSelection(paths: string[]) {
  const unique = [...new Set(paths)];
  store.checkedPaths = unique;
  store.selectedPaths = unique;
}

function toggleAll(value: unknown) {
  const checked = value === true;
  if (checked) {
    setCheckedSelection(store.sortedEntries.map((row) => row.path));
  } else {
    const visible = new Set(store.sortedEntries.map((row) => row.path));
    store.checkedPaths = store.checkedPaths.filter((path) => !visible.has(path));
    store.selectedPaths = store.selectedPaths.filter((path) => !visible.has(path));
  }
}

function toggleRow(row: FileEntry, checked: boolean) {
  const next = new Set(store.checkedPaths);
  if (checked) next.add(row.path);
  else next.delete(row.path);
  setCheckedSelection([...next]);
  if (checked) store.activeRowPath = row.path;
}

// ---- 排序 ----
function sortByKey(key: string) {
  const order: SortOrder =
    store.sortColumn === key && store.sortOrder === 'ascending' ? 'descending' : 'ascending';
  store.setSort(key, order);
}

// ---- 行事件 ----
const rowEventHandlers = {
  onClick: (params: any) => {
    const row = params.rowData as FileEntry;
    const event = params.event as MouseEvent | undefined;
    const rows = store.sortedEntries;
    const clickedIndex = rows.findIndex((item) => item.path === row.path);

    if (event?.shiftKey && clickedIndex >= 0) {
      let anchorIndex = store.activeRowPath
        ? rows.findIndex((item) => item.path === store.activeRowPath)
        : -1;
      // 没有锚点时以当前点击行为锚点，本次只高亮、不勾选
      if (anchorIndex < 0) anchorIndex = clickedIndex;
      if (anchorIndex >= 0) {
        const start = Math.min(anchorIndex, clickedIndex);
        const end = Math.max(anchorIndex, clickedIndex);
        const range = rows.slice(start, end + 1).map((item) => item.path);
        // Shift 连选进入多选：左侧复选框全部勾选
        setCheckedSelection(range);
        store.activeRowPath = rows[anchorIndex].path;
        store.setPreview(row.path);
        return;
      }
    }

    // 普通单击：单选高亮，但不勾选左侧复选框
    store.selectedPaths = [row.path];
    store.checkedPaths = [];
    store.activeRowPath = row.path;
    store.setPreview(row.path);
  },
  onDblclick: (params: any) => {
    void store.openEntry(params.rowData);
  },
  onContextmenu: (params: any) => {
    const row = params.rowData as FileEntry;
    if (!store.selectedPaths.includes(row.path)) {
      store.selectedPaths = [row.path];
      store.checkedPaths = [];
      store.activeRowPath = row.path;
    }
    const event = params.event as MouseEvent;
    event?.preventDefault();
    open(event.clientX, event.clientY, row);
  },
};

function rowClass(params: any): string {
  return selectedSet.value.has(params.rowData.path) ? 'is-selected' : '';
}

// 虚拟表格只把可见行交给表格；这里让可见范围内的化学文件按需解析。
let chemTimer: ReturnType<typeof setTimeout> | null = null;
function onRowsRendered(params: {
  rowCacheStart: number;
  rowCacheEnd: number;
  rowVisibleStart: number;
  rowVisibleEnd: number;
}) {
  if (chemTimer) clearTimeout(chemTimer);
  // 连同预取（cache）行一起解析，并向前后多留一行余量，避免顶部半露的行一直显示“—”
  const start = Math.max(
    0,
    Math.min(params.rowCacheStart, params.rowVisibleStart) - 24,
  );
  const end = Math.max(params.rowCacheEnd, params.rowVisibleEnd) + 25;
  chemTimer = setTimeout(() => {
    store.ensureChemForRows(start, end);
  }, 120);
}

onBeforeUnmount(() => {
  if (chemTimer) clearTimeout(chemTimer);
});

// 目录 / 筛选 / 搜索 / 排序变化时重建表格，滚动位置自然回到顶部。
// TableV2 的 template ref 在 AutoResizer 插槽里不可靠，用 key 更稳妥。
const tableKey = computed(
  () =>
    `${store.currentPath}|${store.filterType}|${store.search}|${store.sortColumn}|${store.sortOrder}`,
);

// AI 结果点击后滚动到对应行；template ref 失效时用 TableV2 组件的 exposed 方法兜底。
const tableRef = ref<any>(null);
watch(
  () => store.revealRequest?.nonce,
  async () => {
    const path = store.revealRequest?.path;
    if (!path) return;
    await nextTick();
    await nextTick();
    const index = store.sortedEntries.findIndex(
      (entry) => entry.path.toLowerCase() === path.toLowerCase(),
    );
    if (index < 0) return;
    const instance =
      tableRef.value ??
      (document.querySelector('.wm-table .el-table-v2') as any)?.__vueParentComponent;
    const scrollToRow = instance?.scrollToRow ?? instance?.exposed?.scrollToRow;
    scrollToRow?.(index);
  },
);

/** 文件列表空白处：同样使用自定义右键菜单（新建 / 粘贴 / 刷新）。 */
function onBlankContextmenu(event: MouseEvent) {
  const el = event.target as HTMLElement;
  // 行内的右键由 rowEventHandlers.onContextmenu 处理；表头保留默认行为但屏蔽原生菜单
  if (el.closest('.el-table-v2__row')) return;
  event.preventDefault();
  if (el.closest('.el-table-v2__header')) return;
  store.selectedPaths = [];
  store.checkedPaths = [];
  store.activeRowPath = null;
  open(event.clientX, event.clientY, null);
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
    case 'job':
      return c.job || '—';
    case 'normalEnd':
      return c.normalEnd == null ? '—' : c.normalEnd ? '是' : '否';
    default:
      return '—';
  }
}

function chemCellClass(key: string): string {
  if (key === 'formula') return 'wm-chem-formula';
  if (key === 'job') return 'wm-chem-job';
  if (key === 'normalEnd') return 'wm-chem-normalend';
  return 'wm-chem-num';
}
</script>

<template>
  <div class="wm-table" style="display: flex; flex-direction: column; height: 100%" @contextmenu="onBlankContextmenu">
    <div class="wm-vtable-wrap" v-loading="store.loading">
      <el-auto-resizer>
        <template #default="{ height, width }">
          <el-table-v2
            ref="tableRef"
            :key="tableKey"
            v-if="width > 0 && height > 0"
            :columns="columns"
            :data="store.sortedEntries"
            :width="width"
            :height="height"
            :row-height="34"
            :header-height="34"
            row-key="path"
            :row-event-handlers="rowEventHandlers"
            :row-class="rowClass"
            :on-rows-rendered="onRowsRendered"
          >
            <template #header-cell="{ column }">
              <div v-if="column.key === '__check__'" class="wm-v2-checkhead">
                <el-checkbox
                  :model-value="allSelected"
                  :indeterminate="someSelected"
                  @change="toggleAll"
                />
              </div>
              <div
                v-else
                class="wm-v2-th"
                :class="{ 'is-right': column.align === 'right', 'is-sortable': column.canSort }"
                @click="column.canSort && sortByKey(String(column.key))"
              >
                <span class="wm-v2-th-label">{{ column.title }}</span>
                <span v-if="column.canSort" class="wm-v2-sort">
                  <template v-if="store.sortColumn === column.key">
                    <span v-if="store.sortOrder === 'ascending'">▲</span>
                    <span v-else>▼</span>
                  </template>
                  <span v-else class="wm-v2-sort-idle">↕</span>
                </span>
              </div>
            </template>

            <template #cell="{ column, rowData }">
              <div v-if="column.key === '__check__'" class="wm-v2-checkcell" @click.stop>
                <el-checkbox
                  :model-value="checkedSet.has(rowData.path)"
                  @click.stop
                  @change="(value: unknown) => toggleRow(rowData, value === true)"
                />
              </div>

              <div v-else-if="column.key === 'name'" class="wm-file-name-cell">
                <span class="wm-file-icon" :class="{ folder: rowData.isDir }">
                  <el-icon><component :is="rowData.isDir ? 'Folder' : iconName(rowData)" /></el-icon>
                </span>
                <span class="wm-file-name" :title="rowData.name">{{ rowData.name }}</span>
              </div>

              <div v-else-if="column.key === 'relativeDir'" class="wm-v2-cell">
                <span class="wm-file-reldir" :title="store.relativePath(rowData.path)">
                  {{ store.relativeDir(rowData.path) || '.' }}
                </span>
              </div>

              <div v-else-if="column.key === 'type'" class="wm-v2-cell">
                <span
                  v-if="rowData.isChem"
                  class="wm-chem-tag"
                  :style="{ background: chemColor(rowData.kind) + '22', color: chemColor(rowData.kind) }"
                >
                  {{ kindShort(rowData.kind) }} · {{ typeLabel(rowData) }}
                </span>
                <span v-else>{{ typeLabel(rowData) }}</span>
              </div>

              <div v-else-if="column.key === 'size'" class="wm-v2-cell is-right">
                <span class="wm-chem-num">{{ rowData.isDir ? '—' : formatSize(rowData.size) }}</span>
              </div>

              <div v-else-if="column.key === 'modified'" class="wm-v2-cell">
                <span class="wm-chem-num">{{ formatDate(rowData.modified) }}</span>
              </div>

              <div v-else-if="column.key === 'created'" class="wm-v2-cell">
                <span class="wm-chem-num">{{ formatDate(rowData.created) }}</span>
              </div>

              <div
                v-else
                class="wm-v2-cell"
                :class="{ 'is-right': column.align === 'right' }"
              >
                <span :class="chemCellClass(column.key)">{{ chemText(rowData, column.key) }}</span>
              </div>
            </template>

            <template #empty>
              <div class="wm-empty">
                <el-icon><FolderOpened /></el-icon>
                <span>{{ store.search || store.filterType !== 'all' ? '没有匹配的文件' : '此文件夹为空' }}</span>
              </div>
            </template>
          </el-table-v2>
        </template>
      </el-auto-resizer>
    </div>
  </div>
</template>
