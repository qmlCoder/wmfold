// Tauri 后端命令的 TypeScript 封装。

import { invoke } from '@tauri-apps/api/core';
import type { ChemMeta, DriveInfo, FileEntry, QuickAccess, Settings } from '../types';

export const api = {
  listDir: (path: string) => invoke<FileEntry[]>('list_dir', { path }),
  getHomeDir: () => invoke<string>('get_home_dir'),
  getQuickAccess: () => invoke<QuickAccess[]>('get_quick_access'),
  getDrives: () => invoke<DriveInfo[]>('get_drives'),

  createFolder: (path: string) => invoke<void>('create_folder', { path }),
  createFile: (path: string) => invoke<void>('create_file', { path }),
  rename: (path: string, newName: string) => invoke<void>('rename_path', { path, newName }),
  remove: (paths: string[], permanent: boolean) => invoke<void>('delete_paths', { paths, permanent }),
  copy: (paths: string[], destDir: string) => invoke<void>('copy_paths', { paths, destDir }),
  move: (paths: string[], destDir: string) => invoke<void>('move_paths', { paths, destDir }),
  open: (path: string) => invoke<void>('open_path', { path }),

  readChem: (path: string) => invoke<ChemMeta>('read_chem', { path }),
  readChemBatch: (paths: string[]) => invoke<ChemMeta[]>('read_chem_batch', { paths }),

  // 用户设置（每种类型显示哪些化学列），保存在 %APPDATA%\wmfold\settings.json
  loadSettings: () => invoke<Settings>('load_settings'),
  saveSettings: (columnSettings: Record<string, string[]>) =>
    invoke<void>('save_settings', { settings: { column_settings: columnSettings } }),
};
