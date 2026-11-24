import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from '../api/tauri';
import type {
  AiConfigView,
  AiToolEvent,
  AiToolStep,
  ChemMeta,
  DriveInfo,
  FileEntry,
  QuickAccess,
  RemoteProfile,
  RemoteStatus,
  RightPanelTab,
  ScanProgressEvent,
  ScanStats,
  SortOrder,
  SubmitJobResult,
} from '../types';
import { THIS_PC, breadcrumbPaths, childPath, parentPath } from '../utils/path';
import { CHEM_BY_KIND, FILTER_KINDS } from '../utils/chemColumns';

// 化学类型代码集合
const CHEM_KINDS = ['gjf', 'log', 'out', 'fch', 'mol', 'sdf', 'xyz', 'molden', 'dmo', 'inp', 'cub'];

/** 名称排序复用一个 Collator，避免对上万个文件逐次创建排序规则。 */
const NAME_COLLATOR = new Intl.Collator('en', { numeric: true, sensitivity: 'base' });

function isChemKind(t: string): boolean {
  return CHEM_KINDS.includes(t);
}

// 当前筛选是否涉及化学（需要解析化学元数据并显示化学列）
function isChemFilter(t: string): boolean {
  return t === 'chem' || isChemKind(t);
}

/** POSIX 路径的父目录（远程服务器用）。 */
function posixParent(p: string): string {
  const t = p.replace(/\/+$/, '');
  const i = t.lastIndexOf('/');
  return i <= 0 ? '/' : t.slice(0, i);
}

