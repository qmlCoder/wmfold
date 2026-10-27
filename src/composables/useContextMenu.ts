import { reactive } from 'vue';
import type { FileEntry } from '../types';

// 全局唯一的右键菜单状态（供 FileList 触发，ContextMenu 渲染）。
const state = reactive<{
  visible: boolean;
  x: number;
  y: number;
  target: FileEntry | null;
}>({
  visible: false,
  x: 0,
  y: 0,
  target: null,
});

export function useContextMenu() {
  function open(x: number, y: number, target: FileEntry | null) {
    state.x = x;
    state.y = y;
    state.target = target;
    state.visible = true;
  }

  function close() {
    state.visible = false;
    state.target = null;
  }

  return { state, open, close };
}
