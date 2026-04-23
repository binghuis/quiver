import type {
  ConflictGroup,
  Skill,
  SkillEcosystem,
  SkillScope,
} from "../model/skill.type";

export type PluginNode = {
  plugin: string;
  name: string;
  marketplace: string;
  total: number;
  enabled: number;
};

export type MarketplaceNode = {
  marketplace: string;
  total: number;
  enabled: number;
  plugins: PluginNode[];
};

export function parsePluginId(
  raw: string | null | undefined,
): { name: string; marketplace: string } | null {
  if (!raw) return null;
  const idx = raw.lastIndexOf("@");
  if (idx < 0) return { name: raw, marketplace: "unknown" };
  return { name: raw.slice(0, idx), marketplace: raw.slice(idx + 1) };
}

export function groupByMarketplace(skills: Skill[]): MarketplaceNode[] {
  const byMarket = new Map<string, Map<string, PluginNode>>();
  for (const s of skills) {
    if (s.scope !== "plugin") continue;
    const parsed = parsePluginId(s.plugin);
    if (!parsed) continue;
    const { name, marketplace } = parsed;
    const plugins = byMarket.get(marketplace) ?? new Map();
    const key = `${name}@${marketplace}`;
    const node = plugins.get(key) ?? {
      plugin: key,
      name,
      marketplace,
      total: 0,
      enabled: 0,
    };
    node.total += 1;
    if (s.enabled) node.enabled += 1;
    plugins.set(key, node);
    byMarket.set(marketplace, plugins);
  }
  return Array.from(byMarket.entries())
    .map(([marketplace, plugins]) => {
      const list = Array.from(plugins.values()).sort((a, b) =>
        a.name.localeCompare(b.name),
      );
      const total = list.reduce((n, p) => n + p.total, 0);
      const enabled = list.reduce((n, p) => n + p.enabled, 0);
      return { marketplace, plugins: list, total, enabled };
    })
    .sort((a, b) => a.marketplace.localeCompare(b.marketplace));
}

export function scopeLabel(scope: SkillScope): string {
  switch (scope) {
    case "user":
      return "用户级";
    case "project":
      return "项目级";
    case "plugin":
      return "插件";
  }
}

export function scopeColor(scope: SkillScope): string {
  switch (scope) {
    case "user":
      return "#ff6363";
    case "project":
      return "#06b6d4";
    case "plugin":
      return "#f59e0b";
  }
}

const PLUGIN_PALETTE = [
  "#f59e0b", // amber
  "#10b981", // emerald
  "#06b6d4", // cyan
  "#6366f1", // indigo
  "#a855f7", // purple
  "#ec4899", // pink
  "#14b8a6", // teal
  "#f97316", // orange
];

export function pluginAccent(plugin: string | null | undefined): string {
  if (!plugin) return scopeColor("plugin");
  let h = 0;
  for (const c of plugin) h = (h * 31 + c.charCodeAt(0)) | 0;
  return PLUGIN_PALETTE[Math.abs(h) % PLUGIN_PALETTE.length];
}

export const ECOSYSTEMS: readonly SkillEcosystem[] = [
  "claude",
  "codex",
  "gemini",
] as const;

export function ecosystemLabel(eco: SkillEcosystem): string {
  switch (eco) {
    case "claude":
      return "Claude";
    case "codex":
      return "Codex";
    case "gemini":
      return "Gemini";
  }
}

export function ecosystemColor(eco: SkillEcosystem): string {
  switch (eco) {
    case "claude":
      return "#d97757"; // Anthropic clay orange
    case "codex":
      return "#10a37f"; // OpenAI green
    case "gemini":
      return "#4285f4"; // Google blue
  }
}

export type EcosystemPresence = {
  ecosystem: SkillEcosystem;
  skill: Skill;
};

/**
 * 一条逻辑 skill 的「主来源」：只要有任一份 plugin presence，就算 plugin 的，
 * 否则归为 user。项目维度当前不处理。
 */
export function logicalPrimaryScope(l: LogicalSkill): "user" | "plugin" {
  return l.presences.some((p) => p.skill.scope === "plugin") ? "plugin" : "user";
}

export type LogicalSkill = {
  /** React key / 稳定标识；插件系为 `plugin\x00name`，独立用户系为 `\x00name` */
  key: string;
  /** UI 显示名 */
  name: string;
  description: string;
  /** 按生态聚合后的物理 skill（每个生态最多一份，多份算冲突） */
  presences: EcosystemPresence[];
  /** 同 logical 内存在 hash 不一致的 presence */
  drifted: boolean;
};

