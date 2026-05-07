import { useCallback, useEffect, useRef, useState } from "react";
import { listSkills, type Skill } from "@/entities/skill";

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; skills: Skill[] }
  | { kind: "error"; message: string };

export function useSkills(projectDir?: string | null) {
  const [state, setState] = useState<State>({ kind: "idle" });
  // 写盘 mutation 进行中时压住所有 reload。delete_marketplace 这类 async 命令
  // 在 Rust 端搬磁盘要几百 ms ~ 几秒，期间任何 onFocus / 手动 rescan / HMR
  // 重渲染触发的 reload，都会扫到「半成品状态」（marketplace 还在 / installed_plugins.json
  // 还没改完），把刚被删掉的 plugin skill 重新捞回 UI。ref 而非 state——闸门
  // 判定要立刻读到最新值，不能等下一个渲染。
  const mutationDepth = useRef(0);

  const reload = useCallback(async () => {
    if (mutationDepth.current > 0) return;
    // 已经有数据时保留旧列表，等新数据到位再一次性覆盖——否则 refresh 类操作
    // 会让整张列表先闪成空、再填回来，视觉上像"卡一下又闪回"。
    setState((prev) => (prev.kind === "ready" ? prev : { kind: "loading" }));
    try {
      const skills = await listSkills(projectDir ?? null);
      setState({ kind: "ready", skills });
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, [projectDir]);

  // 用法：await mutate(async () => { 写盘 + 后续不变量整理 })
  // 内层任何 reload 调用都被忽略；最外层 mutate 退栈时统一拉一次最终结果。
  // 嵌套 mutate 安全（depth 计数），fn throw 也保证 finally 解锁 + reload。
  const mutate = useCallback(
    async <T,>(fn: () => Promise<T>): Promise<T> => {
      mutationDepth.current++;
      try {
        return await fn();
      } finally {
        mutationDepth.current--;
        if (mutationDepth.current === 0) await reload();
      }
    },
    [reload],
  );

  useEffect(() => {
    void reload();
  }, [reload]);

  // 窗口重新获得焦点时刷新一次。用户从 Finder 做外部改动（垃圾桶还原、手动
  // 新建 skill 目录、git pull marketplace 等）后切回 app，不需要手动点刷新。
  useEffect(() => {
    const onFocus = () => {
      void reload();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [reload]);

  const updateLocal = useCallback((id: string, patch: Partial<Skill>) => {
    setState((s) =>
      s.kind === "ready"
        ? {
            kind: "ready",
            skills: s.skills.map((sk) => (sk.id === id ? { ...sk, ...patch } : sk)),
          }
        : s,
    );
  }, []);

  const upsertLocal = useCallback((skill: Skill) => {
    setState((s) =>
      s.kind === "ready"
        ? {
            kind: "ready",
            skills: [skill, ...s.skills.filter((sk) => sk.id !== skill.id)],
          }
        : { kind: "ready", skills: [skill] },
    );
  }, []);

  return { state, reload, mutate, updateLocal, upsertLocal };
}
