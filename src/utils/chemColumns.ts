// 化学信息列的共享配置：列定义、各类型可用列、类型筛选/设置所用常量。
// 被 FileList(表格)、FilterBar(类型按钮)、SettingsDialog(设置勾选) 共同使用。

export interface ChemColDef {
  label: string;
  minWidth: number;
  sortable: boolean;
  align?: 'left' | 'right' | 'center';
}

/** 各化学信息列的展示配置 */
export const CHEM_DEFS: Record<string, ChemColDef> = {
  formula: { label: '化学式', minWidth: 96, sortable: true },
  nAtoms: { label: '原子数', minWidth: 60, sortable: true, align: 'right' },
  charge: { label: '电荷', minWidth: 52, sortable: true, align: 'right' },
  multip: { label: '多重度', minWidth: 60, sortable: true, align: 'right' },
  nBasis: { label: '基组函数', minWidth: 70, sortable: true, align: 'right' },
  molW: { label: '分子量', minWidth: 78, sortable: true, align: 'right' },
  energy: { label: '能量 (a.u.)', minWidth: 86, sortable: true, align: 'right' },
  job: { label: '作业', minWidth: 240, sortable: false },
  normalEnd: { label: '正常结束', minWidth: 90, sortable: false },
};

/** 「全部」默认展示的化学列 */
export const CHEM_ALL = ['formula', 'nAtoms', 'charge', 'multip', 'molW', 'energy', 'job', 'normalEnd'];

/** 每种具体类型可提供的化学列（作为「有限表头」的可选项） */
export const CHEM_BY_KIND: Record<string, string[]> = {
  gjf: ['formula', 'nAtoms', 'charge', 'multip', 'molW', 'job'],
  log: ['formula', 'nAtoms', 'charge', 'multip', 'energy', 'molW', 'job', 'normalEnd'],
  out: ['formula', 'nAtoms', 'charge', 'multip', 'energy', 'molW'],
  fch: ['formula', 'nAtoms', 'charge', 'multip', 'nBasis', 'molW'],
  inp: ['formula', 'nAtoms', 'charge', 'multip', 'molW'],
  mol: ['formula', 'nAtoms', 'molW'],
  sdf: ['formula', 'nAtoms', 'molW'],
  xyz: ['formula', 'nAtoms', 'molW'],
  molden: ['formula', 'nAtoms', 'multip', 'molW'],
  dmo: ['formula', 'nAtoms', 'molW'],
  cub: ['nAtoms'],
};

/** 参与筛选/设置的类型代码（不含 全部/文件夹/化学文件） */
export const FILTER_KINDS = ['gjf', 'log', 'out', 'fch', 'xyz', 'mol', 'sdf', 'molden', 'dmo', 'inp', 'cub'];

/** 类型筛选用按钮文案 */
export const KIND_LABELS: Record<string, string> = {
  gjf: '高斯输入',
  log: '高斯日志',
  out: '量化输出',
  fch: '检查点',
  xyz: 'XYZ',
  mol: 'MOL',
  sdf: 'SDF',
  molden: 'Molden',
  dmo: 'DMol',
  inp: 'Orca 输入',
  cub: '格点',
};