const KEY_SEP = "\x00";

function makeKey(pluginId: string | null, name: string): string {
  return `${pluginId ?? ""}${KEY_SEP}${name}`;
}

/**
 * 把物理 skill 列表聚合为逻辑 skill + 冲突组。
 *
 * 身份模型（血缘优先，而非仅按 name 合并）：
 *   1. 按 (pluginId, name) 分桶。插件原生 skill（pluginId 非空）单独成桶；
 *      user/project-scope 全部 pluginId=null，按 name 合到独立用户桶。
 *   2. 每条插件系 logical = 原生 plugin skill 们 + 所有 hash 匹配的
 *      user/project 派生副本（sync 整目录拷贝，刚派生时 hash 必然一致；
 *      用户改过就漂移成独立生命，留在独立用户桶）。
 *   3. 独立用户系 logical = 剩下的 user/project skill 按 name 合并。
 *   4. 同 logical 内同 ecosystem 出现多份 → 冲突（agent 无法区分）。
 *
 *   这样「插件派生的 user 副本」跟着插件走（整插件删除会级联清它），
 *   「用户手建的同名 skill」保留独立身份；手改漂移的派生副本自动独立。
 */
export function toLogicalSkills(skills: Skill[]): {
  logicals: LogicalSkill[];
  conflicts: ConflictGroup[];
} {
  const pluginBuckets = new Map<string, Skill[]>();
  const userByName = new Map<string, Skill[]>();
  for (const s of skills) {
    const name = s.name || s.id;
    if (s.plugin) push(pluginBuckets, makeKey(s.plugin, name), s);
    else push(userByName, name, s);
  }

  const logicals: LogicalSkill[] = [];
  const conflicts: ConflictGroup[] = [];

  // 插件系：原生 plugin skill + 同名且 hash 匹配的 user/project 派生副本
  for (const [key, list] of pluginBuckets) {
    const name = key.slice(key.indexOf(KEY_SEP) + 1);
    const hashes = new Set(list.map((s) => s.content_hash));
    const userList = userByName.get(name) ?? [];
    const absorbed: Skill[] = [];
    const remaining: Skill[] = [];
    for (const u of userList) {
      (hashes.has(u.content_hash) ? absorbed : remaining).push(u);
    }
    if (remaining.length > 0) userByName.set(name, remaining);
    else userByName.delete(name);

    const { presences, conflictGroups } = buildPresences(key, [
      ...list,
      ...absorbed,
    ]);
    conflicts.push(...conflictGroups);
    if (presences.length > 0) logicals.push(makeLogical(key, presences));
  }

  // 剩下的独立用户/项目系：按 name 合并
  for (const [name, list] of userByName) {
    const key = makeKey(null, name);
    const { presences, conflictGroups } = buildPresences(key, list);
    conflicts.push(...conflictGroups);
    if (presences.length > 0) logicals.push(makeLogical(key, presences));
  }

  logicals.sort((a, b) => a.name.localeCompare(b.name));
  conflicts.sort((a, b) => a.name.localeCompare(b.name));
  return { logicals, conflicts };
}

function push<K>(map: Map<K, Skill[]>, key: K, s: Skill): void {
  const arr = map.get(key) ?? [];
  arr.push(s);
  map.set(key, arr);
}

function buildPresences(
  logicalKey: string,
  list: Skill[],
): { presences: EcosystemPresence[]; conflictGroups: ConflictGroup[] } {
  const byEco = new Map<SkillEcosystem, Skill[]>();
  for (const s of list) push(byEco, s.ecosystem, s);

  const presences: EcosystemPresence[] = [];
  const conflictGroups: ConflictGroup[] = [];
  for (const [eco, group] of byEco) {
    if (group.length > 1) {
      conflictGroups.push({
        key: `${logicalKey}${KEY_SEP}${eco}`,
        ecosystem: eco,
        pluginId: group[0].plugin,
        name: group[0].name || group[0].id,
        skills: group,
      });
      continue;
    }
    presences.push({ ecosystem: eco, skill: group[0] });
  }
  return { presences, conflictGroups };
}

function makeLogical(
  key: string,
  presences: EcosystemPresence[],
): LogicalSkill {
  const sorted = [...presences].sort((a, b) =>
    a.ecosystem.localeCompare(b.ecosystem),
  );
  const hashes = new Set(sorted.map((p) => p.skill.content_hash));
  return {
    key,
    name: sorted[0].skill.name,
    description: sorted[0].skill.description,
    presences: sorted,
    drifted: hashes.size > 1,
  };
}
