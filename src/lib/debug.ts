/**
 * 用户行为记录：调试模式下把「用户点了什么」写进调试日志，用于复现操作类 bug。
 *
 * 原则：只记语义动作（「点击了导入按钮」「拖拽导入 2 个文件」），不记原始事件
 * （鼠标移动/击键）——后者是噪音，会淹没信号。记录时机是**动作发起时**，
 * 结果由后端钩子 / 错误漏斗 / toast 钩子补齐，三者拼出完整的用户故事。
 *
 * 开关由 App 同步（setDebugRecording）；未开启时零开销（直接 return，不发 IPC）。
 * 走统一 ipc 封装，记录本身的失败会进 ipc-error 漏斗且不影响业务。
 */
import { invoke } from "./ipc";

let recording = false;

export function setDebugRecording(active: boolean) {
  recording = active;
}

export function debugNote(event: string, source = "ui") {
  if (!recording) return;
  invoke("debug_note", { source, event }).catch(() => {});
}
