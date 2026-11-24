<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { ElMessageBox } from 'element-plus';
import { api } from '../api/tauri';
import { useExplorer } from '../stores/explorer';
import type { RemoteProfile } from '../types';
import { CHEM_BY_KIND, CHEM_DEFS, FILTER_KINDS, KIND_LABELS } from '../utils/chemColumns';
import { chemColor } from '../utils/format';
import { useUpdater } from '../composables/useUpdater';

const props = withDefaults(defineProps<{ showButton?: boolean }>(), { showButton: true });

const store = useExplorer();

// ---- 软件更新（与标题栏共享同一份状态） ----
const {
  appVersion,
  checking: updateChecking,
  installing: updateInstalling,
  availableUpdate,
  message: updateMessage,
  progress: updateProgress,
  checkForUpdate,
  installUpdate,
} = useUpdater();

function checkUpdate() {
  void checkForUpdate();
}

const visible = computed({
  get: () => store.settingsVisible,
  set: (v: boolean) => {
    store.settingsVisible = v;
  },
});
const tab = computed({
  get: () => store.settingsTab,
  set: (v: 'display' | 'program' | 'ai' | 'remote') => {
    store.settingsTab = v;
  },
});

function emptyMachine(): RemoteProfile {
  return {
    id: '',
    name: '',
    host: '',
    port: 22,
    user: '',
    identityFile: '',
    password: '',
    hostKey: null,
    jobQueryEnabled: false,
    jobQueryCommand: 'squeue -u $USER',
    jobQueryInterval: 10,
    submitCommand: '',
    submitDir: '~/wmfold',
    clearChkPath: false,
  };
}

const machine = ref<RemoteProfile>(emptyMachine());

// ---- AI 对话（DeepSeek） ----
const aiKey = ref('');
const aiModel = ref('deepseek-flash');
const aiBaseUrl = ref('https://api.deepseek.com');
const aiThinking = ref(false);
const aiReasoningEffort = ref('high');
const aiSaving = ref(false);
const aiTesting = ref(false);
const aiTestMessage = ref('');
const aiTestOk = ref(false);

const aiKeySourceText = computed(() => {
  const cfg = store.aiConfig;
  if (!cfg) return '正在读取配置…';
  if (cfg.keySource === 'saved') return `当前使用：已保存的密钥（${cfg.keyPreview}）`;
  if (cfg.keySource === 'env') {
    return `当前使用：环境变量 DEEPSEEK_APIKEY（${cfg.envKeyPreview}）`;
  }
  return '当前未配置 API Key。';
});

const aiKeyPlaceholder = computed(() =>
  store.aiConfig?.hasEnvKey
    ? `留空则使用环境变量（${store.aiConfig.envKeyPreview}）`
    : 'sk-...',
);

async function syncAiFields() {
  await store.loadAiConfig(true);
  const cfg = store.aiConfig;
  aiKey.value = cfg?.apiKey ?? '';
  aiModel.value = cfg?.model ?? 'deepseek-flash';
  aiBaseUrl.value = cfg?.baseUrl ?? 'https://api.deepseek.com';
  aiThinking.value = cfg?.thinking ?? false;
  aiReasoningEffort.value = cfg?.reasoningEffort ?? 'high';
  aiTestMessage.value = '';
}

async function saveAiSettings() {
  aiSaving.value = true;
  aiTestMessage.value = '';
  try {
    const saved = await store.saveAiConfig(
      aiKey.value,
      aiModel.value,
      aiBaseUrl.value,
      aiThinking.value,
      aiReasoningEffort.value,
    );
    aiKey.value = saved.apiKey;
    aiModel.value = saved.model;
    aiBaseUrl.value = saved.baseUrl;
    aiThinking.value = saved.thinking;
    aiReasoningEffort.value = saved.reasoningEffort;
    aiTestOk.value = true;
    aiTestMessage.value = saved.keySource === 'none' ? '已保存（仍未配置 Key）' : '已保存';
  } catch (e) {
    aiTestOk.value = false;
    aiTestMessage.value = String(e);
  } finally {
    aiSaving.value = false;
  }
}

async function testAiConnection() {
  aiTesting.value = true;
  aiTestMessage.value = '';
  try {
    await store.saveAiConfig(
      aiKey.value,
      aiModel.value,
      aiBaseUrl.value,
      aiThinking.value,
      aiReasoningEffort.value,
    );
    const message = await store.testAi();
    aiTestOk.value = true;
    aiTestMessage.value = message;
  } catch (e) {
    aiTestOk.value = false;
    aiTestMessage.value = String(e);
  } finally {
    aiTesting.value = false;
  }
}

