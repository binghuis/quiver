import { invoke as rawInvoke } from "@tauri-apps/api/core";

export function invoke<T>(cmd: string, args?: Record<string, unknown>) {
  return rawInvoke<T>(cmd, args);
}
