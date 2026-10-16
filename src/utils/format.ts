// 格式化与图标等展示辅助函数。

/** 字节数格式化为可读字符串 */
export function formatSize(bytes: number): string {
  if (bytes === 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const v = bytes / Math.pow(1024, i);
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

/** 毫秒时间戳格式化为 "yyyy/MM/dd HH:mm" */
export function formatDate(ms: number | null | undefined): string {
  if (ms == null) return '—';
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 化学类型代码 → 图标名 */
export function chemIcon(kind: string): string {
  const map: Record<string, string> = {
    gjf: 'Document',
    log: 'Document',
    out: 'Document',
    fch: 'Document',
    mol: 'Document',
    sdf: 'Document',
    xyz: 'Document',
    molden: 'Document',
    dmo: 'Document',
    inp: 'Document',
    cub: 'DataLine',
  };
  return map[kind] ?? 'Document';
}

/** 文件扩展名 → 图标名（Element Plus 图标） */
export function fileIcon(entry: { isDir: boolean; extension: string; kind: string; isChem: boolean }): string {
  if (entry.isDir) return 'Folder';
  const map: Record<string, string> = {
    txt: 'Document',
    md: 'Document',
    pdf: 'Document',
    doc: 'Document',
    docx: 'Document',
    xls: 'Document',
    xlsx: 'Document',
    ppt: 'Document',
    pptx: 'Document',
    jpg: 'Picture',
    jpeg: 'Picture',
    png: 'Picture',
    gif: 'Picture',
    bmp: 'Picture',
    webp: 'Picture',
    svg: 'Picture',
    mp3: 'Headset',
    wav: 'Headset',
    flac: 'Headset',
    mp4: 'VideoCamera',
    avi: 'VideoCamera',
    mkv: 'VideoCamera',
    mov: 'VideoCamera',
    zip: 'Files',
    rar: 'Files',
    '7z': 'Files',
    gz: 'Files',
    tar: 'Files',
    exe: 'Cpu',
    dll: 'Cpu',
    py: 'Document',
    rs: 'Document',
    json: 'Document',
    csv: 'Document',
  };
  return map[entry.extension] ?? (entry.isChem ? 'Document' : 'Document');
}

/** 根据文件类型返回描述文本 */
export function typeLabel(entry: { isDir: boolean; extension: string; kind: string; isChem: boolean }): string {
  if (entry.isDir) return '文件夹';
  const chemLabels: Record<string, string> = {
    gjf: '高斯输入文件',
    log: '高斯日志文件',
    out: '量化输出文件',
    fch: '高斯检查点文件',
    mol: 'MDL 分子文件',
    sdf: 'SDF 结构文件',
    xyz: 'XYZ 坐标文件',
    molden: 'Molden 文件',
    dmo: 'DMol 文件',
    inp: 'Orca 输入文件',
    cub: '格点文件',
  };
  if (chemLabels[entry.kind]) return `${chemLabels[entry.kind]}`;
  return `${entry.extension.toUpperCase()} 文件`;
}

/** 化学类型代码 → 简短标签 */
export function kindShort(kind: string): string {
  const map: Record<string, string> = {
    gjf: 'GJF',
    log: 'LOG',
    out: 'OUT',
    fch: 'FCHK',
    mol: 'MOL',
    sdf: 'SDF',
    xyz: 'XYZ',
    molden: 'MOLDEN',
    dmo: 'DMO',
    inp: 'INP',
    cub: 'CUB',
  };
  return map[kind] ?? '';
}

/** 化学文件类型配色 */
export function chemColor(kind: string): string {
  const map: Record<string, string> = {
    gjf: '#409EFF',
    log: '#67C23A',
    out: '#E6A23C',
    fch: '#F56C6C',
    mol: '#909399',
    sdf: '#909399',
    xyz: '#7A9E4E',
    molden: '#8B5CF6',
    dmo: '#0BB3A7',
    inp: '#FF8C00',
    cub: '#3B82F6',
  };
  return map[kind] ?? '#909399';
}
