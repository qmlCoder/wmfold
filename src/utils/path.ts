// Windows 路径解析辅助。

/** 虚拟根，"此电脑" 视图 */
export const THIS_PC = '::ThisPC';

/** 规范化分隔符，统一为 '/' 便于拆分 */
function toSegs(p: string): string[] {
  return p.replace(/\\/g, '/').split('/').filter(Boolean);
}

/** 是否为 "此电脑" 虚拟路径 */
export function isThisPc(p: string): boolean {
  return p === THIS_PC;
}

/** 是否为盘符（如 "C:"） */
function isDrive(seg: string): boolean {
  return /^[a-zA-Z]:$/.test(seg);
}

/** 从路径获取面包屑段列表，如 ["C:", "Users", "you"] */
export function pathSegments(p: string): string[] {
  return toSegs(p);
}

/** 拼接完整 Windows 路径 */
export function joinPath(segs: string[]): string {
  if (segs.length === 0) return '';
  // 盘符根
  if (segs.length === 1 && isDrive(segs[0])) return `${segs[0]}\\`;
  if (segs.length >= 1 && isDrive(segs[0])) return segs.join('\\');
  // 无盘符的相对路径
  return segs.join('\\');
}

/** 获取父路径 */
export function parentPath(p: string): string {
  if (isThisPc(p)) return THIS_PC;
  const segs = toSegs(p);
  if (segs.length === 0) return THIS_PC;
  // 根盘符，如 "C:"
  if (segs.length === 1 && isDrive(segs[0])) return THIS_PC;
  segs.pop();
  return joinPath(segs);
}

/** 添加子路径 */
export function childPath(p: string, name: string): string {
  if (isThisPc(p)) return `${name}\\`;
  const segs = toSegs(p);
  segs.push(name);
  return joinPath(segs);
}

/** 将当前路径与目标盘符/段组合为可导航路径 */
export function asPath(p: string): string {
  if (isThisPc(p)) return p;
  return p;
}

/** 面包屑各节点的可导航路径 */
export function breadcrumbPaths(p: string): { label: string; path: string }[] {
  if (isThisPc(p)) return [{ label: '此电脑', path: THIS_PC }];
  const segs = toSegs(p);
  const crumbs: { label: string; path: string }[] = [{ label: '此电脑', path: THIS_PC }];
  let acc: string[] = [];
  if (segs.length > 0 && isDrive(segs[0])) {
    acc.push(segs[0]);
    crumbs.push({ label: segs[0], path: joinPath(acc) });
    for (let i = 1; i < segs.length; i++) {
      acc.push(segs[i]);
      crumbs.push({ label: segs[i], path: joinPath(acc) });
    }
  } else {
    for (const s of segs) {
      acc.push(s);
      crumbs.push({ label: s, path: joinPath(acc) });
    }
  }
  return crumbs;
}
