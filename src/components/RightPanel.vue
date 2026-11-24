<script setup lang="ts">
import { computed, watch } from 'vue';
import { useExplorer } from '../stores/explorer';
import type { RightPanelTab } from '../types';
import AiChatPanel from './AiChatPanel.vue';
import ChemPanel from './ChemPanel.vue';
import JobPanel from './JobPanel.vue';

const store = useExplorer();

const tab = computed<RightPanelTab>({
  get: () => store.rightPanelTab,
  set: (value) => {
    store.rightPanelTab = value;
  },
});

/** 右侧是否提供作业查询 tab（仅远程机器且启用了查询）。 */
const jobsAvailable = computed(
  () =>
    store.isRemote &&
    !!store.activeRemote?.jobQueryEnabled &&
    !!store.activeRemote?.jobQueryCommand?.trim(),
);

watch(jobsAvailable, (available) => {
  if (!available && tab.value === 'jobs') tab.value = 'preview';
});

// 切到 AI 对话时给聊天区留出足够宽度
watch(tab, (value) => {
  if (value === 'ai' && store.previewWidth < 380) store.setPreviewWidth(420);
});

watch(
  () => store.activeRemote?.id,
  () => {
    if (!jobsAvailable.value && tab.value === 'jobs') tab.value = 'preview';
  },
);
</script>

<template>
  <div class="wm-rightpanel">
    <div class="wm-rightpanel-tabs">
      <button
        class="wm-rightpanel-tab"
        :class="{ 'is-active': tab === 'preview' }"
        @click="tab = 'preview'"
      >
        预览
      </button>
      <button
        class="wm-rightpanel-tab"
        :class="{ 'is-active': tab === 'ai' }"
        title="AI 对话（DeepSeek）"
        @click="tab = 'ai'"
      >
        AI 对话
      </button>
      <button
        v-if="jobsAvailable"
        class="wm-rightpanel-tab"
        :class="{ 'is-active': tab === 'jobs' }"
        @click="tab = 'jobs'"
      >
        作业查询
      </button>
      <span class="wm-rightpanel-spacer" />
      <button class="wm-rightpanel-close" title="关闭右侧面板" @click="store.toggleChemPanel()">
        关闭
      </button>
    </div>

    <div class="wm-rightpanel-body">
      <ChemPanel v-if="tab === 'preview'" />
      <AiChatPanel v-else-if="tab === 'ai'" />
      <JobPanel v-else />
    </div>
  </div>
</template>