function selectMachine(p: RemoteProfile) {
  machine.value = {
    ...p,
    identityFile: p.identityFile ?? '',
    submitCommand: p.submitCommand ?? '',
    submitDir: p.submitDir ?? '~/wmfold',
    clearChkPath: p.clearChkPath ?? false,
  };
}

function newMachine() {
  machine.value = emptyMachine();
}

function normalized(): RemoteProfile {
  return {
    ...machine.value,
    port: Number(machine.value.port) || 22,
    identityFile: (machine.value.identityFile ?? '').trim() || null,
    password: machine.value.password ? machine.value.password : null,
    submitCommand: (machine.value.submitCommand ?? '').trim(),
    submitDir: (machine.value.submitDir ?? '').trim() || '~/wmfold',
    clearChkPath: !!machine.value.clearChkPath,
  };
}

async function saveMachine() {
  if (!machine.value.host.trim() || !machine.value.user.trim()) {
    store.error = '主机和用户名不能为空';
    return;
  }
  try {
    await api.remoteSaveProfile(normalized());
    await store.refreshRemoteProfiles();
    const saved = store.remoteProfiles.find(
      (p) => p.host === machine.value.host.trim() && p.user === machine.value.user.trim(),
    );
    if (saved) selectMachine(saved);
  } catch (e) {
    store.error = String(e);
  }
}

async function removeMachine() {
  if (!machine.value.id) return;
  try {
    await ElMessageBox.confirm(
      `确定删除「${machine.value.name || machine.value.host}」吗？`,
      '删除远程机器',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' },
    );
    const id = machine.value.id;
    await api.remoteDeleteProfile(id);
    await store.refreshRemoteProfiles();
    if (store.activeRemote?.id === id) await store.setActiveRemote(null);
    newMachine();
  } catch {
    /* 取消 */
  }
}

// 打开设置时同步一次机器列表；默认选中第一台（没有则清空表单）
watch(
  () => store.settingsVisible,
  async (open) => {
    if (!open) return;
    await store.refreshRemoteProfiles();
    if (store.remoteProfiles.length > 0 && !machine.value.id) {
      selectMachine(store.remoteProfiles[0]);
    }
  },
);

// 切到 AI 页时读取最新配置（Key / 模型 / Base URL）
watch(
  () => [store.settingsVisible, store.settingsTab] as const,
  ([open, tab]) => {
    if (open && tab === 'ai') void syncAiFields();
  },
);
</script>

