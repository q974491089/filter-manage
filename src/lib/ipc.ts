/**
 * 全项目统一的 IPC 调用入口。
 *
 * 为什么不用 @tauri-apps/api/core 的裸 invoke：命令失败时业务侧大多只 console.error
 * 甚至不 catch，「点了没反应」类问题就无从追溯。这里包一层——任何命令失败先上报
 * 进调试日志（后端在会话未开启时零开销），然后**原样抛出**，业务侧的 catch 行为完全不变。
 *
 * 新代码一律从这里 import { invoke }；转换函数（convertFileSrc）原样再导出。
 */
import { invoke as tauriInvoke, convertFileSrc } from "@tauri-apps/api/core";

export { convertFileSrc };

type InvokeArgs = Parameters<typeof tauriInvoke>[1];

/** 上报走裸 invoke + 吞掉自身错误：上报永远不能反过来制造新错误，也不能递归 */
function noteIpcError(cmd: string, err: unknown) {
  tauriInvoke("debug_note", {
    source: "ipc-error",
    event: `${cmd} 失败：${String(err)}`,
  }).catch(() => {});
}

export async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (err) {
    noteIpcError(cmd, err);
    throw err;
  }
}
