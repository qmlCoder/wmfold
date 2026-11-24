<script setup lang="ts">
import { useExplorer } from '../stores/explorer';
import type { QuickAccess } from '../types';

const store = useExplorer();

function isActive(q: QuickAccess): boolean {
  if (store.isRemote) return store.currentPath === q.path;
  return q.path === '::ThisPC' ? store.isThisPc : store.currentPath === q.path;
}

async function unpin(q: QuickAccess) {
  try {
    await store.removeFromQuickAccess(q.path);
  } catch (e) {
    store.error = String(e);
  }
}

function unavailable(q: QuickAccess): boolean {
  return !store.isQuickAccessAvailable(q.path);
}

function open(q: QuickAccess) {
  if (unavailable(q)) {
    store.error = `目录不存在或不可用：${q.path}`;
    return;
  }
  store.navigate(q.path);
}

function itemTitle(q: QuickAccess): string {
  return unavailable(q) ? `${q.path}（目录不存在，悬停 × 可移除）` : q.path;
}
</script>

<template>
  <div class="wm-bookmarks">
    <span class="wm-bookmarks-label">快速访问</span>

    <div
      v-for="q in store.activeQuickAccess"
      :key="q.path"
      class="wm-bookmark"
      :class="{ 'is-active': isActive(q), 'is-unavailable': unavailable(q) }"
      :title="itemTitle(q)"
      @click="open(q)"
    >
      <el-icon>
        <Monitor v-if="q.path === '::ThisPC'" />
        <HomeFilled v-else-if="q.icon === 'home'" />
        <Folder v-else />
      </el-icon>
      <span class="wm-bookmark-name">{{ q.name }}</span>
      <span v-if="unavailable(q)" class="wm-bookmark-badge">不可用</span>
      <button
        v-if="q.pinned"
        type="button"
        class="wm-bookmark-remove"
        title="从快速访问中移除"
        @click.stop="unpin(q)"
      >
        ×
      </button>
    </div>

    <div v-if="store.activeQuickAccess.length === 0" class="wm-bookmarks-empty">暂无快速访问</div>

    <div class="wm-bookmarks-spacer" />
    <span class="wm-bookmarks-hint">
      {{ store.isRemote ? '远程右键文件夹可添加到快速访问' : '右键文件夹可添加到快速访问' }}
    </span>
  </div>
</template>

<style scoped>
.wm-bookmarks {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 30px;
  flex-shrink: 0;
  padding: 0 8px;
  background: #fafafa;
  border-bottom: 1px solid var(--border, #e0e0e0);
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
  user-select: none;
}

.wm-bookmarks::-webkit-scrollbar {
  display: none;
}

.wm-bookmarks-label {
  margin-right: 6px;
  color: #999;
  font-size: 11px;
  white-space: nowrap;
}

.wm-bookmark {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 220px;
  height: 24px;
  padding: 0 8px;
  border-radius: 5px;
  color: #444;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
}

.wm-bookmark:hover {
  background: #efefef;
}

.wm-bookmark.is-active {
  background: var(--hover-accent, rgba(0, 103, 192, 0.08));
  color: var(--accent, #0067c0);
}

.wm-bookmark.is-unavailable {
  color: #b0b0b0;
  text-decoration: line-through;
}

.wm-bookmark.is-unavailable .el-icon {
  color: #c8c8c8;
}

.wm-bookmark-badge {
  padding: 0 4px;
  border-radius: 3px;
  background: #fdecea;
  color: #d93025;
  font-size: 10px;
  text-decoration: none;
}

.wm-bookmark .el-icon {
  font-size: 13px;
  flex-shrink: 0;
}

.wm-bookmark-name {
  overflow: hidden;
  text-overflow: ellipsis;
}

.wm-bookmark-remove {
  display: none;
  border: none;
  background: transparent;
  color: #999;
  padding: 0 1px;
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
}

.wm-bookmark:hover .wm-bookmark-remove {
  display: inline-block;
}

.wm-bookmark-remove:hover {
  color: #e81123;
}

.wm-bookmarks-empty {
  color: #aaa;
  font-size: 12px;
  padding: 0 6px;
}

.wm-bookmarks-spacer {
  flex: 1;
}

.wm-bookmarks-hint {
  margin-left: 8px;
  color: #bbb;
  font-size: 11px;
  white-space: nowrap;
  flex-shrink: 0;
}
</style>
