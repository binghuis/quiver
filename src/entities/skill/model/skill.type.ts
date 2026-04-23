export type SkillScope = "user" | "project" | "plugin";

export type SkillEcosystem = "claude" | "codex" | "gemini";

export type Skill = {
  id: string;
  name: string;
  description: string;
  ecosystem: SkillEcosystem;
  scope: SkillScope;
  enabled: boolean;
  plugin: string | null;
  path: string;
  body: string;
  content_hash: string;
};

/**
 * 同一命名空间（同 ecosystem + 同 pluginId + 同 name）下出现多份物理 skill 时
 * 产生的冲突组。agent 调用 name 时无法区分，属于非法数据状态，需要用户 resolve。
 */
export type ConflictGroup = {
  key: string;
  ecosystem: SkillEcosystem;
  pluginId: string | null;
  name: string;
  skills: Skill[];
};
