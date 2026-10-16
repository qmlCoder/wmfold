import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { api } from '../api/tauri';
import type { ChemMeta, DriveInfo, FileEntry, SortOrder } from '../types';
import { THIS_PC, breadcrumbPaths, childPath, parentPath } from '../utils/path';
import { CHEM_BY_KIND, FILTER_KINDS } from '../utils/chemColumns';

// 化学类型代码集合
const CHEM_KINDS = ['gjf', 'log', 'out', 'fch', 'mol', 'sdf', 'xyz', 'molden', 'dmo', 'inp', 'cub'];

function isChemKind(t: string): boolean {
  return CHEM_KINDS.includes(t);
}

// 当前筛选是否涉及化学（需要解析化学元数据并显示化学列）
function isChemFilter(t: string): boolean {
  return t === 'chem' || isChemKind(t);
}

function driveToEntry(d: DriveInfo): FileEntry {
  return {
    name: d.label,
    path: `${d.letter}:\\`,
    isDir: true,
    size: 0,
    created: null,
    modified: null,
    extension: '',
    kind: '',
    isChem: false,
    isHidden: false,
    isSymlink: false,
    chem: null,
  };
}

function getField(e: FileEntry, col: string): string | number | null {
  switch (col) {
    case 'name':
      return e.name;
    case 'type':
      return e.kind || e.extension;
    case 'size':
      return e.isDir ? -1 : e.size;
    case 'modified':
      return e.modified ?? 0;
    case 'created':
      return e.created ?? 0;
    case 'formula':
      return e.chem?.formula ?? '';
    case 'nAtoms':
      return e.chem?.nAtoms ?? Number.NEGATIVE_INFINITY;
    case 'charge':
      return e.chem?.charge ?? Number.NEGATIVE_INFINITY;
    case 'multip':
      return e.chem?.multip ?? Number.NEGATIVE_INFINITY;
    case 'nBasis':
      return e.chem?.nBasis ?? Number.NEGATIVE_INFINITY;
    case 'molW':
      return e.chem?.molW ?? Number.NEGATIVE_INFINITY;
    case 'energy':
      return e.chem?.energy ?? Number.NEGATIVE_INFINITY;
    default:
      return e.name;
  }
}

