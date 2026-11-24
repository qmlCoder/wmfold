<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { api } from '../api/tauri';
import { useExplorer } from '../stores/explorer';

interface MachineTab {
  id: string;
  label: string;
  title: string;
  local: boolean;
}

const store = useExplorer();
const appWindow = getCurrentWindow();

const tabs = computed<MachineTab[]>(() => {
  const list: MachineTab[] = [];
  for (const id of store.windowMachineIds) {
    if (id === 'local') {
      list.push({ id, label: '本机', title: '本机', local: true });
      continue;
    }
    const profile = store.remoteProfiles.find((p) => p.id === id);
    if (profile) {
      list.push({
        id,
        label: profile.name || profile.host,
        title: `${profile.user}@${profile.host}:${profile.port}`,
        local: false,
      });
    }
  }
  return list;
});

/** 还没在当前窗口打开的已保存机器（+ 号菜单里列出）。 */
const availableProfiles = computed(() =>
  store.remoteProfiles.filter((p) => !store.windowMachineIds.includes(p.id)),
);

function activate(tab: MachineTab) {
  if (tab.local) {
    void store.setActiveRemote(null);
    return;
  }
  const profile = store.remoteProfiles.find((p) => p.id === tab.id);
  if (profile) void store.setActiveRemote(profile);
}

let dragId: string | null = null;
let dragStartY = 0;

const tabMenu = ref<{ x: number; y: number; tab: MachineTab } | null>(null);
const addMenuOpen = ref(false);
const addMenuPos = ref({ left: 0, top: 0 });

/** 与窗口 label 一一对应的唯一标识。 */
function windowLabelFor(machine: string): string {
  let hash = 2166136261;
  for (const byte of new TextEncoder().encode(machine)) {
    hash ^= byte;
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  const safe = machine.replace(/[^A-Za-z0-9_-]/g, '_').slice(0, 40);
  return `machine-${safe}-${hash.toString(16).padStart(8, '0')}`;
}

/** 用 Tauri 的 WebviewWindow API 创建独立窗口（避免 Rust 侧创建时停在 about:blank）。 */
async function openDetached(machine: string, path: string) {
  const label = windowLabelFor(machine);
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }
  localStorage.setItem(`wmfold.window.${label}`, JSON.stringify({ machine, path }));
  const win = new WebviewWindow(label, {
    url: 'index.html',
    title: 'WMFold',
    width: 1200,
    height: 800,
    minWidth: 900,
    minHeight: 560,
    decorations: false,
    resizable: true,
    center: true,
  });
  win.once('tauri://error', (event) => {
    store.error = `创建窗口失败：${String(event.payload ?? '')}`;
  });
}

function openTabMenu(event: MouseEvent, tab: MachineTab) {
  addMenuOpen.value = false;
  tabMenu.value = { x: event.clientX, y: event.clientY, tab };
}

function closeMenus() {
  tabMenu.value = null;
  addMenuOpen.value = false;
}

function toggleAddMenu(event: MouseEvent) {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  addMenuPos.value = { left: Math.round(rect.left), top: Math.round(rect.bottom + 5) };
  addMenuOpen.value = !addMenuOpen.value;
}

function addMachine(machineId: string) {
  closeMenus();
  void store.addWindowMachine(machineId);
}

function manageMachines() {
  closeMenus();
  store.openSettings('remote');
}

async function detachTab(tab: MachineTab) {
  closeMenus();
  try {
    await openDetached(tab.id, store.currentPath);
    store.removeWindowMachine(tab.id);
    if (store.windowMachineIds.length === 0) await appWindow.close();
  } catch (e) {
    store.error = String(e);
  }
}

async function mergeToMain(tab: MachineTab) {
  closeMenus();
  try {
    await api.mergeMachineWindow('main', tab.id, store.currentPath);
    store.removeWindowMachine(tab.id);
    if (store.windowMachineIds.length === 0) await appWindow.close();
  } catch (e) {
    store.error = String(e);
  }
}

onMounted(() => window.addEventListener('click', closeMenus));
onBeforeUnmount(() => window.removeEventListener('click', closeMenus));

function onDragStart(event: DragEvent, id: string) {
  dragId = id;
  dragStartY = event.clientY;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', id);
  }
}

async function onDragEnd(event: DragEvent) {
  const id = dragId;
  dragId = null;
  if (!id) return;
  // 只有明显向下拖离标题栏时才触发“拖出/合并”，避免误触
  if (event.clientY - dragStartY < 40) return;

  try {
    const scale = window.devicePixelRatio || 1;
    const position = await appWindow.outerPosition();
    const screenX = Math.round(position.x + event.clientX * scale);
    const screenY = Math.round(position.y + event.clientY * scale);
    const target = await api.windowAtPoint(screenX, screenY);

    if (target && target !== appWindow.label) {
      // 拖到另一个窗口上：合并到那个窗口
      await api.mergeMachineWindow(target, id, store.currentPath);
      store.removeWindowMachine(id);
    } else if (!target) {
      // 拖到窗口外空白桌面：拆成独立窗口
      if (store.windowMachineIds.length <= 1) return;
      await openDetached(id, store.currentPath);
      store.removeWindowMachine(id);
    }

    if (store.windowMachineIds.length === 0) {
      await appWindow.close();
    }
  } catch (e) {
    store.error = String(e);
  }
}
</script>

