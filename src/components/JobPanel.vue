<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api } from '../api/tauri';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();

const output = ref('');
const error = ref('');
const loading = ref(false);
const updatedAt = ref<number | null>(null);
let timer: number | undefined;
let requestSeq = 0;

const profile = computed(() => store.activeRemote);
const command = computed(() => profile.value?.jobQueryCommand?.trim() || 'squeue -u $USER');
const interval = computed(() => Math.max(5, profile.value?.jobQueryInterval || 10));

const updatedLabel = computed(() => {
  if (!updatedAt.value) return '';
  return new Date(updatedAt.value).toLocaleTimeString();
});

/** squeue 这类空白分隔的表格输出 → 表格；其它格式退回原样显示。 */
const table = computed(() => {
  const lines = output.value
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0);
  if (lines.length < 2) return null;
  const header = lines[0].trim().split(/\s+/);
  if (header.length < 3) return null;
  const rows = lines.slice(1).map((line) => line.trim().split(/\s+/));
  if (!rows.every((row) => row.length === header.length)) return null;
  return { header, rows };
});

async function refresh() {
  const p = profile.value;
  if (!p || !p.jobQueryEnabled) return;
  const seq = ++requestSeq;
  loading.value = true;
  error.value = '';
  try {
    const text = await api.remoteJobQuery(p, command.value);
    if (seq !== requestSeq) return;
    output.value = text;
    updatedAt.value = Date.now();
  } catch (e) {
    if (seq !== requestSeq) return;
    error.value = String(e);
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

function stopTimer() {
  if (timer !== undefined) {
    window.clearInterval(timer);
    timer = undefined;
  }
}

function startTimer() {
  stopTimer();
  timer = window.setInterval(() => void refresh(), interval.value * 1000);
}

watch(
  [() => store.activeRemote?.id, () => command.value, () => interval.value],
  () => {
    output.value = '';
    error.value = '';
    updatedAt.value = null;
    void refresh();
    startTimer();
  },
);

onMounted(() => {
  void refresh();
  startTimer();
});

onBeforeUnmount(stopTimer);
</script>

<template>
  <div class="wm-jobpanel">
    <div class="wm-jobpanel-toolbar">
      <span class="wm-jobpanel-cmd" :title="command">{{ command }}</span>
      <span v-if="loading" class="wm-jobpanel-hint">查询中…</span>
      <span v-else-if="updatedLabel" class="wm-jobpanel-hint">更新于 {{ updatedLabel }}</span>
      <span class="wm-jobpanel-spacer" />
      <span class="wm-jobpanel-hint">每 {{ interval }} 秒刷新</span>
      <button class="wm-jobpanel-btn" :disabled="loading" @click="refresh">刷新</button>
    </div>

    <div v-if="error" class="wm-jobpanel-error">{{ error }}</div>

    <div class="wm-jobpanel-body">
      <table v-if="table" class="wm-jobs-table">
        <thead>
          <tr>
            <th v-for="(h, i) in table.header" :key="i">{{ h }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, r) in table.rows" :key="r">
            <td v-for="(cell, c) in row" :key="c">{{ cell }}</td>
          </tr>
        </tbody>
      </table>
      <pre v-else-if="output" class="wm-jobs-raw">{{ output }}</pre>
      <el-empty
        v-else-if="!loading"
        description="暂无作业输出"
        :image-size="60"
      />
    </div>
  </div>
</template>

<style scoped>
.wm-jobpanel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.wm-jobpanel-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding: 6px 10px;
  border-bottom: 1px solid #eee;
  background: #f5f7fa;
  font-size: 12px;
  color: #666;
}

.wm-jobpanel-cmd {
  max-width: 45%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: Consolas, 'Cascadia Mono', monospace;
  color: #444;
}

.wm-jobpanel-hint {
  color: #999;
  white-space: nowrap;
}

.wm-jobpanel-spacer {
  flex: 1;
}

.wm-jobpanel-btn {
  border: 1px solid #dcdfe6;
  background: #fff;
  color: #333;
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
  cursor: pointer;
}

.wm-jobpanel-btn:hover:not(:disabled) {
  border-color: var(--accent, #0067c0);
  color: var(--accent, #0067c0);
}

.wm-jobpanel-btn:disabled {
  opacity: 0.55;
  cursor: default;
}

.wm-jobpanel-error {
  flex-shrink: 0;
  padding: 6px 10px;
  border-bottom: 1px solid #f3d3d3;
  background: #fdf2f2;
  color: #c0392b;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-word;
}

.wm-jobpanel-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px 0 8px;
}

.wm-jobs-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-family: Consolas, 'Cascadia Mono', monospace;
  white-space: nowrap;
}

.wm-jobs-table th,
.wm-jobs-table td {
  text-align: left;
  padding: 4px 8px;
  border-bottom: 1px solid #eee;
}

.wm-jobs-table th {
  position: sticky;
  top: 0;
  background: #f7f8fa;
  color: #666;
  font-weight: 600;
}

.wm-jobs-table tbody tr:hover {
  background: #f5f7fa;
}

.wm-jobs-raw {
  margin: 0;
  padding: 8px 10px;
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 12px;
  line-height: 1.5;
  color: #333;
  white-space: pre;
  overflow: auto;
}
</style>