export const useExplorer = defineStore('explorer', () => {
  // ---- 状态 ----
  const currentPath = ref<string>(THIS_PC);
  const history = ref<string[]>([]);
  const historyIndex = ref(-1);
  const entries = ref<FileEntry[]>([]);
  const drives = ref<DriveInfo[]>([]);
  const quickAccess = ref<{ name: string; path: string; icon: string }[]>([]);
  const homeDir = ref('');
  const loading = ref(false);
  const chemLoading = ref(false);
  const selectedPaths = ref<string[]>([]);
  const sortColumn = ref<string>('name');
  const sortOrder = ref<SortOrder>('ascending');
  const showChemPanel = ref(true); // 预览窗格默认开启
  const previewPath = ref<string | null>(null);
  const search = ref('');
  const filterType = ref<string>('all'); // 'all' | 具体类型代码

  // 化学列是否可见：选中了化学类型时显示
  const chemColsVisible = computed(() => isChemFilter(filterType.value));

  // 设置：每种类型显示哪些化学列（默认显示该类型全部可用列）
  const columnSettings = ref<Record<string, string[]>>(
    Object.fromEntries(FILTER_KINDS.map((k) => [k, [...(CHEM_BY_KIND[k] ?? [])]])) as Record<string, string[]>,
  );

  function persistColumnSettings() {
    try {
      localStorage.setItem('wmfold.columnSettings', JSON.stringify(columnSettings.value));
    } catch {
      /* ignore */
    }
  }

  function columnsFor(kind: string): string[] {
    return columnSettings.value[kind] ?? [];
  }

  function isColumnEnabled(kind: string, col: string): boolean {
    return (columnSettings.value[kind] ?? []).includes(col);
  }

  function setColumnEnabled(kind: string, col: string, enabled: boolean) {
    const cur = new Set(columnSettings.value[kind] ?? CHEM_BY_KIND[kind] ?? []);
    if (enabled) cur.add(col);
    else cur.delete(col);
    columnSettings.value = { ...columnSettings.value, [kind]: [...cur] };
    persistColumnSettings();
  }

  function resetColumnSettings() {
    columnSettings.value = Object.fromEntries(
      FILTER_KINDS.map((k) => [k, [...(CHEM_BY_KIND[k] ?? [])]]),
    ) as Record<string, string[]>;
    persistColumnSettings();
  }
  const clipboard = ref<{ action: 'copy' | 'cut'; paths: string[] } | null>(null);
  const error = ref('');

  // ---- 派生状态 ----
  const crumbPaths = computed(() => breadcrumbPaths(currentPath.value));
  const isThisPc = computed(() => currentPath.value === THIS_PC);

  const sortedEntries = computed(() => {
    let list = entries.value;
    const q = search.value.trim().toLowerCase();
    if (q) list = list.filter((e) => e.name.toLowerCase().includes(q));
    // 类型筛选：只显示所选类型的文件
    const ft = filterType.value;
    if (ft !== 'all') {
      if (ft === 'folder') {
        list = list.filter((e) => e.isDir);
      } else if (ft === 'chem') {
        list = list.filter((e) => e.isChem);
      } else {
        list = list.filter((e) => e.kind === ft);
      }
    }

    const dirs = list.filter((e) => e.isDir);
    const files = list.filter((e) => !e.isDir);
    const cmp = (a: FileEntry, b: FileEntry) => {
      const va = getField(a, sortColumn.value);
      const vb = getField(b, sortColumn.value);
      let r = 0;
      if (typeof va === 'string' && typeof vb === 'string') {
        r = va.localeCompare(vb, 'en', { numeric: true, sensitivity: 'base' });
      } else {
        const na = typeof va === 'number' ? va : -Infinity;
        const nb = typeof vb === 'number' ? vb : -Infinity;
        r = na - nb;
        if (Number.isNaN(r)) r = 0;
      }
      if (sortOrder.value === 'descending') r = -r;
      return r;
    };
    dirs.sort(cmp);
    files.sort(cmp);
    return [...dirs, ...files];
  });

  const selectedEntries = computed(() =>
    entries.value.filter((e) => selectedPaths.value.includes(e.path)),
  );

  const hasSelection = computed(() => selectedEntries.value.length > 0);

  // 各文件类型在当前目录(未过滤)下的数量
  const typeCounts = computed(() => {
    const counts: Record<string, number> = { all: entries.value.length };
    for (const k of FILTER_KINDS) {
      counts[k] = entries.value.filter((e) => e.kind === k && !e.isDir).length;
    }
    return counts;
  });

  // ---- 动作 ----
  async function init() {
    try {
      homeDir.value = await api.getHomeDir();
      quickAccess.value = await api.getQuickAccess();
      drives.value = await api.getDrives();
    } catch (e) {
      error.value = String(e);
    }
    // 读取用户设置（每种类型显示哪些列）
    try {
      const saved = localStorage.getItem('wmfold.columnSettings');
      if (saved) Object.assign(columnSettings.value, JSON.parse(saved));
    } catch {
      /* ignore */
    }
    await refresh();
  }

  async function loadDrives() {
    drives.value = await api.getDrives();
  }

  async function navigate(path: string, pushHistory = true) {
    currentPath.value = path;
    selectedPaths.value = [];
    previewPath.value = null;
    if (pushHistory) {
      // 截断到当前索引，避免残留前向历史
      history.value = history.value.slice(0, historyIndex.value + 1);
      history.value.push(path);
      historyIndex.value = history.value.length - 1;
    }
    await loadContent();
  }

  // 为当前已加载的条目解析化学元数据（单独的 try/catch，失败不影响列表）
  async function fillChemForEntries() {
    const chemPaths = entries.value.filter((f) => f.isChem && !f.isDir).map((f) => f.path);
    if (!chemPaths.length) return;
    chemLoading.value = true;
    try {
      const metas = await api.readChemBatch(chemPaths);
      const byPath = new Map<string, ChemMeta>();
      chemPaths.forEach((p, i) => byPath.set(p, metas[i]));
      entries.value = entries.value.map((f) =>
        f.isChem ? { ...f, chem: byPath.get(f.path) ?? null } : f,
      );
    } catch {
      // 解析失败不阻塞列表展示
    } finally {
      chemLoading.value = false;
    }
  }

  async function loadContent() {
    loading.value = true;
    error.value = '';
    try {
      if (isThisPc.value) {
        entries.value = drives.value.map(driveToEntry);
        return;
      }
      const files = await api.listDir(currentPath.value);
      entries.value = files;
      // 仅在选中了化学类型时才解析，避免默认常规列表中做无谓解析
      if (chemColsVisible.value) {
        await fillChemForEntries();
      }
    } catch (e) {
      error.value = String(e);
      entries.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function refresh() {
    await loadContent();
  }

  function goBack() {
    if (historyIndex.value > 0) {
      historyIndex.value--;
      navigate(history.value[historyIndex.value], false);
    }
  }

  function goForward() {
    if (historyIndex.value < history.value.length - 1) {
      historyIndex.value++;
      navigate(history.value[historyIndex.value], false);
    }
  }

  function goUp() {
    const parent = parentPath(currentPath.value);
    if (parent !== currentPath.value) navigate(parent);
  }

  function toThisPc() {
    navigate(THIS_PC);
  }

  async function openEntry(e: FileEntry) {
    if (e.isDir) {
      await navigate(e.path);
    } else {
      try {
        await api.open(e.path);
      } catch (err) {
        error.value = String(err);
      }
    }
  }

  async function openEntryByPath(p: string) {
    const e = entries.value.find((x) => x.path === p);
    if (e) await openEntry(e);
  }

  async function createFolder(name: string) {
    const p = childPath(currentPath.value, name);
    await api.createFolder(p);
    await refresh();
  }

  async function createFile(name: string) {
    const p = childPath(currentPath.value, name);
    await api.createFile(p);
    await refresh();
  }

  async function deleteSelected(permanent: boolean) {
    if (!selectedPaths.value.length) return;
    await api.remove(selectedPaths.value, permanent);
    selectedPaths.value = [];
    await refresh();
  }

  async function renameEntry(e: FileEntry, newName: string) {
    await api.rename(e.path, newName);
    await refresh();
  }

  function copySelected(action: 'copy' | 'cut') {
    if (!selectedPaths.value.length) return;
    clipboard.value = { action, paths: [...selectedPaths.value] };
  }

  async function paste() {
    const clip = clipboard.value;
    if (!clip) return;
    const dest = currentPath.value;
    if (clip.action === 'copy') await api.copy(clip.paths, dest);
    else await api.move(clip.paths, dest);
    clipboard.value = null;
    await refresh();
  }

  function setSort(col: string, order: SortOrder) {
    sortColumn.value = col;
    sortOrder.value = order;
  }

  function setFilterType(t: string) {
    filterType.value = t;
    // 仅在需要化学数据且尚未解析时补充解析，避免整目录重取导致闪烁
    if (isChemFilter(t) && entries.value.some((e) => e.isChem && !e.chem)) {
      fillChemForEntries();
    }
  }

  function setPreview(p: string | null) {
    previewPath.value = p;
  }

  function toggleChemPanel() {
    showChemPanel.value = !showChemPanel.value;
  }

  return {
    currentPath,
    history,
    historyIndex,
    entries,
    drives,
    quickAccess,
    homeDir,
    loading,
    chemLoading,
    selectedPaths,
    sortColumn,
    sortOrder,
    chemColsVisible,
    filterType,
    columnSettings,
    columnsFor,
    isColumnEnabled,
    setColumnEnabled,
    resetColumnSettings,
    showChemPanel,
    previewPath,
    search,
    clipboard,
    error,
    crumbPaths,
    isThisPc,
    sortedEntries,
    selectedEntries,
    hasSelection,
    typeCounts,
    init,
    loadDrives,
    navigate,
    refresh,
    goBack,
    goForward,
    goUp,
    toThisPc,
    openEntry,
    openEntryByPath,
    createFolder,
    createFile,
    deleteSelected,
    renameEntry,
    copySelected,
    paste,
    setSort,
    setFilterType,
    setPreview,
    toggleChemPanel,
  };
});
