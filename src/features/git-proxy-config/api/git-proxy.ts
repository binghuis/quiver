import { invoke } from "@/shared/api/tauri";

export const getGitProxy = () => invoke<string | null>("get_git_proxy");

export const setGitProxy = (proxy: string | null) =>
  invoke<void>("set_git_proxy", { proxy });
