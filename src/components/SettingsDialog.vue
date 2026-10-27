<script setup lang="ts">
import { ref } from 'vue';
import { useExplorer } from '../stores/explorer';
import { CHEM_BY_KIND, CHEM_DEFS, FILTER_KINDS, KIND_LABELS } from '../utils/chemColumns';
import { chemColor } from '../utils/format';

const store = useExplorer();
const visible = ref(false);
</script>

<template>
  <button class="cmd-btn" @click="visible = true">
    <el-icon><Setting /></el-icon>
    <span>设置</span>
  </button>

  <el-dialog v-model="visible" title="设置 · 每种文件类型显示的列" width="80%" top="8vh">
    <div class="wm-settings">
      <template v-for="k in FILTER_KINDS" :key="k">
        <div class="wm-settings-group">
          <div class="wm-settings-title">
            <span class="wm-filter-dot" :style="{ background: chemColor(k) }" />
            <span>{{ KIND_LABELS[k] }}</span>
          </div>
          <div class="wm-settings-cols">
            <el-checkbox
              v-for="col in CHEM_BY_KIND[k]"
              :key="col"
              :model-value="store.isColumnEnabled(k, col)"
              @change="(v: unknown) => store.setColumnEnabled(k, col, v === true)"
            >
              {{ CHEM_DEFS[col]?.label ?? col }}
            </el-checkbox>
          </div>
        </div>
      </template>
    </div>

    <template #footer>
      <el-button @click="store.resetColumnSettings()">恢复默认</el-button>
      <el-button type="primary" @click="visible = false">确定</el-button>
    </template>
  </el-dialog>
</template>
