export type {
  ConflictGroup,
  Skill,
  SkillEcosystem,
  SkillScope,
} from "./model/skill.type";
export {
  listSkills,
  toggleSkills,
  importFromGithub,
  revealInFinder,
  deleteMarketplace,
  deleteSkillPresence,
  syncSkillToEcosystem,
  refreshClaudeMarketplace,
} from "./api/skills.api";
export type { ImportResult } from "./api/skills.api";
export {
  parsePluginId,
  groupByMarketplace,
  scopeLabel,
  scopeColor,
  pluginAccent,
  ECOSYSTEMS,
  ecosystemLabel,
  ecosystemColor,
  toLogicalSkills,
  logicalPrimaryScope,
} from "./lib/grouping";
export type {
  MarketplaceNode,
  PluginNode,
  LogicalSkill,
  EcosystemPresence,
} from "./lib/grouping";
export { EcosystemIcon } from "./ui/ecosystem-icon";