<template>
  <div class="wm-machine-tabs">
    <div
      v-for="tab in tabs"
      :key="tab.id"
      class="wm-machine-tab"
      :class="{
        'is-active': tab.local ? !store.isRemote : store.activeRemote?.id === tab.id,
      }"
      :title="`${tab.title}（拖动或右键可拆分为独立窗口）`"
      draggable="true"
      @click="activate(tab)"
      @dblclick="store.windowLabel === 'main' ? detachTab(tab) : mergeToMain(tab)"
      @contextmenu.prevent.stop="openTabMenu($event, tab)"
      @dragstart="onDragStart($event, tab.id)"
      @dragend="onDragEnd"
    >
      <el-icon>
        <Monitor v-if="tab.local" />
        <Connection v-else />
      </el-icon>
      <span>{{ tab.label }}</span>
    </div>

    <button
      v-if="store.windowLabel === 'main'"
      class="wm-machine-tab is-add"
      title="打开已保存的机器"
      @click.stop="toggleAddMenu"
    >
      <el-icon><Plus /></el-icon>
    </button>

    <div
      v-if="addMenuOpen"
      class="wm-machine-pop"
      :style="{ left: `${addMenuPos.left}px`, top: `${addMenuPos.top}px` }"
      @click.stop
    >
      <div class="wm-machine-pop-title">打开已保存的机器</div>
      <div
        v-for="p in availableProfiles"
        :key="p.id"
        class="wm-machine-pop-item"
        @click="addMachine(p.id)"
      >
        <el-icon><Connection /></el-icon>
        <span class="wm-machine-pop-name">{{ p.name || p.host }}</span>
        <small>{{ p.user }}@{{ p.host }}</small>
      </div>
      <div v-if="availableProfiles.length === 0" class="wm-machine-pop-empty">
        没有其他已保存的机器
      </div>
      <div class="wm-machine-pop-sep" />
      <div class="wm-machine-pop-item" @click="manageMachines">
        <el-icon><Setting /></el-icon>
        <span class="wm-machine-pop-name">管理 / 添加远程机器…</span>
      </div>
    </div>

    <div
      v-if="tabMenu"
      class="wm-tab-menu"
      :style="{ left: `${tabMenu.x}px`, top: `${tabMenu.y}px` }"
      @click.stop
    >
      <div class="wm-tab-menu-item" @click="detachTab(tabMenu.tab)">在新窗口打开</div>
      <div
        v-if="store.windowLabel !== 'main'"
        class="wm-tab-menu-item"
        @click="mergeToMain(tabMenu.tab)"
      >
        合并到主窗口
      </div>
    </div>
  </div>
</template>

<style scoped>
.wm-machine-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  height: var(--wm-titlebar-height, 32px);
  flex-shrink: 0;
  padding: 0 4px;
  background: transparent;
  border-bottom: none;
  overflow-x: auto;
  overflow-y: hidden;
  user-select: none;
  scrollbar-width: none;
}

.wm-machine-tabs::-webkit-scrollbar {
  display: none;
}

.wm-machine-tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 10px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: #555;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
}

.wm-machine-tab:hover {
  background: #f0f0f0;
}

.wm-machine-tab.is-active {
  background: #e8f0fe;
  border: none;
  color: var(--accent, #0067c0);
  font-weight: 600;
  box-shadow: none;
}

.wm-machine-tab .el-icon {
  font-size: 14px;
}

.wm-machine-tab.is-add {
  width: 24px;
  padding: 0;
  justify-content: center;
  color: #888;
}

.wm-machine-tab.is-add:hover {
  color: var(--accent, #0067c0);
}

.wm-machine-pop {
  position: fixed;
  z-index: 4000;
  min-width: 240px;
  padding: 5px;
  background: #ffffff;
  border: 1px solid #e4e7ed;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
}

.wm-machine-pop-title {
  padding: 5px 10px 4px;
  font-size: 11px;
  color: #999;
}

.wm-machine-pop-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 5px;
  font-size: 12px;
  color: #303133;
  cursor: pointer;
  white-space: nowrap;
}

.wm-machine-pop-item:hover {
  background: #f5f7fa;
}

.wm-machine-pop-name {
  overflow: hidden;
  text-overflow: ellipsis;
}

.wm-machine-pop-item small {
  margin-left: auto;
  color: #999;
  font-size: 11px;
}

.wm-machine-pop-sep {
  height: 1px;
  margin: 4px 6px;
  background: #eee;
}

.wm-machine-pop-empty {
  padding: 7px 10px;
  color: #999;
  font-size: 12px;
}

.wm-tab-menu {
  position: fixed;
  z-index: 4000;
  min-width: 170px;
  padding: 4px;
  background: #ffffff;
  border: 1px solid #e4e7ed;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
}

.wm-tab-menu-item {
  padding: 7px 10px;
  border-radius: 5px;
  font-size: 12px;
  color: #303133;
  cursor: pointer;
  white-space: nowrap;
}

.wm-tab-menu-item:hover {
  background: #f5f7fa;
}
</style>