/** POSIX 面包屑。 */
function posixCrumbs(p: string): { label: string; path: string }[] {
  const parts = p.split('/').filter(Boolean);
  const out = [{ label: '/', path: '/' }];
  let acc = '';
  for (const part of parts) {
    acc += `/${part}`;
    out.push({ label: part, path: acc });
  }
  return out;
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
  // 文件条目可能上万条：用 shallowRef 避免把每个条目深度代理，递归读取时明显更快
  const entries = shallowRef<FileEntry[]>([]);
  const drives = ref<DriveInfo[]>([]);
  const quickAccess = ref<QuickAccess[]>([]);
  const homeDir = ref('');
  const loading = ref(false);
  const chemLoading = ref(false);
  /** 当前目录/扫描的化学文件总进度 */
  const chemDone = ref(0);
  const chemTotal = ref(0);
  /** 已尝试解析过化学信息的路径，避免重复入队 */
  const chemAttempted = new Set<string>();
  /** 后台解析队列；priority 中的路径是当前可见行，会先出队 */
  const chemPending = new Set<string>();
  const chemPriority = new Set<string>();
  let chemGeneration = 0;
  let chemWorkerRunning = false;
  const selectedPaths = ref<string[]>([]);
  /** 左侧勾选框真正选中的路径（单击只是当前行高亮，不勾选复选框）。 */
  const checkedPaths = ref<string[]>([]);
  /** Shift 连选的锚点行。 */
  const activeRowPath = ref<string | null>(null);
  // 任何清空 / 缩小 selectedPaths 的操作都同步丢弃已不存在的勾选状态
  watch(selectedPaths, (paths) => {
    if (!paths.length) {
      activeRowPath.value = null;
      checkedPaths.value = [];
      return;
    }
    if (!checkedPaths.value.length) return;
    const set = new Set(paths);
    checkedPaths.value = checkedPaths.value.filter((path) => set.has(path));
  });
  const sortColumn = ref<string>('name');
  const sortOrder = ref<SortOrder>('ascending');
  const showChemPanel = ref(true); // 预览窗格默认开启
  const previewPath = ref<string | null>(null);
  /** 请求文件列表滚动到某一行（AI 结果点击 / 定位用），nonce 保证重复点击也会触发。 */
  const revealRequest = ref<{ path: string; nonce: number } | null>(null);
  const search = ref('');
  const filterType = ref<string>('all'); // 'all' | 具体类型代码

  // ---- 递归读取 ----
  /** 当前是否处于递归视图（列表里是当前文件夹及所有子文件夹中的文件） */
  const recursiveMode = ref(false);
  /** 递归视图的根目录（扫描开始时的 currentPath） */
  const recursiveRoot = ref('');
  const scanning = ref(false);
  const scanId = ref(0);
  const scanProgress = ref<{ dirs: number; files: number; current: string; elapsedMs: number }>({
    dirs: 0,
    files: 0,
    current: '',
    elapsedMs: 0,
  });
  const scanStats = ref<ScanStats | null>(null);
  /** 单次递归读取的文件上限（避免误扫整个磁盘时占满内存） */
  const SCAN_MAX_ENTRIES = 100_000;
  let nextScanId = 1;

  // ---- 右侧面板 ----
  const rightPanelTab = ref<RightPanelTab>('preview');

  // ---- AI 对话 ----
  interface AiChatEntry {
    role: 'user' | 'assistant';
    content: string;
    /** 该轮对话发送时所在文件夹，用于解析回答里的相对路径 */
    rootPath?: string;
    /** 思考模式下的思维链，仅用于下一轮回传，不显示 */
    reasoningContent?: string;
    steps?: AiToolStep[];
    error?: string;
  }
  const aiMessages = ref<AiChatEntry[]>([]);
  const aiSending = ref(false);
  const aiConfig = ref<AiConfigView | null>(null);
  const aiConfigLoading = ref(false);
  const aiRequestId = ref(0);
  let nextAiRequestId = 1;

  // 化学列是否可见：选中了化学类型时显示
  const chemColsVisible = computed(() => isChemFilter(filterType.value));

  // 设置：每种类型显示哪些化学列（默认显示该类型全部可用列）
  const columnSettings = ref<Record<string, string[]>>(
    Object.fromEntries(FILTER_KINDS.map((k) => [k, [...(CHEM_BY_KIND[k] ?? [])]])) as Record<string, string[]>,
  );

  // 程序设置：隐藏文件（Windows 隐藏属性 / Linux 点文件 / 远程点文件）
  const showHidden = ref(false);
  // 程序设置：外部终端工具 auto / wt / powershell / cmd
  const terminalTool = ref('auto');

  function persistSettings() {
    // 写入 %APPDATA%\wmfold\settings.json；失败时仅在错误条上提示，不打断操作
    api
      .saveSettings({
        column_settings: { ...columnSettings.value },
        show_hidden: showHidden.value,
        terminal_tool: terminalTool.value,
      })
      .catch((e) => {
        error.value = `保存设置失败：${String(e)}`;
      });
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
    persistSettings();
  }

  function setShowHidden(value: boolean) {
    showHidden.value = value;
    persistSettings();
  }

  function setTerminalTool(value: string) {
    terminalTool.value = value;
    persistSettings();
  }

  function resetColumnSettings() {
    columnSettings.value = Object.fromEntries(
      FILTER_KINDS.map((k) => [k, [...(CHEM_BY_KIND[k] ?? [])]]),
    ) as Record<string, string[]>;
    persistSettings();
  }
  const clipboard = ref<{
    action: 'copy' | 'cut';
    sources: { path: string; profile: RemoteProfile | null }[];
  } | null>(null);
  const error = ref('');

  // ---- 远程机器 ----
  const remoteProfiles = ref<RemoteProfile[]>([]);
  /** 当前浏览的机器；null = 本机 */
  const activeRemote = ref<RemoteProfile | null>(null);
  const isRemote = computed(() => activeRemote.value !== null);
  /** 当前远程机器的用户主目录（用于远程「用户目录」快速访问项） */
  const remoteHome = ref('');
  /** 当前远程机器上用户固定的目录 */
  const remotePins = ref<QuickAccess[]>([]);
  /** 已连接机器的状态缓存：再次切换时不必重新 ssh 检查/安装 */
  const remoteStatusCache = ref<Record<string, RemoteStatus>>({});
  /** 远程机器固定项缓存：切换机器时先显示缓存，避免每次都 ssh 读取 */
  const remotePinsCache = ref<Record<string, QuickAccess[]>>({});

  // ---- 提交作业 ----
  /** 右键提交作业的源文件；profile 为 null 表示本机文件 */
  const submitSource = ref<{
    path: string;
    name: string;
    profile: RemoteProfile | null;
  } | null>(null);
  const submitDialogVisible = ref(false);
  const submitTargetId = ref('');
  const submitting = ref(false);
  const submitResult = ref<SubmitJobResult | null>(null);
  /** 配置了提交命令的机器 */
  const submitTargets = computed(() =>
    remoteProfiles.value.filter((p) => (p.submitCommand ?? '').trim().length > 0),
  );
  const submitTarget = computed(
    () => submitTargets.value.find((p) => p.id === submitTargetId.value) ?? null,
  );
  /** 目标机器当前/曾经打开的文件夹是否可直接作为提交目录 */
  const submitTargetFromOpenFolder = computed(() => {
    const target = submitTarget.value;
    if (!target) return false;
    if (activeRemote.value?.id === target.id) return true;
    const open =
      windowMachineIds.value.includes(target.id) ||
      detachedMachineIds.value.includes(target.id);
    return open && !!lastPaths.value[target.id];
  });
  /** 目标机器正在浏览时提交到当前文件夹；已打开过则用最后路径；否则使用默认提交目录 */
  const submitTargetDir = computed(() => {
    const target = submitTarget.value;
    if (!target) return '';
    if (
      activeRemote.value?.id === target.id &&
      currentPath.value &&
      currentPath.value !== THIS_PC
    ) {
      return currentPath.value;
    }
    if (submitTargetFromOpenFolder.value) {
      const last = lastPaths.value[target.id];
      if (last) return last;
    }
    return target.submitDir?.trim() || '~/wmfold';
  });

  // ---- 多窗口：当前窗口包含哪些机器 tab ----
  const windowLabel = ref('main');
  const windowMachineIds = ref<string[]>(['local']);
  /** 主窗口中已被拖出成独立窗口的机器（不显示对应 tab） */
  const detachedMachineIds = ref<string[]>([]);

  function removeWindowMachine(machineId: string) {
    const wasActive =
      machineId === 'local' ? !isRemote.value : activeRemote.value?.id === machineId;
    windowMachineIds.value = windowMachineIds.value.filter((id) => id !== machineId);
    if (windowLabel.value === 'main') {
      detachedMachineIds.value = [...new Set([...detachedMachineIds.value, machineId])];
    }
    // 拖走的是当前 tab 时，自动切换到窗口里剩下的机器
    if (wasActive && windowMachineIds.value.length > 0) {
      if (windowMachineIds.value.includes('local')) {
        void setActiveRemote(null);
      } else {
        const next =
          remoteProfiles.value.find((p) => windowMachineIds.value.includes(p.id)) ?? null;
        void setActiveRemote(next);
      }
    }
  }

  /** 其他窗口把机器 tab 拖进来（或独立窗口关闭后归还）时调用。 */
  function attachMachine(machineId: string, path: string) {
    detachedMachineIds.value = detachedMachineIds.value.filter((id) => id !== machineId);
    if (!windowMachineIds.value.includes(machineId)) {
      windowMachineIds.value = [...windowMachineIds.value, machineId];
    }
    if (path) {
      lastPaths.value = { ...lastPaths.value, [machineId]: path };
    }
  }

  /** 从标题栏「+」菜单打开一台已保存的机器（新开一个 tab 并切换过去）。 */
  async function addWindowMachine(machineId: string) {
    detachedMachineIds.value = detachedMachineIds.value.filter((id) => id !== machineId);
    if (!windowMachineIds.value.includes(machineId)) {
      windowMachineIds.value = [...windowMachineIds.value, machineId];
    }
    if (machineId === 'local') {
      await setActiveRemote(null);
      return;
    }
    const profile = remoteProfiles.value.find((p) => p.id === machineId);
    if (profile) await setActiveRemote(profile);
  }

  // ---- 界面尺寸 / 面板 ----
  const previewWidth = ref<number>(
    Math.min(Math.max(Number(localStorage.getItem('wmfold.previewWidth')) || 320, 220), 900),
  );

  function setPreviewWidth(width: number) {
    previewWidth.value = width;
    try {
      localStorage.setItem('wmfold.previewWidth', String(width));
    } catch {
      /* 忽略 */
    }
  }

  /** 用系统终端打开当前目录（远程机器会新开 ssh 窗口并 cd 到远端目录）。 */
  async function openTerminal(path?: string) {
    let target = path;
    if (!target) {
      target = isRemote.value
        ? currentPath.value
        : currentPath.value === THIS_PC
          ? homeDir.value
          : currentPath.value;
    }
    if (!target) return;
    try {
      await api.openInTerminal(target, isRemote.value ? activeRemote.value : null);
    } catch (e) {
      error.value = String(e);
    }
  }

  function profileCacheKey(profile: RemoteProfile): string {
    return `${profile.id}|${profile.host}|${profile.port}|${profile.user}`;
  }

  /** 每台机器最后停留的目录（切换机器时恢复） */
  const lastPaths = ref<Record<string, string>>({});
  /** 快速访问项是否可用（目录被删除 / 移动后置为 true） */
  const unavailableQuickAccess = ref<Record<string, boolean>>({});

  function machineKey(): string {
    return activeRemote.value?.id ?? 'local';
  }

  function quickAccessKey(path: string): string {
    return `${machineKey()}::${path}`;
  }

  function isQuickAccessAvailable(path: string): boolean {
    return unavailableQuickAccess.value[quickAccessKey(path)] !== true;
  }

  async function validateQuickAccess() {
    const next = { ...unavailableQuickAccess.value };

    const localItems = quickAccess.value.filter((q) => q.path !== '::ThisPC');
    if (localItems.length) {
      try {
        const flags = await api.pathsExist(localItems.map((q) => q.path));
        localItems.forEach((q, i) => {
          next[`local::${q.path}`] = !flags[i];
        });
      } catch {
        /* 检查失败时保持原状态 */
      }
    }

    const profile = activeRemote.value;
    if (profile && remotePins.value.length) {
      try {
        const flags = await api.remotePathsExist(profile, remotePins.value.map((q) => q.path));
        remotePins.value.forEach((q, i) => {
          next[`${profile.id}::${q.path}`] = !flags[i];
        });
      } catch {
        /* 检查失败时保持原状态 */
      }
    }

    unavailableQuickAccess.value = next;
  }

  /** 记住当前机器停留的目录。 */
  function rememberPath() {
    lastPaths.value = { ...lastPaths.value, [machineKey()]: currentPath.value };
  }

  interface MachineSnapshot {
    currentPath: string;
    history: string[];
    historyIndex: number;
    selectedPaths: string[];
    sortColumn: string;
    sortOrder: SortOrder;
    previewPath: string | null;
    search: string;
    filterType: string;
    showChemPanel: boolean;
  }

  /** 每台机器独立的界面状态（切换 tab 时保存/恢复）。 */
  const machineSnapshots = new Map<string, MachineSnapshot>();

  function saveMachineSnapshot() {
    machineSnapshots.set(machineKey(), {
      currentPath: currentPath.value,
      history: [...history.value],
      historyIndex: historyIndex.value,
      selectedPaths: [...selectedPaths.value],
      sortColumn: sortColumn.value,
      sortOrder: sortOrder.value,
      previewPath: previewPath.value,
      search: search.value,
      filterType: filterType.value,
      showChemPanel: showChemPanel.value,
    });
  }

  function restoreMachineSnapshot(key: string): boolean {
    const snapshot = machineSnapshots.get(key);
    if (!snapshot) return false;
    currentPath.value = snapshot.currentPath;
    history.value = [...snapshot.history];
    historyIndex.value = snapshot.historyIndex;
    selectedPaths.value = [...snapshot.selectedPaths];
    sortColumn.value = snapshot.sortColumn;
    sortOrder.value = snapshot.sortOrder;
    previewPath.value = snapshot.previewPath;
    search.value = snapshot.search;
    filterType.value = snapshot.filterType;
    showChemPanel.value = snapshot.showChemPanel;
    return true;
  }

  /** 当前机器对应的快速访问项：本机 = 后端默认项 + 固定项；远程 = 该机器的主目录 + 固定项。 */
  const activeQuickAccess = computed<QuickAccess[]>(() => {
    if (isRemote.value) {
      const items: QuickAccess[] = remoteHome.value
        ? [{ name: '用户目录', path: remoteHome.value, icon: 'home', pinned: false }]
        : [];
      for (const pin of remotePins.value) {
        if (!items.some((q) => q.path === pin.path)) items.push(pin);
      }
      return items;
    }
    return quickAccess.value;
  });

  // ---- 设置对话框 ----
  const settingsVisible = ref(false);
  const settingsTab = ref<'display' | 'program' | 'ai' | 'remote'>('display');

  // ---- 派生状态 ----
  const crumbPaths = computed(() =>
    isRemote.value ? posixCrumbs(currentPath.value) : breadcrumbPaths(currentPath.value),
  );
  const isThisPc = computed(() => !isRemote.value && currentPath.value === THIS_PC);

  /** 按「显示隐藏文件」设置过滤后的条目（列表与类型计数共用）。 */
  const visibleEntries = computed(() =>
    showHidden.value ? entries.value : entries.value.filter((e) => !e.isHidden),
  );

  /** 仅应用搜索（含扫描模式的相对路径）后的条目，用于类型按钮的“过滤数/总数”。 */
  const searchMatchedEntries = computed(() => {
    const q = search.value.trim().toLowerCase();
    if (!q) return visibleEntries.value;
    return visibleEntries.value.filter(
      (e) =>
        e.name.toLowerCase().includes(q) ||
        // 递归视图里也按相对路径搜索，方便在多层子目录中定位
        (recursiveMode.value && relativePath(e.path).toLowerCase().includes(q)),
    );
  });

  /** 经过搜索 / 类型筛选并排序后的全部条目（虚拟表格直接使用全量数据）。 */
  const sortedEntries = computed(() => {
    let list = searchMatchedEntries.value;
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
        r = NAME_COLLATOR.compare(va, vb);
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

  /**
   * 虚拟表格只渲染可见行；可见行会插队到后台全量解析队列前面，
   * 其余文件在后台继续解析，保证首屏先用上化学信息。
   */
  function ensureChemForRows(start: number, end: number) {
    if (!chemColsVisible.value) return;
    const rows = sortedEntries.value.slice(Math.max(0, start), Math.max(0, end));
    const paths = rows
      .filter((e) => e.isChem && !e.chem && !chemAttempted.has(e.path))
      .map((e) => e.path);
    if (paths.length) prioritizeChemPaths(paths);
  }

  /** 清空当前目录/扫描的解析队列（切换目录或重新扫描时调用）。 */
  function resetChemQueue() {
    chemGeneration += 1;
    chemPending.clear();
    chemPriority.clear();
    chemAttempted.clear();
    chemDone.value = 0;
    chemTotal.value = 0;
    chemLoading.value = false;
  }

  /** 把当前列表里的全部化学文件放进后台队列。 */
  function startChemQueue() {
    const paths = [
      ...new Set(
        entries.value
          .filter((entry) => entry.isChem && !entry.isDir)
          .map((entry) => entry.path),
      ),
    ];
    chemPending.clear();
    chemPriority.clear();
    chemAttempted.clear();
    for (const path of paths) chemPending.add(path);
    chemDone.value = 0;
    chemTotal.value = paths.length;
    chemLoading.value = paths.length > 0;
  }

  /** 可见行优先：从后台队列提到 priority。 */
  function prioritizeChemPaths(paths: string[]) {
    for (const path of paths) {
      if (chemAttempted.has(path)) continue;
      if (!chemPending.has(path)) {
        chemPending.add(path);
        chemTotal.value += 1;
      }
      chemPriority.add(path);
    }
    kickChemWorker();
  }

  function takeChemBatch(size: number): string[] {
    const out: string[] = [];
    for (const path of chemPriority) {
      chemPriority.delete(path);
      chemPending.delete(path);
      if (chemAttempted.has(path)) continue;
      out.push(path);
      if (out.length >= size) break;
    }
    if (out.length < size) {
      for (const path of chemPending) {
        if (chemAttempted.has(path)) {
          chemPending.delete(path);
          continue;
        }
        chemPending.delete(path);
        out.push(path);
        if (out.length >= size) break;
      }
    }
    return out;
  }

  function kickChemWorker() {
    if (chemWorkerRunning) return;
    if (!chemPending.size && !chemPriority.size) return;
    chemWorkerRunning = true;
    const generation = chemGeneration;
    void (async () => {
      try {
        while (generation === chemGeneration) {
          const chunk = takeChemBatch(6);
          if (!chunk.length) break;
          for (const path of chunk) chemAttempted.add(path);
          try {
            const metas = await api.readChemBatch(chunk);
            if (generation !== chemGeneration) return;
            const byPath = new Map<string, ChemMeta>();
            chunk.forEach((path, index) => byPath.set(path, metas[index]));
            entries.value = entries.value.map((entry) =>
              entry.isChem ? { ...entry, chem: byPath.get(entry.path) ?? entry.chem } : entry,
            );
          } catch {
            // 失败的块不阻塞后续；允许将来重新入队
            for (const path of chunk) chemAttempted.delete(path);
          }
          if (generation !== chemGeneration) return;
          chemDone.value += chunk.length;
          // 让出事件循环，保证滚动 / 点击仍然流畅
          await new Promise<void>((resolve) => setTimeout(resolve, 0));
        }
      } finally {
        chemWorkerRunning = false;
        if (generation === chemGeneration) chemLoading.value = false;
        void api.flushChemCache().catch(() => {});
        if (generation !== chemGeneration && (chemPending.size || chemPriority.size)) {
          kickChemWorker();
        }
      }
    })();
  }

  const selectedEntries = computed(() =>
    entries.value.filter((e) => selectedPaths.value.includes(e.path)),
  );

  const hasSelection = computed(() => selectedEntries.value.length > 0);

  function countKinds(list: FileEntry[]): Record<string, number> {
    const counts: Record<string, number> = { all: list.length };
    for (const k of FILTER_KINDS) {
      counts[k] = list.filter((e) => e.kind === k && !e.isDir).length;
    }
    return counts;
  }

  // 各文件类型在当前目录(未过滤)下的总数
  const typeCounts = computed(() => countKinds(visibleEntries.value));
  // 仅按搜索过滤后的数量，用于显示“过滤出的数量 / 总数量”
  const filteredTypeCounts = computed(() => countKinds(searchMatchedEntries.value));

  // ---- 动作 ----
  async function init() {
    let initialMachine: string | null = null;
    let initialPath = '';
    try {
      const label = getCurrentWindow().label;
      windowLabel.value = label;
      if (label !== 'main') {
        const raw = localStorage.getItem(`wmfold.window.${label}`);
        if (raw) {
          const info = JSON.parse(raw) as { machine?: string; path?: string };
          initialMachine = info.machine ?? null;
          initialPath = info.path ?? '';
        }
      }
    } catch {
      /* 非 Tauri 预览环境 */
    }

    try {
      homeDir.value = await api.getHomeDir();
      quickAccess.value = await api.getQuickAccess();
      drives.value = await api.getDrives();
      remoteProfiles.value = await api.remoteProfiles();
    } catch (e) {
      error.value = String(e);
    }
    // 读取用户设置（每种类型显示哪些列，存于 %APPDATA%\wmfold\settings.json）
    try {
      const saved = await api.loadSettings();
      if (saved?.column_settings) Object.assign(columnSettings.value, saved.column_settings);
      showHidden.value = saved?.show_hidden ?? false;
      terminalTool.value = saved?.terminal_tool || 'auto';
    } catch {
      /* 读取失败时沿用默认列 */
    }

    // 一次性迁移：老版本设置里没有「作业」列，为 gjf/log 补上；
    // 用户之后主动取消勾选时不会再被加回来。
    const jobColumnMigrationKey = 'wmfold.jobColumnMigrated.v1';
    if (!localStorage.getItem(jobColumnMigrationKey)) {
      const next = { ...columnSettings.value };
      let changed = false;
      for (const kind of ['gjf', 'log']) {
        const cols = next[kind] ?? [];
        if (!cols.includes('job')) {
          next[kind] = [...cols, 'job'];
          changed = true;
        }
      }
      if (changed) {
        columnSettings.value = next;
        persistSettings();
      }
      localStorage.setItem(jobColumnMigrationKey, '1');
    }

    // 0.3.34 新增 log「正常结束」列；已有用户配置里补上（一次性）
    const normalEndMigrationKey = 'wmfold.normalEndColumnMigrated.v1';
    if (!localStorage.getItem(normalEndMigrationKey)) {
      const next = { ...columnSettings.value };
      const cols = next.log ?? [];
      if (!cols.includes('normalEnd')) {
        next.log = [...cols, 'normalEnd'];
        columnSettings.value = next;
        persistSettings();
      }
      localStorage.setItem(normalEndMigrationKey, '1');
    }

    if (initialMachine) {
      // 从机器 tab 拖出的独立窗口：只包含这一台机器
      windowMachineIds.value = [initialMachine];
      if (initialMachine === 'local') {
        await refresh();
        if (initialPath) await navigate(initialPath);
      } else {
        const profile = remoteProfiles.value.find((p) => p.id === initialMachine);
        if (profile) await setActiveRemote(profile, initialPath || undefined);
        else {
          error.value = `未找到远程机器：${initialMachine}`;
          await refresh();
        }
      }
    } else {
      // 主窗口默认只打开「本机」；其他已保存的机器从标题栏「+」菜单打开
      windowMachineIds.value = ['local'];
      await refresh();
    }

    if (windowLabel.value !== 'main') {
      for (const id of windowMachineIds.value) {
        void api.registerMachineWindow(
          windowLabel.value,
          id,
          id === initialMachine ? initialPath : '',
        );
      }
    }
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
  // `paths` 可指定只解析当前可见的行；否则解析前若干个化学文件。
  async function loadContent() {
    // 普通目录浏览 / 切换机器时退出递归视图（并取消未完成的扫描）
    clearRecursiveState();
    resetChemQueue();
    if (isRemote.value) {
      await loadRemoteContent();
      return;
    }
    loading.value = true;
    error.value = '';
    try {
      if (isThisPc.value) {
        entries.value = drives.value.map(driveToEntry);
        rememberPath();
        return;
      }
      const files = await api.listDir(currentPath.value);
      entries.value = files;
      // 后台解析全部化学文件；首屏可见行插队优先
      startChemQueue();
      ensureChemForRows(0, 60);
      kickChemWorker();
      rememberPath();
    } catch (e) {
      error.value = String(e);
      entries.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function loadRemoteContent() {
    const profile = activeRemote.value;
    if (!profile) return;
    loading.value = true;
    error.value = '';
    try {
      const listing = await api.remoteList(profile, currentPath.value, chemColsVisible.value);
      entries.value = listing.entries;
      currentPath.value = listing.path;
      rememberPath();
    } catch (e) {
      error.value = String(e);
      entries.value = [];
    } finally {
      loading.value = false;
    }
  }

  /** 远程模式下按需取单个文件的化学信息（用于预览）。 */
  async function loadRemoteChem(path: string) {
    const profile = activeRemote.value;
    if (!profile) return;
    chemLoading.value = true;
    try {
      const entry = await api.remoteInfo(profile, path);
      entries.value = entries.value.map((e) => (e.path === path ? { ...e, chem: entry.chem } : e));
    } catch {
      /* 预览解析失败不影响浏览 */
    } finally {
      chemLoading.value = false;
    }
  }

  async function refresh() {
    // 递归视图下 F5 / 刷新按钮重新扫描同一个根目录
    if (recursiveMode.value) {
      await startRecursive();
      return;
    }
    await loadContent();
    void validateQuickAccess();
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
    const parent = isRemote.value ? posixParent(currentPath.value) : parentPath(currentPath.value);
    if (parent !== currentPath.value) navigate(parent);
  }

  function toThisPc() {
    activeRemote.value = null;
    navigate(THIS_PC);
  }

  // ---- 远程机器 ----

  /** 恢复某台机器的状态并加载目录（无快照时回到 fallbackPath）。 */
  async function restoreAndLoad(key: string, fallbackPath: string, initialPath?: string) {
    const restored = restoreMachineSnapshot(key);
    let target: string;
    if (initialPath) {
      target = initialPath;
      currentPath.value = target;
      history.value = [target];
      historyIndex.value = 0;
      selectedPaths.value = [];
      previewPath.value = null;
    } else if (restored) {
      target = currentPath.value;
    } else {
      target = lastPaths.value[key] || fallbackPath;
      currentPath.value = target;
      history.value = [target];
      historyIndex.value = 0;
      selectedPaths.value = [];
      previewPath.value = null;
    }
    await loadContent();
    if (error.value && target !== fallbackPath) {
      error.value = '';
      currentPath.value = fallbackPath;
      history.value = [fallbackPath];
      historyIndex.value = 0;
      selectedPaths.value = [];
      previewPath.value = null;
      await loadContent();
    }
  }

  async function setActiveRemote(profile: RemoteProfile | null, initialPath?: string) {
    if (!profile) {
      saveMachineSnapshot();
      activeRemote.value = null;
      remoteHome.value = '';
      remotePins.value = [];
      history.value = [];
      historyIndex.value = -1;
      await restoreAndLoad('local', THIS_PC, initialPath);
      void validateQuickAccess();
      return;
    }
    saveMachineSnapshot();
    loading.value = true;
    error.value = '';
    const cacheKey = profileCacheKey(profile);
    try {
      let status: RemoteStatus;
      const cached = remoteStatusCache.value[cacheKey];
      if (cached?.installed && cached.wmfoldVersion) {
        // 已连接过：跳过远程检查/安装，保持连接状态，切换更快
        status = cached;
      } else {
        status = await api.remoteCheck(profile);
        if (!status.installed || !status.wmfoldVersion || status.outdated) {
          await api.remoteInstall(profile);
          status = await api.remoteCheck(profile);
        }
        remoteStatusCache.value = { ...remoteStatusCache.value, [cacheKey]: status };
      }
      activeRemote.value = profile;
      remoteHome.value = status.home || '/';
      const cachedPins = remotePinsCache.value[profile.id];
      if (cachedPins) {
        remotePins.value = cachedPins;
      } else {
        remotePins.value = await api.remoteQuickAccess(profile.id).catch(() => []);
        remotePinsCache.value = { ...remotePinsCache.value, [profile.id]: remotePins.value };
      }
      // 恢复这台机器自己的目录 / 筛选 / 选择 / 终端
      await restoreAndLoad(profile.id, remoteHome.value, initialPath);
      void validateQuickAccess();
    } catch (e) {
      delete remoteStatusCache.value[cacheKey];
      error.value = String(e);
      activeRemote.value = null;
      remoteHome.value = '';
      remotePins.value = [];
    } finally {
      loading.value = false;
    }
  }

  /** 切回本机并打开指定路径（快速访问 / 磁盘用）。 */
  async function goLocal(path: string = THIS_PC) {
    saveMachineSnapshot();
    activeRemote.value = null;
    remoteHome.value = '';
    remotePins.value = [];
    restoreMachineSnapshot('local');
    await navigate(path);
    void validateQuickAccess();
  }

  /** 重新读取当前远程机器的固定项。 */
  async function refreshRemoteQuickAccess() {
    const profile = activeRemote.value;
    if (!profile) {
      remotePins.value = [];
      return;
    }
    try {
      remotePins.value = await api.remoteQuickAccess(profile.id);
      remotePinsCache.value = { ...remotePinsCache.value, [profile.id]: remotePins.value };
    } catch (e) {
      error.value = String(e);
    }
  }

  async function refreshRemoteProfiles() {
    try {
      remoteProfiles.value = await api.remoteProfiles();
    } catch (e) {
      error.value = String(e);
    }
    // 当前机器正在浏览时同步最新配置（密码 / 作业查询设置改完立即生效）
    if (activeRemote.value) {
      const updated = remoteProfiles.value.find((p) => p.id === activeRemote.value?.id);
      if (updated) activeRemote.value = updated;
    }
    if (windowLabel.value === 'main') {
      // 保留已经打开的 tab；只清理已被删除的机器
      const open = windowMachineIds.value.filter(
        (id) => id === 'local' || remoteProfiles.value.some((p) => p.id === id),
      );
      if (open.length !== windowMachineIds.value.length) {
        windowMachineIds.value = open;
        if (activeRemote.value && !open.includes(activeRemote.value.id)) {
          void setActiveRemote(null);
        }
      }
    }
  }

  function openSettings(tab: 'display' | 'program' | 'ai' | 'remote' = 'display') {
    settingsTab.value = tab;
    settingsVisible.value = true;
  }

  /** 右键「提交作业」：记录源文件并打开选择机器的对话框。 */
  function openSubmitJob(path: string) {
    const name = path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;
    submitSource.value = { path, name, profile: activeRemote.value };
    submitTargetId.value = submitTargets.value[0]?.id ?? '';
    submitResult.value = null;
    submitDialogVisible.value = true;
  }

  async function submitJob() {
    const source = submitSource.value;
    const target = submitTarget.value;
    if (!source || !target || submitting.value) return;
    submitting.value = true;
    submitResult.value = null;
    try {
      submitResult.value = await api.submitJob({
        sourceProfile: source.profile,
        sourcePath: source.path,
        targetProfile: target,
        targetDir: submitTargetDir.value || null,
      });
      if (submitResult.value.ok && activeRemote.value?.id === target.id) {
        await refresh();
      }
    } catch (e) {
      error.value = String(e);
    } finally {
      submitting.value = false;
    }
  }

  /** 远程模式下禁用本机文件操作。 */
  function requireLocal(action: string): boolean {
    if (isRemote.value) {
      error.value = `远程模式下暂不支持${action}`;
      return false;
    }
    return true;
  }

  /** 在系统文件资源管理器中打开某个文件夹；不传参数时打开当前文件夹。 */
  async function openInExplorer(path?: string) {
    if (isRemote.value) {
      error.value = '远程目录暂不支持在文件资源管理器中打开';
      return;
    }
    const target = path ?? currentPath.value;
    if (!target || target === THIS_PC) {
      error.value = '请先进入一个文件夹，再在文件资源管理器中打开';
      return;
    }
    try {
      await api.openInExplorer(target);
    } catch (e) {
      error.value = String(e);
    }
  }

  async function openEntry(e: FileEntry) {    if (e.isDir) {
      await navigate(e.path);
      return;
    }
    if (isRemote.value) {
      setPreview(e.path);
      return;
    }
    try {
      await api.open(e.path);
    } catch (err) {
      error.value = String(err);
    }
  }

  async function openEntryByPath(p: string) {
    const e = entries.value.find((x) => x.path === p);
    if (e) await openEntry(e);
  }

  /** 统一比较本机路径（大小写与分隔符不敏感）。 */
  function normalizeLocalPath(p: string): string {
    return p.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
  }

  function requestReveal(path: string) {
    revealRequest.value = {
      path,
      nonce: (revealRequest.value?.nonce ?? 0) + 1,
    };
  }

  /**
   * AI 工具结果点击后的定位：
   * - 扫描模式：保留扫描结果，文件直接选中/预览并滚动；文件夹用搜索限定到该子目录。
   * - 普通模式：文件夹直接进入；文件进入其父目录后选中并滚动。
   */
  async function revealAiPath(path: string, isDir: boolean) {
    if (!path) return;
    if (isRemote.value) {
      error.value = 'AI 结果定位暂仅支持本机文件';
      return;
    }

    if (recursiveMode.value) {
      if (isDir) {
        filterType.value = 'all';
        selectedPaths.value = [];
        checkedPaths.value = [];
        activeRowPath.value = null;
        previewPath.value = null;
        const rel = relativePath(path).replace(/[\\/]+$/, '').replace(/\//g, '\\');
        search.value =
          normalizeLocalPath(path) === normalizeLocalPath(recursiveRoot.value)
            ? ''
            : rel
              ? `${rel}\\`
              : '';
        return;
      }
      const entry = entries.value.find(
        (item) => !item.isDir && normalizeLocalPath(item.path) === normalizeLocalPath(path),
      );
      if (!entry) {
        error.value = '扫描结果中没有找到该文件';
        return;
      }
      if (
        !sortedEntries.value.some(
          (item) => normalizeLocalPath(item.path) === normalizeLocalPath(entry.path),
        )
      ) {
        filterType.value = 'all';
        search.value = '';
      }
      selectedPaths.value = [entry.path];
      checkedPaths.value = [];
      activeRowPath.value = entry.path;
      setPreview(entry.path);
      requestReveal(entry.path);
      return;
    }

    if (isDir) {
      await navigate(path);
      return;
    }

    const parent = parentPath(path);
    if (normalizeLocalPath(parent) !== normalizeLocalPath(currentPath.value)) {
      await navigate(parent);
    }
    const entry = entries.value.find(
      (item) => !item.isDir && normalizeLocalPath(item.path) === normalizeLocalPath(path),
    );
    if (!entry) {
      error.value = '文件列表中未找到该文件';
      return;
    }
    if (
      !sortedEntries.value.some(
        (item) => normalizeLocalPath(item.path) === normalizeLocalPath(entry.path),
      )
    ) {
      filterType.value = 'all';
      search.value = '';
    }
    selectedPaths.value = [entry.path];
    checkedPaths.value = [];
    activeRowPath.value = entry.path;
    setPreview(entry.path);
    requestReveal(entry.path);
  }

  /**
   * 把文件夹添加到快速访问（持久化为用户固定项）。
   * @returns 是否新加入；已在内置列表或已固定时为 false
   */
  async function addToQuickAccess(path: string): Promise<boolean> {
    if (isRemote.value) {
      const profile = activeRemote.value;
      if (!profile) return false;
      const added = await api.remoteAddQuickAccess(profile.id, path);
      await refreshRemoteQuickAccess();
      await validateQuickAccess();
      return added;
    }
    const added = await api.addQuickAccess(path);
    quickAccess.value = await api.getQuickAccess();
    await validateQuickAccess();
    return added;
  }

  /** 从快速访问中取消固定（本机 / 远程都支持）。 */
  async function removeFromQuickAccess(path: string) {
    if (isRemote.value) {
      const profile = activeRemote.value;
      if (!profile) return;
      await api.remoteRemoveQuickAccess(profile.id, path);
      await refreshRemoteQuickAccess();
      await validateQuickAccess();
      return;
    }
    await api.removeQuickAccess(path);
    quickAccess.value = await api.getQuickAccess();
    await validateQuickAccess();
  }

  async function createFolder(name: string) {
    if (!requireLocal('新建文件夹')) return;
    const p = childPath(currentPath.value, name);
    await api.createFolder(p);
    await refresh();
  }

  async function createFile(name: string) {
    if (!requireLocal('新建文件')) return;
    const p = childPath(currentPath.value, name);
    await api.createFile(p);
    await refresh();
  }

  async function deleteSelected(permanent: boolean) {
    if (!requireLocal('删除')) return;
    if (!selectedPaths.value.length) return;
    await api.remove(selectedPaths.value, permanent);
    selectedPaths.value = [];
    await refresh();
  }

  async function renameEntry(e: FileEntry, newName: string) {
    if (!requireLocal('重命名')) return;
    await api.rename(e.path, newName);
    await refresh();
  }

  function copySelected(action: 'copy' | 'cut') {
    if (!selectedPaths.value.length) return;
    // 记录每个来源属于哪台机器（null = 本机），粘贴时可跨设备传输
    clipboard.value = {
      action,
      sources: selectedPaths.value.map((path) => ({ path, profile: activeRemote.value })),
    };
  }

  async function paste() {
    const clip = clipboard.value;
    if (!clip) return;
    const destRemote = activeRemote.value;
    if (!destRemote && isThisPc.value) {
      error.value = '请先进入目标文件夹再粘贴';
      return;
    }
    const dest = currentPath.value;
    loading.value = true;
    error.value = '';
    try {
      const localSources = clip.sources.filter((s) => !s.profile).map((s) => s.path);
      const remoteGroups = new Map<string, { profile: RemoteProfile; paths: string[] }>();
      for (const s of clip.sources) {
        if (!s.profile) continue;
        const group = remoteGroups.get(s.profile.id);
        if (group) group.paths.push(s.path);
        else remoteGroups.set(s.profile.id, { profile: s.profile, paths: [s.path] });
      }

      if (!destRemote) {
        // 目标在本机
        if (localSources.length) {
          if (clip.action === 'copy') await api.copy(localSources, dest);
          else await api.move(localSources, dest);
        }
        for (const group of remoteGroups.values()) {
          await api.remoteDownload(group.profile, group.paths, dest);
          if (clip.action === 'cut') await api.remoteDelete(group.profile, group.paths);
        }
      } else {
        // 目标在远程
        if (localSources.length) {
          await api.remoteUpload(destRemote, localSources, dest);
          // 跨设备剪切：传输成功后清理本地源
          if (clip.action === 'cut') await api.remove(localSources, true);
        }
        for (const group of remoteGroups.values()) {
          if (group.profile.id === destRemote.id) {
            // 同一台远程机器内：直接 cp / mv
            await api.remoteCopyWithin(group.profile, group.paths, dest, clip.action === 'cut');
          } else {
            // 远程 A → 远程 B：经本机临时目录中转
            await api.remoteTransfer(
              group.profile,
              destRemote,
              group.paths,
              dest,
              clip.action === 'cut',
            );
          }
        }
      }

      clipboard.value = null;
      await refresh();
    } catch (e) {
      error.value = `粘贴失败：${String(e)}`;
    } finally {
      loading.value = false;
    }
  }

  function setSort(col: string, order: SortOrder) {
    sortColumn.value = col;
    sortOrder.value = order;
  }

  function setFilterType(t: string) {
    filterType.value = t;
    if (isRemote.value) {
      // 远程模式：切到化学类型且尚未解析时，重新拉取带化学信息的列表
      if (isChemFilter(t) && entries.value.some((e) => e.isChem && !e.chem)) {
        loadRemoteContent();
      }
      return;
    }
    // 仅在需要化学数据且尚未解析时补充解析；虚拟表格会继续按可见行补齐
    if (isChemFilter(t) && entries.value.some((e) => e.isChem && !e.chem)) {
      ensureChemForRows(0, 60);
    }
  }

  /** 清除搜索和类型过滤，恢复完整列表（AI 定位文件夹后的“取消过滤”也走这里）。 */
  function clearFilters() {
    search.value = '';
    filterType.value = 'all';
  }

  function setPreview(p: string | null) {
    previewPath.value = p;
    if (p && isRemote.value) {
      const e = entries.value.find((x) => x.path === p);
      if (e && e.isChem && !e.chem) void loadRemoteChem(p);
    }
  }

  function toggleChemPanel() {
    showChemPanel.value = !showChemPanel.value;
  }

  // ---- 递归读取 ----

  /** 递归视图中文件相对于根目录的路径。 */
  function relativePath(path: string): string {
    const root = recursiveRoot.value.replace(/[\\/]+$/, '');
    if (!root) return path;
    if (path.length > root.length && path.slice(0, root.length).toLowerCase() === root.toLowerCase()) {
      const rest = path.slice(root.length).replace(/^[\\/]+/, '');
      return rest || path;
    }
    return path;
  }

  /** 递归视图中文件所在的子文件夹（用于名称列的第二行）。 */
  function relativeDir(path: string): string {
    const rel = relativePath(path);
    const index = Math.max(rel.lastIndexOf('\\'), rel.lastIndexOf('/'));
    return index > 0 ? rel.slice(0, index) : '';
  }

  /** 退出递归视图并取消正在进行的扫描（不重新加载目录）。 */
  function clearRecursiveState() {
    if (scanning.value && scanId.value) {
      void api.cancelScan(scanId.value).catch(() => {});
    }
    scanning.value = false;
    recursiveMode.value = false;
    recursiveRoot.value = '';
    scanStats.value = null;
    scanProgress.value = { dirs: 0, files: 0, current: '', elapsedMs: 0 };
    scanId.value = 0;
  }

  /** 用递归方式读取当前文件夹及其所有子文件夹中的文件。 */
  async function startRecursive() {
    if (isRemote.value) {
      error.value = '扫描暂仅支持本机文件夹';
      return;
    }
    if (currentPath.value === THIS_PC) {
      error.value = '请先进入一个文件夹，再使用扫描';
      return;
    }
    if (scanning.value) return;

    const root = currentPath.value;
    const id = nextScanId++;
    scanId.value = id;
    scanning.value = true;
    loading.value = true;
    recursiveMode.value = false;
    recursiveRoot.value = '';
    scanStats.value = null;
    scanProgress.value = { dirs: 0, files: 0, current: root, elapsedMs: 0 };
    entries.value = [];
    resetChemQueue();
    selectedPaths.value = [];
    previewPath.value = null;
    error.value = '';

    try {
      const result = await api.scanRecursive(root, id, showHidden.value, SCAN_MAX_ENTRIES);
      // 扫描期间用户可能已经导航到别的目录 / 重新扫描，过期结果直接丢弃
      if (scanId.value !== id) return;
      entries.value = result.entries;
      recursiveRoot.value = root;
      recursiveMode.value = true;
      scanStats.value = {
        dirs: result.dirs,
        files: result.files,
        skipped: result.skipped,
        truncated: result.truncated,
        cancelled: result.cancelled,
        elapsedMs: result.elapsedMs,
      };
      if (result.entries.length) {
        startChemQueue();
        ensureChemForRows(0, 60);
        kickChemWorker();
      }
    } catch (e) {
      if (scanId.value === id) {
        error.value = `扫描失败：${String(e)}`;
        clearRecursiveState();
        await loadContent();
      }
    } finally {
      if (scanId.value === id) {
        scanning.value = false;
        loading.value = false;
      }
    }
  }

  /** 请求取消当前递归扫描（后端返回已读取的部分结果）。 */
  async function cancelRecursiveScan() {
    if (!scanning.value || !scanId.value) return;
    try {
      await api.cancelScan(scanId.value);
    } catch {
      /* 取消失败时让扫描自然结束 */
    }
  }

  /** 退出递归视图，回到普通目录列表。 */
  async function stopRecursive() {
    if (scanning.value) await cancelRecursiveScan();
    clearRecursiveState();
    await loadContent();
  }

  /** 后端 `scan-progress` 事件。 */
  function onScanProgress(event: ScanProgressEvent) {
    if (event.scanId !== scanId.value) return;
    scanProgress.value = {
      dirs: event.dirs,
      files: event.files,
      current: event.current,
      elapsedMs: event.elapsedMs,
    };
  }

  // ---- AI 对话 ----

  /** 打开右侧 AI 对话面板（必要时把面板加宽）。 */
  function openAiPanel() {
    if (!showChemPanel.value) showChemPanel.value = true;
    rightPanelTab.value = 'ai';
    if (previewWidth.value < 380) setPreviewWidth(420);
    void loadAiConfig();
  }

  async function loadAiConfig(force = false) {
    if (aiConfig.value && !force) return;
    aiConfigLoading.value = true;
    try {
      aiConfig.value = await api.aiConfig();
    } catch (e) {
      error.value = String(e);
    } finally {
      aiConfigLoading.value = false;
    }
  }

  /** 保存 API Key / 模型 / Base URL / 思考模式，返回最新配置。 */
  async function saveAiConfig(
    apiKey: string,
    model: string,
    baseUrl: string,
    thinking: boolean,
    reasoningEffort: string,
  ) {
    const saved = await api.aiSaveConfig(apiKey, model, baseUrl, thinking, reasoningEffort);
    aiConfig.value = saved;
    return saved;
  }

  /** 测试 DeepSeek 连接。 */
  function testAi() {
    return api.aiTest();
  }

  function clearAiChat() {
    aiMessages.value = [];
  }

  /** 把工具调用事件合并到当前回答的步骤列表。 */
  function onAiToolEvent(event: AiToolEvent) {
    if (event.requestId !== aiRequestId.value || !aiSending.value) return;
    for (let i = aiMessages.value.length - 1; i >= 0; i--) {
      const message = aiMessages.value[i];
      if (message.role !== 'assistant') continue;
      const steps = message.steps ? [...message.steps] : [];
      const index = steps.findIndex((step) => step.id === event.step.id);
      if (index >= 0) steps[index] = event.step;
      else steps.push(event.step);
      message.steps = steps;
      return;
    }
  }

  /** 发送一条 AI 消息（后端会自动调用工具读取文件）。 */
  async function sendAiMessage(raw: string) {
    const content = raw.trim();
    if (!content || aiSending.value) return;
    if (isRemote.value) {
      error.value = 'AI 对话暂仅支持本机文件，请切换到本机后使用';
      return;
    }
    if (currentPath.value === THIS_PC) {
      error.value = '请先进入一个文件夹，再使用 AI 对话';
      return;
    }

    await loadAiConfig();
    if (!aiConfig.value || aiConfig.value.keySource === 'none') {
      error.value =
        '尚未配置 DeepSeek API Key：请到 设置 → AI 对话 填写，或设置环境变量 DEEPSEEK_APIKEY';
      openSettings('ai');
      return;
    }

    const requestId = nextAiRequestId++;
    aiRequestId.value = requestId;
    const rootPath = currentPath.value;
    aiMessages.value.push({ role: 'user', content, rootPath });
    const placeholder: AiChatEntry = { role: 'assistant', content: '', rootPath, steps: [] };
    aiMessages.value.push(placeholder);
    aiSending.value = true;

    try {
      // 只发送真实有内容的消息，跳过正在生成的占位回答
      const history = aiMessages.value
        .filter((message) => !message.error && message.content.trim().length > 0)
        .slice(-20)
        .map((message) => ({
          role: message.role,
          content: message.content,
          ...(message.reasoningContent
            ? { reasoningContent: message.reasoningContent }
            : {}),
        }));
      const reply = await api.aiChat({
        requestId,
        root: currentPath.value,
        messages: history,
        selected: selectedPaths.value.slice(0, 30),
        recursive: recursiveMode.value,
      });
      if (aiRequestId.value !== requestId) return;
      const index = aiMessages.value.indexOf(placeholder);
      if (index >= 0) {
        aiMessages.value[index] = {
          role: 'assistant',
          content: reply.content,
          rootPath: placeholder.rootPath,
          reasoningContent: reply.reasoningContent,
          steps: reply.steps,
        };
      }
    } catch (e) {
      placeholder.error = String(e);
    } finally {
      if (aiRequestId.value === requestId) aiSending.value = false;
    }
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
    chemDone,
    chemTotal,
    selectedPaths,
    checkedPaths,
    activeRowPath,
    sortColumn,
    sortOrder,
    chemColsVisible,
    filterType,
    columnSettings,
    columnsFor,
    isColumnEnabled,
    setColumnEnabled,
    resetColumnSettings,
    showHidden,
    setShowHidden,
    terminalTool,
    setTerminalTool,
    showChemPanel,
    previewPath,
    revealRequest,
    search,
    clipboard,
    error,
    crumbPaths,
    isThisPc,
    remoteProfiles,
    activeRemote,
    isRemote,
    remoteHome,
    submitSource,
    submitDialogVisible,
    submitTargetId,
    submitTargets,
    submitTarget,
    submitTargetFromOpenFolder,
    submitTargetDir,
    submitting,
    submitResult,
    openSubmitJob,
    submitJob,
    windowLabel,
    windowMachineIds,
    detachedMachineIds,
    lastPaths,
    removeWindowMachine,
    attachMachine,
    addWindowMachine,
    activeQuickAccess,
    isQuickAccessAvailable,
    previewWidth,
    setPreviewWidth,
    openTerminal,
    openInExplorer,
    settingsVisible,
    settingsTab,
    sortedEntries,
    visibleEntries,
    selectedEntries,
    hasSelection,
    typeCounts,
    filteredTypeCounts,
    init,
    loadDrives,
    navigate,
    refresh,
    goBack,
    goForward,
    goUp,
    toThisPc,
    setActiveRemote,
    goLocal,
    refreshRemoteProfiles,
    openSettings,
    openEntry,
    openEntryByPath,
    revealAiPath,
    addToQuickAccess,
    removeFromQuickAccess,
    createFolder,
    createFile,
    deleteSelected,
    renameEntry,
    copySelected,
    paste,
    setSort,
    setFilterType,
    clearFilters,
    setPreview,
    toggleChemPanel,

    // 递归读取
    recursiveMode,
    recursiveRoot,
    scanning,
    scanId,
    scanProgress,
    scanStats,
    ensureChemForRows,
    relativePath,
    relativeDir,
    startRecursive,
    cancelRecursiveScan,
    stopRecursive,
    onScanProgress,

    // 右侧面板 / AI 对话
    rightPanelTab,
    aiMessages,
    aiSending,
    aiConfig,
    aiConfigLoading,
    openAiPanel,
    loadAiConfig,
    saveAiConfig,
    testAi,
    clearAiChat,
    sendAiMessage,
    onAiToolEvent,
  };
});
