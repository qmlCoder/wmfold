// Tauri 后端命令的 TypeScript 封装。

import { invoke } from '@tauri-apps/api/core';
import type {
  AiChatMessage,
  AiConfigView,
  AiReply,
  ChemMeta,
  DriveInfo,
  FileEntry,
  MolStructure,
  QuickAccess,
  RemoteListing,
  RemoteProfile,
  RemoteStatus,
  ScanResult,
  Settings,
  SubmitJobResult,
} from '../types';

export const api = {
  listDir: (path: string) => invoke<FileEntry[]>('list_dir', { path }),
  getHomeDir: () => invoke<string>('get_home_dir'),
  /** 资源管理器右键菜单启动时携带的目录（--open） */
  takeLaunchPath: () => invoke<string | null>('take_launch_path'),
  getQuickAccess: () => invoke<QuickAccess[]>('get_quick_access'),
  pathsExist: (paths: string[]) => invoke<boolean[]>('paths_exist', { paths }),
  /** 把文件夹添加到快速访问；已存在（含内置项）时返回 false */
  addQuickAccess: (path: string) => invoke<boolean>('add_quick_access', { path }),
  /** 取消快速访问中的固定项 */
  removeQuickAccess: (path: string) => invoke<void>('remove_quick_access', { path }),
  getDrives: () => invoke<DriveInfo[]>('get_drives'),

  createFolder: (path: string) => invoke<void>('create_folder', { path }),
  createFile: (path: string) => invoke<void>('create_file', { path }),
  rename: (path: string, newName: string) => invoke<void>('rename_path', { path, newName }),
  remove: (paths: string[], permanent: boolean) => invoke<void>('delete_paths', { paths, permanent }),
  copy: (paths: string[], destDir: string) => invoke<void>('copy_paths', { paths, destDir }),
  move: (paths: string[], destDir: string) => invoke<void>('move_paths', { paths, destDir }),
  open: (path: string) => invoke<void>('open_path', { path }),
  openInExplorer: (path: string) => invoke<void>('open_in_explorer', { path }),

  readChem: (path: string) => invoke<ChemMeta>('read_chem', { path }),
  readChemBatch: (paths: string[]) => invoke<ChemMeta[]>('read_chem_batch', { paths }),
  flushChemCache: () => invoke<void>('flush_chem_cache'),
  readStructure: (path: string) => invoke<MolStructure>('read_structure', { path }),

  /** 递归读取当前文件夹下的所有子文件夹（通过 scan-progress 事件报告进度）。 */
  scanRecursive: (path: string, scanId: number, includeHidden: boolean, maxEntries?: number) =>
    invoke<ScanResult>('scan_recursive', { path, scanId, includeHidden, maxEntries }),
  cancelScan: (scanId: number) => invoke<void>('cancel_scan', { scanId }),

  // ---- AI 对话（DeepSeek） ----
  aiConfig: () => invoke<AiConfigView>('ai_config'),
  aiSaveConfig: (
    apiKey: string,
    model: string,
    baseUrl: string,
    thinking: boolean,
    reasoningEffort: string,
  ) =>
    invoke<AiConfigView>('ai_save_config', {
      apiKey,
      model,
      baseUrl,
      thinking,
      reasoningEffort,
    }),
  aiTest: () => invoke<string>('ai_test'),
  aiChat: (payload: {
    requestId: number;
    root: string;
    messages: AiChatMessage[];
    selected: string[];
    recursive: boolean;
  }) => invoke<AiReply>('ai_chat', payload),

  // 用户设置（化学列展示 + 程序设置），保存在 %APPDATA%\wmfold\settings.json
  loadSettings: () => invoke<Settings>('load_settings'),
  saveSettings: (settings: Settings) => invoke<void>('save_settings', { settings }),

  // 远程服务器（SSH + 远端 wmfold -cli）
  remoteProfiles: () => invoke<RemoteProfile[]>('remote_profiles'),
  remoteSaveProfile: (profile: RemoteProfile) =>
    invoke<RemoteProfile[]>('remote_save_profile', { profile }),
  remoteDeleteProfile: (id: string) =>
    invoke<RemoteProfile[]>('remote_delete_profile', { id }),
  remoteCheck: (profile: RemoteProfile) =>
    invoke<RemoteStatus>('remote_check', { profile }),
  remoteInstall: (profile: RemoteProfile) =>
    invoke<string>('remote_install', { profile }),
  remoteList: (profile: RemoteProfile, path: string, chem: boolean) =>
    invoke<RemoteListing>('remote_list', { profile, path, chem }),
  remoteInfo: (profile: RemoteProfile, path: string) =>
    invoke<FileEntry>('remote_info', { profile, path }),
  remoteStructure: (profile: RemoteProfile, path: string) =>
    invoke<MolStructure>('remote_structure', { profile, path }),
  remoteHome: (profile: RemoteProfile) =>
    invoke<string>('remote_home', { profile }),
  remoteQuickAccess: (machine: string) =>
    invoke<QuickAccess[]>('remote_quick_access', { machine }),
  remoteAddQuickAccess: (machine: string, path: string) =>
    invoke<boolean>('remote_add_quick_access', { machine, path }),
  remoteRemoveQuickAccess: (machine: string, path: string) =>
    invoke<void>('remote_remove_quick_access', { machine, path }),
  remotePathsExist: (profile: RemoteProfile, paths: string[]) =>
    invoke<boolean[]>('remote_paths_exist', { profile, paths }),

  // 跨设备文件传输（本地 ↔ 远程，远程 ↔ 远程）
  remoteUpload: (profile: RemoteProfile, sources: string[], destDir: string) =>
    invoke<void>('remote_upload', { profile, sources, destDir }),
  remoteDownload: (profile: RemoteProfile, sources: string[], destDir: string) =>
    invoke<void>('remote_download', { profile, sources, destDir }),
  remoteCopyWithin: (
    profile: RemoteProfile,
    sources: string[],
    destDir: string,
    moveMode: boolean,
  ) => invoke<void>('remote_copy_within', { profile, sources, destDir, moveMode }),
  remoteDelete: (profile: RemoteProfile, paths: string[]) =>
    invoke<void>('remote_delete', { profile, paths }),
  /** 把源文件提交到目标机器（自动上传/改写 chk/执行提交命令） */
  submitJob: (payload: {
    sourceProfile: RemoteProfile | null;
    sourcePath: string;
    targetProfile: RemoteProfile;
    targetDir: string | null;
  }) => invoke<SubmitJobResult>('submit_job', payload),
  remoteTransfer: (
    from: RemoteProfile,
    to: RemoteProfile,
    sources: string[],
    destDir: string,
    moveMode: boolean,
  ) => invoke<void>('remote_transfer', { from, to, sources, destDir, moveMode }),

  /** 远程作业查询（squeue 等，执行配置的命令并返回文本输出） */
  remoteJobQuery: (profile: RemoteProfile, command: string) =>
    invoke<string>('remote_job_query', { profile, command }),

  // 用系统终端打开当前目录；远程机器会新开一个 ssh 窗口
  openInTerminal: (path: string, remote: RemoteProfile | null) =>
    invoke<void>('open_in_terminal', { path, remote }),

  // 多窗口：机器 tab 拖出 / 合并
  mergeMachineWindow: (target: string, machine: string, path: string) =>
    invoke<void>('merge_machine_window', { target, machine, path }),
  windowAtPoint: (x: number, y: number) =>
    invoke<string | null>('window_at_point', { x, y }),
  registerMachineWindow: (label: string, machine: string, path: string) =>
    invoke<void>('register_machine_window', { label, machine, path }),
};
