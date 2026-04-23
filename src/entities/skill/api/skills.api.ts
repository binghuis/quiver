import { invoke } from "@/shared/api/tauri";
import type { Skill, SkillEcosystem } from "../model/skill.type";

export function listSkills(projectDir?: string | null) {
  return invoke<Skill[]>("list_skills", { projectDir: projectDir ?? null });
}

/**
 * 批量切换。后端是纯执行器，**不做聚合** —— 前端要精确传入一次操作应该翻的
 * 所有副本 id（通常是同一条 LogicalSkill 的 presences）。这样后端不会超出前端
 * 的逻辑分组边界（如 splitNameGroup 下被拆开的两条同名 logical）。
 */
export function toggleSkills(ids: string[], enabled: boolean) {
  return invoke<void>("toggle_skills", { ids, enabled });
}

export type ImportResult =
  | { kind: "skill"; skill: Skill }
  | {
      kind: "marketplace";
      name: string;
      plugin_count: number;
      skill_count: number;
    };

export function importFromGithub(repoUrl: string) {
  return invoke<ImportResult>("import_from_github", { repoUrl });
}

export function revealInFinder(path: string) {
  return invoke<void>("reveal_in_finder", { path });
}

export function deleteMarketplace(name: string) {
  return invoke<void>("delete_marketplace", { name });
}

export function deleteSkillPresence(
  skillId: string,
  projectDir?: string | null,
) {
  return invoke<void>("delete_skill_presence", {
    skillId,
    projectDir: projectDir ?? null,
  });
}

export function syncSkillToEcosystem(
  sourceId: string,
  targetEcosystem: SkillEcosystem,
  overwrite: boolean = false,
) {
  return invoke<Skill>("sync_skill_to_ecosystem", {
    sourceId,
    targetEcosystem,
    overwrite,
  });
}

export function refreshClaudeMarketplace(name: string) {
  return invoke<void>("refresh_claude_marketplace", { name });
}
