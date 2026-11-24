// 与 Rust 端数据结构一一对应的 TypeScript 类型。

export interface ChemMeta {
  /** 人类可读文件类型，如 "高斯输入文件" */
  fileType: string;
  /** 短类型代码，如 "gjf" */
  fileKind: string;
  /** 化学式，如 "H2O" */
  formula: string;
  /** 原子数 */
  nAtoms: number;
  /** 总电荷 */
  charge: number;
  /** 自旋多重度 */
  multip: number;
  /** α 电子数 */
  nAlpha: number;
  /** β 电子数 */
  nBeta: number;
  /** 基组函数数 */
  nBasis: number;
  /** 分子量 */
  molW: number;
  /** 总能量（Hartree） */
  energy: number | null;
  /** gjf / log 的 Gaussian 作业关键词（route） */
  job: string | null;
  /** log 文件是否正常结束（rswfn read_normalend）；非 log 为 null */
  normalEnd: boolean | null;
  /** 是否解析成功 */
  ok: boolean;
}

export interface MolAtom {
  element: string;
  x: number;
  y: number;
  z: number;
}

export interface MolStructure {
  ok: boolean;
  atoms: MolAtom[];
  /** 成键原子索引对，索引对应 atoms */
  bonds: [number, number][];
}

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  created: number | null;
  modified: number | null;
  extension: string;
  /** 化学类型代码（gjf 等），非化学文件为空 */
  kind: string;
  isChem: boolean;
  isHidden: boolean;
  isSymlink: boolean;
  chem: ChemMeta | null;
}

export interface DriveInfo {
  letter: string;
  label: string;
  total: number | null;
  free: number | null;
}

export interface QuickAccess {
  name: string;
  path: string;
  icon: string;
  /** 是否为用户右键“添加到快速访问”固定的目录（内置项为 false） */
  pinned: boolean;
}

export type SortOrder = 'ascending' | 'descending' | null;
export type SortProp = string;

/** 远程服务器配置（对应 Rust `remotes::RemoteProfile`）。 */
export interface RemoteProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  user: string;
  identityFile?: string | null;
  /** 密码登录（可选；填写后优先用随包的 plink 连接） */
  password?: string | null;
  /** 首次密码登录后自动缓存的主机密钥指纹 */
  hostKey?: string | null;
  /** 是否启用右侧作业查询面板 */
  jobQueryEnabled?: boolean;
  /** 作业查询命令，例如 squeue -u $USER */
  jobQueryCommand?: string;
  /** 自动刷新间隔（秒） */
  jobQueryInterval?: number;
  /** 提交作业命令模板；必须包含 $PATH 占位符，留空表示该机器不参与提交 */
  submitCommand?: string;
  /** 默认提交目录；未打开目标机器文件夹时使用，默认 ~/wmfold */
  submitDir?: string;
  /** 提交 gjf 时把 %chk= 的路径去掉，只保留文件名 */
  clearChkPath?: boolean;
}

/** 提交作业结果。 */
export interface SubmitJobResult {
  ok: boolean;
  command: string;
  remotePath: string;
  stdout: string;
  stderr: string;
  exitCode: number | null;
}

/** 连接检查结果。 */
export interface RemoteStatus {
  os: string;
  home: string;
  installed: boolean;
  wmfoldVersion: string | null;
  binaryPath: string | null;
  outdated: boolean;
}

/** 远端 `wmfold -cli list` 的返回。 */
export interface RemoteListing {
  protocol: number;
  path: string;
  entries: FileEntry[];
}

/**
 * 用户设置（对应 Rust `settings::Settings`）。
 * 注意：JSON 键名保持 snake_case，与 0.3.4 GPUI 版写出的 settings.json 兼容。
 */
export interface Settings {
  column_settings: Record<string, string[]>;
  /** 程序设置：是否显示隐藏文件 */
  show_hidden: boolean;
  /** 程序设置：外部终端工具（auto / wt / powershell / cmd） */
  terminal_tool: string;
}

// ---- 递归读取 ----

/** 递归扫描进度事件（后端 `scan-progress`）。 */
export interface ScanProgressEvent {
  scanId: number;
  dirs: number;
  files: number;
  current: string;
  elapsedMs: number;
  done: boolean;
  cancelled: boolean;
  truncated: boolean;
}

/** `scan_recursive` 的返回值。 */
export interface ScanResult {
  entries: FileEntry[];
  dirs: number;
  files: number;
  skipped: number;
  truncated: boolean;
  cancelled: boolean;
  elapsedMs: number;
}

/** 递归视图的统计信息。 */
export interface ScanStats {
  dirs: number;
  files: number;
  skipped: number;
  truncated: boolean;
  cancelled: boolean;
  elapsedMs: number;
}

// ---- AI 对话（DeepSeek） ----

export interface AiChatMessage {
  role: 'user' | 'assistant';
  content: string;
  /** 思考模式：上一轮 assistant 的思维链，工具调用请求需要回传 */
  reasoningContent?: string;
}

/** 工具返回的一个可点击文件 / 文件夹结果。 */
export interface AiToolResult {
  /** 绝对路径 */
  path: string;
  name: string;
  /** 相对于当前文件夹的显示路径 */
  rel: string;
  isDir: boolean;
}

/** 一次工具调用（读取了哪个文件 / 搜索了什么）。 */
export interface AiToolStep {
  id: string;
  name: string;
  label: string;
  status: 'running' | 'ok' | 'error';
  summary: string;
  results: AiToolResult[];
}

export interface AiReply {
  content: string;
  steps: AiToolStep[];
  model: string;
  reasoningContent?: string;
}

/** 后端 `ai-tool` 事件。 */
export interface AiToolEvent {
  requestId: number;
  step: AiToolStep;
}

/** AI 配置（Key 来源、模型、Base URL）。 */
export interface AiConfigView {
  /** 已保存的 API Key（仅用于设置页编辑） */
  apiKey: string;
  hasEnvKey: boolean;
  /** 环境变量 Key 的末 4 位 */
  envKeyPreview: string;
  /** saved / env / none */
  keySource: 'saved' | 'env' | 'none';
  keyPreview: string;
  model: string;
  baseUrl: string;
  /** 思考模式开关 */
  thinking: boolean;
  /** low / high / max */
  reasoningEffort: string;
}

/** 右侧面板 tab。 */
export type RightPanelTab = 'preview' | 'ai' | 'jobs';
