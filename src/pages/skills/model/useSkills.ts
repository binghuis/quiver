import { useCallback, useEffect, useState } from "react";
import { listSkills, type Skill } from "@/entities/skill";

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; skills: Skill[] }
  | { kind: "error"; message: string };

export function useSkills(projectDir?: string | null) {
  const [state, setState] = useState<State>({ kind: "idle" });

  const reload = useCallback(async () => {
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

  useEffect(() => {
    reload();
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

  return { state, reload, updateLocal, upsertLocal };
}
