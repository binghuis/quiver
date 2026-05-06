export type SkillScope = "user" | "project" | "plugin";

export type SkillEcosystem = "claude" | "codex" | "gemini";

export type Skill = {
  id: string;
  name: string;
  description: string;
  ecosystem: SkillEcosystem;
  scope: SkillScope;
  /** 文件级 enabled：SKILL.md 还是 SKILL.md.disabled */
  enabled: boolean;
  plugin: string | null;
  path: string;
  body: string;
  content_hash: string;
  /**
   * 仅对 Claude plugin scope 有意义：宿主插件在 `~/.claude/settings.json` 的
   * `enabledPlugins` 里是不是 true。false 时整个插件不加载，单 skill 的 enabled
   * 完全不起作用——UI 应当锁住该 skill 的 toggle，并整体显示为「插件已停用」。
   * user / project 以及非 Claude 生态：永远 true。
   */
  plugin_enabled: boolean;
  /**
   * SKILL.md frontmatter 里 `disable-model-invocation: true`。模型不会自己调，
   * 只能 `/skill-name` 手动触发——UI 上要标。
   */
  disable_model_invocation: boolean;
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
