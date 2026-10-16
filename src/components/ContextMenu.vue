<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue';
import { ElMessageBox } from 'element-plus';
import { useContextMenu } from '../composables/useContextMenu';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();
const { state, close } = useContextMenu();

const target = () => state.target;
const isDir = () => target()?.isDir ?? false;

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

onMounted(() => window.addEventListener('click', onGlobalClick));
onBeforeUnmount(() => window.removeEventListener('click', onGlobalClick));
</script>

<template>
  <div
    v-if="state.visible"
    class="wm-menu"
    :style="{ left: state.x + 'px', top: state.y + 'px' }"
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
      <div class="menu-sep" />
      <div class="menu-item" @click="store.refresh(); close()">
        <el-icon><Refresh /></el-icon>
        <span>刷新</span>
      </div>
    </template>
  </div>
</template>
