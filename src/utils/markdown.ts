// 极简、安全的 Markdown 渲染：先转义 HTML，再只生成白名单标签。
// 只用于 AI 回复的展示，不处理链接 / 图片，避免把模型输出当成 HTML 执行。
// 支持标题、列表、代码块、行内代码、粗体以及 GitHub 风格表格。

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function inline(text: string): string {
  return text
    .replace(/`([^`\n]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
}

const PATH_MARKER = '\u0001';
const PATH_EXT =
  'gjf|log|out|fchk?|xyz|mol|sdf|molden|dmo|inp|cub|chk|wfn|wfx|txt|json|md|py|rs|tsx?|jsx?|vue|css|html|csv|dat|png|jpe?g|gif|pdf|zip|gz|tar|bat|ps1|sh';
// 文件路径（带扩展名）或文件夹（以 / 或 \ 结尾）。前后加边界，避免把普通单词拆开。
const PATH_RE = new RegExp(
  `(?<![\\p{L}\\p{N}_.+\\-])(?:` +
    `(?:[\\p{L}\\p{N}_.+\\-]+[\\\\/])*[\\p{L}\\p{N}_.+\\-]+\\.(?:${PATH_EXT})` +
    `|(?:[\\p{L}\\p{N}_.+\\-]+[\\\\/])+` +
    `)(?![\\p{L}\\p{N}_.+\\-\\\\/])`,
  'giu',
);

function isLikelyPath(token: string, before: string): boolean {
  if (!token) return false;
  if (before.includes('://')) return false;
  if (/\.(?:com|cn|org|net|io|dev)\/$/i.test(token)) return false;
  const trimmed = token.replace(/[\\/]+$/, '');
  if (!trimmed) return false;
  // 纯数字/日期的斜杠串（如 2026/10/14）不是路径
  if (!/[A-Za-z\u4e00-\u9fa5]/.test(trimmed)) return false;
  return true;
}

/**
 * 转义并渲染行内格式，同时把路径变成可点击的 span。
 * 先在原始文本里插入占位符，再走 escape/inline，避免路径中的字符被格式规则破坏。
 */
function inlineWithPaths(text: string): string {
  const paths: { path: string; dir: boolean; display: string }[] = [];
  let marked = '';
  let last = 0;
  PATH_RE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = PATH_RE.exec(text)) !== null) {
    const token = match[0];
    const before = text.slice(Math.max(0, match.index - 8), match.index);
    if (!isLikelyPath(token, before)) continue;
    const dir = /[\\/]$/.test(token);
    marked += text.slice(last, match.index);
    marked += `${PATH_MARKER}${paths.length}${PATH_MARKER}`;
    paths.push({ path: token, dir, display: token });
    last = match.index + token.length;
  }
  if (!paths.length) return inline(escapeHtml(text));
  marked += text.slice(last);

  return inline(escapeHtml(marked)).replace(
    new RegExp(`${PATH_MARKER}(\\d+)${PATH_MARKER}`, 'g'),
    (_, index: string) => {
      const item = paths[Number(index)];
      const rawPath = item.path.replace(/[\\/]+$/, '');
      return `<span class="wm-ai-path" role="link" tabindex="0" data-wm-path="${escapeHtml(rawPath)}" data-wm-dir="${item.dir ? '1' : '0'}" title="点击在文件列表中定位">${escapeHtml(item.display)}</span>`;
    },
  );
}

const PIPE_PLACEHOLDER = '\u0000';

/** 按 `|` 拆分表格行，支持 \| 转义和首尾竖线。 */
function splitTableRow(line: string): string[] {
  let text = line.trim();
  if (text.startsWith('|')) text = text.slice(1);
  if (text.endsWith('|')) text = text.slice(0, -1);
  text = text.replace(/\\\|/g, PIPE_PLACEHOLDER);
  return text
    .split('|')
    .map((cell) => cell.split(PIPE_PLACEHOLDER).join('|').trim());
}

