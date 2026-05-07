import { useCallback, useEffect, useState } from "react";
import { getGitProxy, setGitProxy } from "../api/git-proxy";

type State =
  | { kind: "loading" }
  | { kind: "ready"; proxy: string }
  | { kind: "error"; message: string };

export function useGitProxy() {
  const [state, setState] = useState<State>({ kind: "loading" });

  const load = useCallback(async () => {
    setState({ kind: "loading" });
    try {
      const proxy = (await getGitProxy()) ?? "";
      setState({ kind: "ready", proxy });
    } catch (e) {
      setState({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const save = useCallback(async (proxy: string) => {
    const trimmed = proxy.trim();
    await setGitProxy(trimmed.length > 0 ? trimmed : null);
    setState({ kind: "ready", proxy: trimmed });
  }, []);

  return { state, save, reload: load };
}
