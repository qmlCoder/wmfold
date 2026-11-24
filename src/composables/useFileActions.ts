// 文件操作（新建 / 删除 / 重命名）的共用封装。
// 命令栏按钮与全局快捷键走同一套逻辑，避免两处行为不一致；操作结果直接反映在列表中，
// 不使用自动消失的 ElMessage 弹窗。

import { ElMessageBox } from 'element-plus';
import { useExplorer } from '../stores/explorer';

export function useFileActions() {
  const store = useExplorer();

  async function newFolder() {
    try {
      const { value } = await ElMessageBox.prompt('请输入文件夹名称', '新建文件夹', {
        confirmButtonText: '创建',
        cancelButtonText: '取消',
        inputValue: '新建文件夹',
        inputPlaceholder: '文件夹名称',
      });
      if (value) {
        await store.createFolder(value);
      }
    } catch {
      /* 取消 */
    }
  }

  async function newFile() {
    try {
      const { value } = await ElMessageBox.prompt('请输入文件名（含扩展名）', '新建文件', {
        confirmButtonText: '创建',
        cancelButtonText: '取消',
        inputValue: '新建化学文件.xyz',
        inputPlaceholder: '文件名',
      });
      if (value) {
        await store.createFile(value);
      }
    } catch {
      /* 取消 */
    }
  }

  /**
   * 删除选中项（默认进回收站）。
   * @param warnWhenEmpty 没有选中项时是否弹出提示（快捷键触发时保持静默）
   */
  async function deleteSelected(_warnWhenEmpty = false) {
    if (!store.hasSelection) return;
    try {
      await ElMessageBox.confirm(
        `确定要删除选中的 ${store.selectedEntries.length} 个项目吗？`,
        '删除',
        { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' },
      );
      await store.deleteSelected(false);
    } catch {
      /* 取消 */
    }
  }

  /**
   * 重命名当前选中的单个项目。
   * @param warnWhenEmpty 没有选中项时是否弹出提示（快捷键触发时保持静默）
   */
  async function rename(_warnWhenEmpty = false) {
    const sel = store.selectedEntries[0];
    if (!sel) return;
    try {
      const { value } = await ElMessageBox.prompt('请输入新名称', '重命名', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        inputValue: sel.name,
      });
      if (value) {
        await store.renameEntry(sel, value);
      }
    } catch {
      /* 取消 */
    }
  }

  return { newFolder, newFile, deleteSelected, rename };
}
