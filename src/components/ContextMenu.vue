<script setup lang="ts">
import { nextTick, onMounted, onBeforeUnmount, ref, watch } from 'vue';
import { ElMessageBox } from 'element-plus';
import { useContextMenu } from '../composables/useContextMenu';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();
const { state, close } = useContextMenu();

const menuRef = ref<HTMLElement | null>(null);
const pos = ref({ left: 0, top: 0 });
const posReady = ref(false);

/** 菜单先按点击位置渲染，再按实际尺寸收进窗口，保证上下左右都不越界。 */
watch(
  () => [state.visible, state.x, state.y],
  async () => {
    if (!state.visible) {
      posReady.value = false;
      return;
    }
    posReady.value = false;
    pos.value = { left: state.x, top: state.y };
    await nextTick();
    const el = menuRef.value;
    if (!el) {
      posReady.value = true;
      return;
    }
    const rect = el.getBoundingClientRect();
    const pad = 6;
    const maxLeft = Math.max(pad, window.innerWidth - rect.width - pad);
    const maxTop = Math.max(pad, window.innerHeight - rect.height - pad);
    pos.value = {
      left: Math.min(Math.max(pad, state.x), maxLeft),
      top: Math.min(Math.max(pad, state.y), maxTop),
    };
    posReady.value = true;
  },
  { immediate: true, flush: 'post' },
);

const target = () => state.target;
const isDir = () => target()?.isDir ?? false;

/** 路径比较用的归一化 key：忽略末尾分隔符；本机 Windows 路径忽略大小写，远程 POSIX 路径区分大小写 */
function pathKey(p: string): string {
  const norm = p.replace(/[\\/]+$/, '');
  return store.isRemote ? norm : norm.toLowerCase();
}

/** 当前右键目标是否已经在快速访问区域中（内置项或用户固定项，本机/远程通用） */
function inQuickAccess(): boolean {
  const t = target();
  if (!t) return false;
  return store.activeQuickAccess.some((q) => pathKey(q.path) === pathKey(t.path));
}

/** 把右键的文件夹添加到快速访问（持久化，重启后仍在） */
async function addToQuickAccess() {
  const t = target();
  const already = inQuickAccess();
  close();
  if (!t || !t.isDir) return;
  if (already) return;
  try {
    await store.addToQuickAccess(t.path);
  } catch (e) {
    store.error = String(e);
  }
}

function onGlobalClick(e: MouseEvent) {
  // 点击菜单之外关闭
  const el = (e.target as HTMLElement).closest('.wm-menu');
  if (!el) close();
}

function stop(e: Event) {
  e.preventDefault();
  e.stopPropagation();
}

async function open() {
  if (target()) await store.openEntry(target()!);
  close();
}

async function rename() {
  const t = target();
  if (!t) return;
  try {
    const { value } = await ElMessageBox.prompt('请输入新名称', '重命名', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      inputValue: t.name,
    });
    if (value) await store.renameEntry(t, value);
  } catch {
    /* 取消 */
  }
  close();
}

function copy(action: 'copy' | 'cut') {
  if (target()) store.copySelected(action);
  close();
}

async function paste() {
  await store.paste();
  close();
}