<template>
  <button v-if="props.showButton" class="cmd-btn" title="设置" @click="store.openSettings('display')">
    <el-icon><Setting /></el-icon>
    <span>设置</span>
  </button>

  <el-dialog
    v-model="visible"
    title="设置"
    width="82%"
    top="6vh"
    append-to-body
    destroy-on-close
  >
    <el-tabs v-model="tab">
      <el-tab-pane label="展示信息" name="display">
        <div class="wm-settings">
          <p class="wm-settings-hint">每种文件类型在表格中展示哪些化学信息列：</p>
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
      </el-tab-pane>

      <el-tab-pane label="程序设置" name="program">
        <div class="wm-program-settings">
          <div class="wm-program-row">
            <div>
              <div class="wm-program-title">显示隐藏文件</div>
              <div class="wm-program-desc">
                显示 Windows 隐藏属性的文件；Linux 服务器 / 远程机器上为以「.」开头的文件。
                关闭后列表和类型计数都会隐藏这些文件。
              </div>
            </div>
            <el-switch
              :model-value="store.showHidden"
              @change="(v: unknown) => store.setShowHidden(v === true)"
            />
          </div>

          <div class="wm-program-row">
            <div>
              <div class="wm-program-title">外部终端工具</div>
              <div class="wm-program-desc">
                命令栏终端按钮 / 右键「在终端中打开」/ Ctrl+` 使用的终端程序。
                「自动」会依次尝试 Windows Terminal、PowerShell、命令提示符。
              </div>
            </div>
            <el-select
              :model-value="store.terminalTool"
              style="width: 220px"
              @change="(v: string) => store.setTerminalTool(v)"
            >
              <el-option label="自动（推荐）" value="auto" />
              <el-option label="Windows Terminal (wt.exe)" value="wt" />
              <el-option label="Windows PowerShell" value="powershell" />
              <el-option label="命令提示符 (cmd.exe)" value="cmd" />
            </el-select>
          </div>

          <div class="wm-program-row">
            <div>
              <div class="wm-program-title">软件更新</div>
              <div class="wm-program-desc">
                当前版本 <strong>{{ appVersion || '—' }}</strong>。通过 Tauri 2 updater 从发布服务器检查并安装新版本。
              </div>
              <div v-if="updateMessage" class="wm-update-message">{{ updateMessage }}</div>
              <div v-if="updateProgress" class="wm-update-progress">{{ updateProgress }}</div>
              <div v-if="availableUpdate?.body" class="wm-update-notes">
                {{ availableUpdate.body }}
              </div>
            </div>
            <div class="wm-update-actions">
              <el-button
                size="small"
                :loading="updateChecking"
                :disabled="updateInstalling"
                @click="checkUpdate"
              >
                检查更新
              </el-button>
              <el-button
                v-if="availableUpdate"
                size="small"
                type="primary"
                :loading="updateInstalling"
                @click="installUpdate"
              >
                下载并安装
              </el-button>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="AI 对话" name="ai">
        <div class="wm-program-settings">
          <p class="wm-settings-hint">
            右侧「AI 对话」使用 DeepSeek 读取当前文件夹中的文件并回答问题。Key 只保存在本机，
            Windows 上使用 DPAPI 加密（仅当前用户可解密）。
          </p>

          <div class="wm-program-row" style="align-items: flex-start">
            <div>
              <div class="wm-program-title">DeepSeek API Key</div>
              <div class="wm-program-desc">
                优先使用这里保存的 Key；留空则读取环境变量 <code>DEEPSEEK_APIKEY</code>。
                在 DeepSeek 开放平台创建 Key 后粘贴到这里即可。
              </div>
              <div
                class="wm-ai-setting-status"
                :class="{ 'is-ok': store.aiConfig && store.aiConfig.keySource !== 'none' }"
              >
                {{ aiKeySourceText }}
              </div>
            </div>
            <el-input
              v-model="aiKey"
              type="password"
              show-password
              :placeholder="aiKeyPlaceholder"
              style="width: 340px"
            />
          </div>

          <div class="wm-program-row">
            <div>
              <div class="wm-program-title">模型</div>
              <div class="wm-program-desc">
                默认 <code>deepseek-flash</code>（DeepSeek-V4.1-Flash，支持思考/非思考）；
                需要更强推理可选 <code>deepseek-v4-pro</code>。
              </div>
            </div>
            <el-select
              v-model="aiModel"
              style="width: 340px"
              filterable
              allow-create
              default-first-option
            >
              <el-option label="deepseek-flash" value="deepseek-flash" />
              <el-option label="deepseek-v4-pro" value="deepseek-v4-pro" />
            </el-select>
          </div>

          <div class="wm-program-row" style="align-items: flex-start">
            <div>
              <div class="wm-program-title">思考模式</div>
              <div class="wm-program-desc">
                默认关闭，读取文件时响应更快；开启后模型会先推理再回答，更稳但更慢。
                强度对应官网 <code>reasoning_effort</code> 的 low / high / max。
                切换后建议清空对话，让新设置从新一轮开始生效。
              </div>
            </div>
            <div style="display: flex; align-items: center; gap: 8px; width: 340px">
              <el-switch v-model="aiThinking" />
              <el-select
                v-model="aiReasoningEffort"
                :disabled="!aiThinking"
                style="flex: 1"
              >
                <el-option label="低 (low)" value="low" />
                <el-option label="高 (high)" value="high" />
                <el-option label="最高 (max)" value="max" />
              </el-select>
            </div>
          </div>

          <div class="wm-program-row">
            <div>
              <div class="wm-program-title">Base URL</div>
              <div class="wm-program-desc">
                默认 <code>https://api.deepseek.com</code>；使用中转服务时改成对应的
                OpenAI 兼容地址（通常到 <code>/v1</code> 为止，不要包含
                <code>/chat/completions</code>）。
              </div>
            </div>
            <el-input v-model="aiBaseUrl" style="width: 340px" placeholder="https://api.deepseek.com" />
          </div>

          <div class="wm-ai-setting-actions">
            <el-button size="small" type="primary" :loading="aiSaving" @click="saveAiSettings">
              保存
            </el-button>
            <el-button size="small" :loading="aiTesting" @click="testAiConnection">
              测试连接
            </el-button>
            <span
              v-if="aiTestMessage"
              class="wm-ai-setting-message"
              :class="{ 'is-error': !aiTestOk }"
            >
              {{ aiTestMessage }}
            </span>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="远程机器" name="remote">
        <div class="wm-remote-settings">
          <aside class="wm-remote-machines">
            <div class="wm-remote-machines-title">已添加的机器</div>
            <div
              v-for="p in store.remoteProfiles"
              :key="p.id"
              class="wm-remote-machine"
              :class="{ 'is-active': p.id === machine.id }"
              @click="selectMachine(p)"
            >
              <div class="wm-remote-machine-name">{{ p.name || p.host }}</div>
              <small>{{ p.user }}@{{ p.host }}:{{ p.port }}</small>
            </div>
            <div v-if="store.remoteProfiles.length === 0" class="wm-remote-empty">
              还没有添加任何远程机器
            </div>
            <el-button size="small" style="width: 100%; margin-top: 8px" @click="newMachine">
              ＋ 新建机器
            </el-button>
          </aside>

          <section class="wm-remote-form">
            <div class="wm-remote-form-title">
              {{ machine.id ? '编辑机器' : '新增机器' }}
            </div>
            <el-input v-model="machine.name" size="small" placeholder="名称（显示在侧边栏，可选）" style="margin-bottom: 8px" />
            <el-input v-model="machine.host" size="small" placeholder="主机名 / IP，例如 192.168.31.200" style="margin-bottom: 8px" />
            <div style="display: flex; gap: 8px; margin-bottom: 8px">
              <el-input v-model="machine.user" size="small" placeholder="用户名" />
              <el-input v-model.number="machine.port" size="small" placeholder="端口" style="width: 90px" />
            </div>
            <el-input
              v-model="machine.identityFile"
              size="small"
              placeholder="私钥路径（可选，留空则用 ssh-agent / ~/.ssh 配置）"
              style="margin-bottom: 8px"
            />
            <el-input
              v-model="machine.password"
              type="password"
              show-password
              size="small"
              placeholder="密码（可选；填写后优先使用密码登录）"
              style="margin-bottom: 12px"
            />

            <div class="wm-remote-job">
              <el-checkbox v-model="machine.jobQueryEnabled">启用作业查询（右侧面板）</el-checkbox>
              <div class="wm-remote-job-row">
                <el-input
                  v-model="machine.jobQueryCommand"
                  size="small"
                  :disabled="!machine.jobQueryEnabled"
                  placeholder="查询命令，例如 squeue -u $USER"
                />
                <el-input-number
                  v-model="machine.jobQueryInterval"
                  size="small"
                  :min="5"
                  :max="600"
                  :step="5"
                  :disabled="!machine.jobQueryEnabled"
                  controls-position="right"
                  style="width: 130px"
                />
                <span class="wm-remote-job-unit">秒</span>
              </div>
              <div class="wm-remote-job-hint">
                在右侧「作业查询」面板执行该命令并自动刷新，默认 <code>squeue -u $USER</code>。
              </div>
            </div>
            <div class="wm-remote-job" style="margin-top: 12px">
              <div style="font-weight: 600; margin-bottom: 6px">提交作业</div>
              <el-input
                v-model="machine.submitCommand"
                size="small"
                placeholder="提交命令，必须包含 $PATH，例如 sbatch $PATH"
                style="margin-bottom: 6px"
              />
              <div class="wm-remote-job-hint" style="margin-bottom: 8px">
                右键文件 →「提交作业」时，在这台机器上执行该命令；<code>$PATH</code>
                会替换为远端文件路径。命令通过远端登录 shell 执行（会加载服务器 PATH），
                留空表示这台机器不参与提交。
              </div>
              <el-input
                v-model="machine.submitDir"
                size="small"
                placeholder="默认提交路径，例如 ~/wmfold"
                style="margin-bottom: 6px"
              />
              <div class="wm-remote-job-hint" style="margin-bottom: 8px">
                目标机器当前正在浏览某个文件夹时提交到该文件夹；否则提交到这里的默认路径。
              </div>
              <el-checkbox v-model="machine.clearChkPath">
                清除 chk 路径（gjf 的 %chk= 只保留文件名）
              </el-checkbox>
            </div>
            <div style="display: flex; gap: 8px">
              <el-button size="small" type="primary" @click="saveMachine">保存</el-button>
              <el-button size="small" type="danger" plain :disabled="!machine.id" @click="removeMachine">
                删除
              </el-button>
            </div>
            <div class="wm-remote-hint">
              支持 SSH 密码或密钥登录（密码使用随包的 plink 连接，保存在本机并用 Windows
              DPAPI 加密；留空则用密钥 / ssh-agent）。连接时若服务器没有 wmfold，
              会自动安装随包的 Linux 版本到 <code>~/.local/bin/wmfold</code>。
            </div>
            <div v-if="store.activeRemote?.id === machine.id && machine.id" class="wm-remote-current">
              当前正在浏览这台机器
            </div>
          </section>
        </div>
      </el-tab-pane>
    </el-tabs>

    <template #footer>
      <el-button v-if="tab === 'display'" @click="store.resetColumnSettings()">恢复默认</el-button>
      <el-button type="primary" @click="visible = false">关闭</el-button>
    </template>
  </el-dialog>
</template>
