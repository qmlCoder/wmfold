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
  /** 是否解析成功 */
  ok: boolean;
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
}

export type SortOrder = 'ascending' | 'descending' | null;
export type SortProp = string;

/**
 * 用户设置（对应 Rust `settings::Settings`）。
 * 注意：JSON 键名保持 snake_case，与 0.3.4 GPUI 版写出的 settings.json 兼容。
 */
export interface Settings {
  column_settings: Record<string, string[]>;
}