async function remove() {
  const t = target();
  if (!t) return;
  try {
    await ElMessageBox.confirm(`确定要删除「${t.name}」吗？`, '删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    });
    await store.deleteSelected(false);
  } catch {
    /* 取消 */
  }
  close();
}

async function newFolder() {
  try {
    const { value } = await ElMessageBox.prompt('请输入文件夹名称', '新建文件夹', {
      confirmButtonText: '创建',
      cancelButtonText: '取消',
      inputValue: '新建文件夹',
    });
    if (value) await store.createFolder(value);
  } catch {
    /* 取消 */
  }
  close();
}

async function newFile() {
  try {
    const { value } = await ElMessageBox.prompt('请输入文件名（含扩展名）', '新建文件', {
      confirmButtonText: '创建',
      cancelButtonText: '取消',
      inputValue: '新建化学文件.xyz',
    });
    if (value) await store.createFile(value);
  } catch {
    /* 取消 */
  }
  close();
}

function showChemPanel() {
  store.toggleChemPanel();
  close();
}

/** 提交作业：选择一台配置了提交命令的机器。 */
function submitJob() {
  const t = target();
  close();
  if (!t || t.isDir) return;
  if (store.submitTargets.length === 0) {
    store.error = '还没有机器配置提交作业命令，请先在 设置 → 远程机器 中配置';
    return;
  }
  store.openSubmitJob(t.path);
}

/** 在当前文件夹（或右键的文件夹）打开系统终端。 */
function openTerminal() {
  const t = target();
  void store.openTerminal(t?.isDir ? t.path : undefined);
  close();
}

/** 在系统文件资源管理器中显示：右键文件夹时打开该文件夹，空白处打开当前文件夹。 */
function openInExplorer() {
  const t = target();
  const path = t?.isDir ? t.path : undefined;
  close();
  void store.openInExplorer(path);
}

onMounted(() => window.addEventListener('click', onGlobalClick));
onBeforeUnmount(() => window.removeEventListener('click', onGlobalClick));
</script>

<template>
  <div
    v-if="state.visible"
    ref="menuRef"
    class="wm-menu"
    :style="{
      left: pos.left + 'px',
      top: pos.top + 'px',
      visibility: posReady ? 'visible' : 'hidden',
    }"
    @click="stop"
    @contextmenu="stop"
    @mousedown.stop
  >
    <template v-if="state.target">
      <div class="menu-item" @click="open">
        <el-icon><FolderOpened /></el-icon>
        <span>{{ isDir() ? '打开' : '打开' }}</span>
      </div>
      <div v-if="state.target.isChem" class="menu-item" @click="showChemPanel">
        <el-icon><DataLine /></el-icon>
        <span>查看化学详情</span>
      </div>
      <div v-if="!isDir()" class="menu-item" @click="submitJob">
        <el-icon><Upload /></el-icon>
        <span>提交作业</span>
      </div>
      <div
        v-if="isDir()"
        class="menu-item"
        :class="{ 'is-disabled': inQuickAccess() }"
        @click="addToQuickAccess"
      >
        <el-icon><Star /></el-icon>
        <span>{{ inQuickAccess() ? '已在快速访问中' : '添加到快速访问' }}</span>
      </div>
      <div v-if="isDir()" class="menu-item" @click="openTerminal">
        <el-icon><Platform /></el-icon>
        <span>在终端中打开</span>
      </div>
      <div v-if="isDir() && !store.isRemote" class="menu-item" @click="openInExplorer">
        <el-icon><Monitor /></el-icon>
        <span>在文件资源管理器中显示</span>
      </div>
      <div class="menu-sep" />
      <div class="menu-item" @click="copy('copy')">
        <el-icon><CopyDocument /></el-icon>
        <span>复制</span>
      </div>
      <div class="menu-item" @click="copy('cut')">
        <el-icon><Scissor /></el-icon>
        <span>剪切</span>
      </div>
      <div class="menu-item" @click="rename">
        <el-icon><EditPen /></el-icon>
        <span>重命名</span>
      </div>
      <div class="menu-sep" />
      <div class="menu-item is-danger" @click="remove">
        <el-icon><Delete /></el-icon>
        <span>删除</span>
      </div>
    </template>

    <template v-else>
      <div class="menu-item" @click="newFolder">
        <el-icon><FolderAdd /></el-icon>
        <span>新建文件夹</span>
      </div>
      <div class="menu-item" @click="newFile">
        <el-icon><DocumentAdd /></el-icon>
        <span>新建文件</span>
      </div>
      <div v-if="!store.isRemote && !store.isThisPc" class="menu-item" @click="openInExplorer">
        <el-icon><Monitor /></el-icon>
        <span>在文件资源管理器中显示</span>
      </div>
      <div class="menu-item" @click="openTerminal">
        <el-icon><Platform /></el-icon>
        <span>在终端中打开</span>
      </div>
      <div class="menu-sep" />
      <div v-if="store.clipboard" class="menu-item" @click="paste">
        <el-icon><DocumentCopy /></el-icon>
        <span>粘贴</span>
      </div>
      <div v-if="store.clipboard" class="menu-sep" />
      <div class="menu-item" @click="store.refresh(); close()">
        <el-icon><Refresh /></el-icon>
        <span>刷新</span>
      </div>
    </template>
  </div>
</template>
