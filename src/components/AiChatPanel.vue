<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useExplorer } from '../stores/explorer';
import { renderMarkdown } from '../utils/markdown';
import type { AiToolResult, AiToolStep } from '../types';

const store = useExplorer();

const input = ref('');
const listRef = ref<HTMLElement | null>(null);
const expandedSteps = ref<Set<string>>(new Set());
const RESULT_PREVIEW = 6;

function resultItems(step: AiToolStep): AiToolResult[] {
  return expandedSteps.value.has(step.id)
    ? (step.results ?? [])
    : (step.results ?? []).slice(0, RESULT_PREVIEW);
}

function toggleStepResults(id: string) {
  const next = new Set(expandedSteps.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expandedSteps.value = next;
}

function openResult(item: AiToolResult) {
  void store.revealAiPath(item.path, item.isDir);
}

/** 把 AI 回答里的相对路径解析为绝对路径（以该轮对话的根目录为基准）。 */
function resolveMarkdownPath(raw: string, root: string): string {
  const value = raw.trim().replace(/[\\/]+$/, '');
  if (/^[A-Za-z]:[\\/]/.test(value) || value.startsWith('\\\\')) return value;
  const relative = value.replace(/^\.\//, '').replace(/^\.\\/, '');
  if (!root || root === '::ThisPC') return relative;
  return `${root.replace(/[\\/]+$/, '')}\\${relative.replace(/\//g, '\\')}`;
}

function openMarkdownPath(event: Event) {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const link = target.closest<HTMLElement>('[data-wm-path]');
  if (!link) return;
  event.preventDefault();
  const raw = link.dataset.wmPath;
  if (!raw) return;
  const msg = link.closest<HTMLElement>('.wm-ai-msg');
  const index = Number(msg?.dataset.aiIndex ?? -1);
  const root = store.aiMessages[index]?.rootPath || store.currentPath;
  void store.revealAiPath(
    resolveMarkdownPath(raw, root),
    link.dataset.wmDir === '1',
  );
}

function onContentKeydown(event: KeyboardEvent) {
  if (event.key !== 'Enter' && event.key !== ' ') return;
  openMarkdownPath(event);
}

const keyState = computed(() => {
  const cfg = store.aiConfig;
  if (!cfg || cfg.keySource === 'none') {
    return { label: '未配置 API Key', cls: 'is-missing' };
  }
  if (cfg.keySource === 'env') {
    return { label: `环境变量 DEEPSEEK_APIKEY（${cfg.envKeyPreview}）`, cls: 'is-ok' };
  }
  return { label: `已保存的 Key（${cfg.keyPreview}）`, cls: 'is-ok' };
});

const canSend = computed(() => !store.aiSending && input.value.trim().length > 0);

const suggestions = [
  '这个文件夹里有哪些文件？按类型汇总一下',
  '帮我找出所有 gjf 文件，并列出它们的作业关键词',
  '最新的 log 文件是否正常结束？能量是多少？',
  '搜索文件名包含 “opt” 的文件',
];

function send() {
  const text = input.value;
  if (!text.trim() || store.aiSending) return;
  input.value = '';
  void store.sendAiMessage(text);
}

function useSuggestion(text: string) {
  input.value = text;
  send();
}

function scrollToBottom() {
  nextTick(() => {
    const el = listRef.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

watch(
  () => {
    const last = store.aiMessages[store.aiMessages.length - 1];
    return [
      store.aiMessages.length,
      last?.content.length ?? 0,
      last?.steps?.length ?? 0,
      last?.steps?.map((step) => step.status).join('') ?? '',
      store.aiSending,
    ] as const;
  },
  scrollToBottom,
);

onMounted(() => {
  void store.loadAiConfig();
  scrollToBottom();
});
</script>

<template>
  <div class="wm-ai">
    <div class="wm-ai-head">
      <span class="wm-ai-key" :class="keyState.cls" :title="keyState.label">
        <el-icon>
          <Key v-if="store.aiConfig?.keySource === 'none'" />
          <CircleCheck v-else />
        </el-icon>
        {{ keyState.label }}
      </span>
      <span class="wm-spacer" />
      <button class="wm-ai-link" @click="store.openSettings('ai')">设置</button>
      <button
        v-if="store.aiMessages.length"
        class="wm-ai-link"
        title="清空对话"
        @click="store.clearAiChat()"
      >
        清空
      </button>
    </div>

    <div class="wm-ai-scope" :title="store.currentPath">
      <el-icon><FolderOpened /></el-icon>
      {{
        store.isRemote
          ? '作用范围：远程目录（暂不支持）'
          : store.recursiveMode
            ? `扫描范围（含子文件夹）：${store.currentPath}`
            : `作用范围（仅当前文件夹）：${store.currentPath}`
      }}
    </div>

    <div ref="listRef" class="wm-ai-list">
      <div v-if="!store.aiMessages.length" class="wm-ai-empty">
        <el-icon class="wm-ai-empty-icon"><ChatDotRound /></el-icon>
        <div class="wm-ai-empty-title">和 DeepSeek 一起读文件</div>
        <div class="wm-ai-empty-desc">
          描述你的需求，AI 会按需读取当前文件夹（含所有子文件夹）中的文件再回答。
        </div>
        <div class="wm-ai-suggestions">
          <button v-for="item in suggestions" :key="item" @click="useSuggestion(item)">
            {{ item }}
          </button>
        </div>
      </div>

      <div
        v-for="(message, index) in store.aiMessages"
        v-show="message.role === 'user' || message.content || message.error || message.steps?.length"
        :key="index"
        class="wm-ai-msg"
        :class="`is-${message.role}`"
        :data-ai-index="index"
      >
        <div class="wm-ai-role">{{ message.role === 'user' ? '我' : 'DeepSeek' }}</div>

        <div v-if="message.steps?.length" class="wm-ai-steps">
          <template v-for="step in message.steps" :key="step.id">
            <div class="wm-ai-step" :class="`is-${step.status}`">
              <el-icon class="wm-ai-step-icon">
                <Loading v-if="step.status === 'running'" class="is-spinning" />
                <CircleCheck v-else-if="step.status === 'ok'" />
                <WarningFilled v-else />
              </el-icon>
              <span class="wm-ai-step-label">{{ step.label }}</span>
              <span class="wm-ai-step-summary">{{ step.summary }}</span>
            </div>

            <div v-if="step.results?.length" class="wm-ai-results">
              <button
                v-for="item in resultItems(step)"
                :key="item.path"
                class="wm-ai-result"
                :title="item.path"
                @click="openResult(item)"
              >
                <el-icon>
                  <Folder v-if="item.isDir" />
                  <Document v-else />
                </el-icon>
                <span class="wm-ai-result-name">{{ item.rel || item.name }}</span>
              </button>
              <button
                v-if="step.results.length > RESULT_PREVIEW"
                class="wm-ai-result-more"
                @click="toggleStepResults(step.id)"
              >
                {{
                  expandedSteps.has(step.id)
                    ? '收起'
                    : `+${step.results.length - RESULT_PREVIEW}`
                }}
              </button>
            </div>
          </template>
        </div>

        <div
          v-if="message.content"
          class="wm-ai-content"
          v-html="renderMarkdown(message.content)"
          @click="openMarkdownPath"
          @keydown="onContentKeydown"
        />
        <div v-if="message.error" class="wm-ai-error">{{ message.error }}</div>
      </div>

      <div v-if="store.aiSending" class="wm-ai-thinking">
        <el-icon class="is-spinning"><Loading /></el-icon>
        正在思考并读取文件…
      </div>
    </div>

    <div class="wm-ai-input">
      <textarea
        v-model="input"
        rows="3"
        placeholder="询问当前文件夹中的文件… Enter 发送，Shift+Enter 换行"
        @keydown.enter.exact.prevent="send"
      />
      <div class="wm-ai-input-foot">
        <span class="wm-ai-hint">
          {{ store.aiConfig?.model || 'deepseek-flash' }}{{ store.aiConfig?.thinking ? ' · 思考' : '' }}
        </span>
        <button class="wm-ai-send" :disabled="!canSend" @click="send">发送</button>
      </div>
    </div>
  </div>
</template>