/** 第二行是否是 `| --- | :---: |` 这样的表格分隔行。 */
function isTableSeparator(line: string): boolean {
  if (!line.includes('|')) return false;
  const cells = splitTableRow(line);
  return cells.length > 0 && cells.every((cell) => /^:?-{2,}:?$/.test(cell.replace(/\s+/g, '')));
}

function alignStyle(align: string): string {
  return align ? ` style="text-align:${align}"` : '';
}

function renderTable(headers: string[], aligns: string[], rows: string[][]): string {
  const head = headers
    .map((cell, index) => `<th${alignStyle(aligns[index])}>${inlineWithPaths(cell)}</th>`)
    .join('');
  const body = rows
    .map(
      (cells) =>
        `<tr>${headers
          .map(
            (_, index) =>
              `<td${alignStyle(aligns[index])}>${inlineWithPaths(cells[index] ?? '')}</td>`,
          )
          .join('')}</tr>`,
    )
    .join('');
  return `<div class="wm-ai-table-wrap"><table class="wm-ai-table"><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table></div>`;
}

/** 把 AI 回复渲染为可安全插入的 HTML 片段。 */
export function renderMarkdown(source: string): string {
  const lines = source.replace(/\r\n?/g, '\n').split('\n');
  const out: string[] = [];
  let code: string[] | null = null;
  let list: string[] | null = null;

  const flushList = () => {
    if (list && list.length) {
      out.push(`<ul>${list.map((item) => `<li>${item}</li>`).join('')}</ul>`);
    }
    list = null;
  };

  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i];

    if (code) {
      if (raw.trim().startsWith('```')) {
        out.push(`<pre class="wm-ai-code"><code>${escapeHtml(code.join('\n'))}</code></pre>`);
        code = null;
      } else {
        code.push(raw);
      }
      continue;
    }
    if (raw.trim().startsWith('```')) {
      flushList();
      code = [];
      continue;
    }

    const line = raw.trimEnd();
    if (!line.trim()) {
      flushList();
      continue;
    }

    // GitHub 风格表格：当前行是表头，下一行是 | --- | --- |
    if (line.includes('|') && i + 1 < lines.length && isTableSeparator(lines[i + 1])) {
      flushList();
      const headers = splitTableRow(line);
      const aligns = splitTableRow(lines[i + 1]).map((cell) => {
        const text = cell.replace(/\s+/g, '');
        const left = text.startsWith(':');
        const right = text.endsWith(':');
        if (left && right) return 'center';
        if (right) return 'right';
        if (left) return 'left';
        return '';
      });
      const rows: string[][] = [];
      let next = i + 2;
      while (next < lines.length) {
        const rowLine = lines[next];
        if (!rowLine.trim() || !rowLine.includes('|')) break;
        if (isTableSeparator(rowLine)) {
          next++;
          continue;
        }
        rows.push(splitTableRow(rowLine));
        next++;
      }
      out.push(renderTable(headers, aligns, rows));
      i = next - 1;
      continue;
    }

    if (/^#{1,6}\s+/.test(line)) {
      flushList();
      out.push(
        `<div class="wm-ai-h">${inlineWithPaths(line.replace(/^#{1,6}\s+/, ''))}</div>`,
      );
      continue;
    }
    if (/^\s*[-*+]\s+/.test(line)) {
      (list ??= []).push(inlineWithPaths(line.replace(/^\s*[-*+]\s+/, '')));
      continue;
    }
    if (/^\s*\d+[.)]\s+/.test(line)) {
      flushList();
      out.push(`<div class="wm-ai-li">${inlineWithPaths(line)}</div>`);
      continue;
    }

    flushList();
    out.push(`<div class="wm-ai-p">${inlineWithPaths(line)}</div>`);
  }

  if (code) {
    out.push(`<pre class="wm-ai-code"><code>${escapeHtml(code.join('\n'))}</code></pre>`);
  }
  flushList();
  return out.join('');
}
