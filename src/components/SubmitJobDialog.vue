<script setup lang="ts">
import { computed } from 'vue';
import { useExplorer } from '../stores/explorer';

const store = useExplorer();

const sourceLabel = computed(() => {
  const src = store.submitSource;
  if (!src) return '';
  return src.profile ? `${src.profile.name || src.profile.host} 上的文件` : '本机文件';
});

const targetDirHint = computed(() => {
  const target = store.submitTarget;
  if (!target) return '';
  if (store.submitTargetFromOpenFolder) {
    return `目标机器已打开的文件夹：${store.submitTargetDir}`;
  }
  return `目标机器默认提交路径：${store.submitTargetDir}`;
});
</script>

<template>
  <el-dialog
    v-model="store.submitDialogVisible"
    title="提交作业"
    width="620px"
    append-to-body
    destroy-on-close
  >
    <div v-if="store.submitSource" class="wm-submit-source">
      <div class="wm-submit-file">{{ store.submitSource.name }}</div>
      <div class="wm-submit-meta">{{ sourceLabel }}</div>
      <div class="wm-submit-path" :title="store.submitSource.path">{{ store.submitSource.path }}</div>
    </div>

    <div class="wm-submit-form">
      <div class="wm-submit-label">提交到哪台机器</div>
      <el-select
        v-model="store.submitTargetId"
        size="small"
        placeholder="选择已配置提交命令的机器"
        style="width: 100%"
      >
        <el-option
          v-for="p in store.submitTargets"
          :key="p.id"
          :label="`${p.name || p.host} (${p.user}@${p.host})`"
          :value="p.id"
        />
      </el-select>

      <template v-if="store.submitTarget">
        <div class="wm-submit-label" style="margin-top: 12px">提交目录</div>
        <div class="wm-submit-dir">{{ store.submitTargetDir }}</div>
        <div class="wm-submit-hint">{{ targetDirHint }}</div>

        <div class="wm-submit-label" style="margin-top: 12px">提交命令</div>
        <pre class="wm-submit-command">{{ store.submitTarget.submitCommand }}</pre>
        <div class="wm-submit-hint">
          执行前会把 <code>$PATH</code> 替换为远端文件路径；命令在远端登录 shell 中执行。
          删除 chk 路径设置：{{ store.submitTarget.clearChkPath ? '已开启' : '未开启' }}
        </div>
      </template>
      <el-empty v-else description="没有配置提交作业命令的机器" :image-size="64" />
    </div>

    <div v-if="store.submitResult" class="wm-submit-result">
      <el-alert
        :type="store.submitResult.ok ? 'success' : 'error'"
        :closable="false"
        show-icon
        :title="
          store.submitResult.ok
            ? '提交命令已执行'
            : `提交命令执行失败${store.submitResult.exitCode !== null ? `（退出码 ${store.submitResult.exitCode}）` : ''}`
        "
      />
      <div class="wm-submit-result-meta">
        远端文件：<code>{{ store.submitResult.remotePath }}</code>
      </div>
      <div class="wm-submit-result-meta">
        实际命令：<code>{{ store.submitResult.command }}</code>
      </div>
      <pre v-if="store.submitResult.stdout" class="wm-submit-output">{{ store.submitResult.stdout }}</pre>
      <pre v-if="store.submitResult.stderr" class="wm-submit-output is-error">{{ store.submitResult.stderr }}</pre>
    </div>

    <template #footer>
      <el-button size="small" @click="store.submitDialogVisible = false">关闭</el-button>
      <el-button
        size="small"
        type="primary"
        :loading="store.submitting"
        :disabled="!store.submitTarget"
        @click="store.submitJob()"
      >
        提交
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.wm-submit-source {
  padding: 10px 12px;
  border: 1px solid #e4e7ed;
  border-radius: 6px;
  background: #fafafa;
}
.wm-submit-file {
  font-weight: 600;
  color: #303133;
}
.wm-submit-meta {
  margin-top: 2px;
  font-size: 12px;
  color: #909399;
}
.wm-submit-path {
  margin-top: 4px;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  color: #606266;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wm-submit-form {
  margin-top: 14px;
}
.wm-submit-label {
  margin-bottom: 5px;
  font-size: 12px;
  font-weight: 600;
  color: #303133;
}
.wm-submit-dir,
.wm-submit-command {
  padding: 6px 8px;
  margin: 0;
  border-radius: 4px;
  background: #f5f7fa;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  color: #303133;
  white-space: pre-wrap;
  word-break: break-all;
}
.wm-submit-hint {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.5;
  color: #909399;
}
.wm-submit-result {
  margin-top: 16px;
}
.wm-submit-result-meta {
  margin-top: 6px;
  font-size: 12px;
  color: #606266;
  word-break: break-all;
}
.wm-submit-output {
  max-height: 180px;
  overflow: auto;
  padding: 8px 10px;
  margin: 8px 0 0;
  border-radius: 4px;
  background: #f5f7fa;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-all;
}
.wm-submit-output.is-error {
  color: #c45656;
  background: #fef0f0;
}
</style>
