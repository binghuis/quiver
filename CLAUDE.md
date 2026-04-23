# Quiver — 给 Claude Code 的项目说明

Quiver 是一个 Tauri（Rust）+ React 的桌面 App，集中管理 Claude / Codex / Gemini 三个 agent 生态里的 **skill**（`SKILL.md` 形式的提示包）。FSD 目录约定（`shared / entities / features / widgets / pages`）。

本文只记录**必须先读，读完才能改得对**的内容——身份模型、跨生态同步语义、删除命令矩阵。其他可读源码反推的东西不写。

---

## 三层身份

Skill 的「是不是同一个」在三个层面定义，混淆任意两层都会引入 bug。

### L0 物理
`<ecosystem_root>/skills/<dir_name>/SKILL.md`（或 `.disabled` 后缀）。扫描器唯一真相。

### L1 后端 id
见 [src-tauri/src/skills.rs](src-tauri/src/skills.rs) 的 `id_for()`：
```
ecosystem:scope:plugin:dir_name
  e.g. "claude:user:my-log"
       "claude:plugin:review@foo-market:review"
       "codex:user:my-log"
```
- `toggle_skills` / `delete_skill_presence` / `sync_skill_to_ecosystem` 全部按此寻址
- `~/.claude/quiver/state.json` 的 `disabled` 集合也是这个 id
- 后端绝**不**按 `frontmatter name` 查找

### L2 前端 logical key
见 [src/entities/skill/lib/grouping.ts](src/entities/skill/lib/grouping.ts) 的 `toLogicalSkills()`：
```
Logical key = (pluginId | null, frontmatter_name || dir_name)
```

聚合规则：
- 同 key、不同 ecosystem → 同一 logical 的不同 `presence`
- 同 key、同 ecosystem 出现多份物理 skill → **冲突**（见下节），**不**进 logical
- 插件副本（`pluginId != null`）与它同步到 user 的副本（`pluginId == null`）是**两条独立 logical**——key 不同。它们的联动靠 `content_hash` 匹配（级联删除 / push-to-all）

---

## 同一命名空间同名 = 非法

> 一个 agent 用 `foo` 这个命令时无法区分两个都叫 `foo` 的 skill，这是用户数据的错误状态，不是 UI 要合法展示的东西。

`toLogicalSkills` 返回 `{ logicals, conflicts }`：
- 同 `(ecosystem, pluginId, name)` 出现 ≥2 份 → 汇入 `conflicts: ConflictGroup[]`
- UI 由 [src/widgets/skill-list/ui/skill-list.tsx](src/widgets/skill-list/ui/skill-list.tsx) 的 `ConflictRow` 渲染成红色警告条目，顶置在列表上方
- 冲突里的物理 skill **不**参与正常 logical，需要用户删除或重命名某一份 resolve

常见触发：用户在 `~/.claude/skills/a/` 和 `~/.claude/skills/b/` 分别写了 `SKILL.md`，但 frontmatter 都 `name: foo`。

---

## `content_hash` 的职责

FNV-1a 64-bit，非对抗模型（用户不会故意构造碰撞）。

| 用途 | 位置 |
|---|---|
| Drift 检测（同 logical 内 `hashes.size > 1`） | grouping.ts `makeLogical` |
| 级联删除识别（跨 logical 找插件的同步副本） | skills-page.tsx `derivedPresenceIdsFor` |
| Push-to-all 跳过已同步的生态 | skills-page.tsx `handlePushToAll` |

**不用于身份聚合**——身份只看 `(pluginId, name, ecosystem)`。hash 仅用来识别「这两份物理 skill 内容一样」，以决定该不该联动。

---

## 同步 / 删除 / 切换命令矩阵

| 命令 | 入参 | 作用 | 级联 |
|---|---|---|---|
| `toggle_skills(ids, enabled)` | 物理 id 列表 | 按文件名翻 `SKILL.md` ↔ `SKILL.md.disabled` + 写 state | 无，后端是纯执行器 |
| `sync_skill_to_ecosystem(sourceId, targetEco, overwrite)` | 源 id + 目标生态 | 整目录复制到 `<targetEco>/skills/<dir_name>/` | 继承源的 enabled 位到 state |
| `delete_marketplace(name)` | marketplace 名 | `plugins/marketplaces/<name>` → 系统回收站；清 `plugins/cache/<name>`；清 installed_plugins.json 里 `@<name>` 后缀的条目 | 前端 `derivedPresenceIdsFor` 按 hash 级联删 user 副本 |
| `delete_skill_presence(skillId)` | 单个物理 id | 该 skill 整目录 → 系统回收站 + 清 state | 无 |
| `refresh_claude_marketplace(name)` | marketplace 名 | `git pull --ff-only` + 清 cache | 失败会传回前端 `setActionError` |

插件自带的 skill 不允许单独删（`delete_skill_presence` 对 `plugin` scope 返回错误）；要删就删整个 marketplace。

Toggle 的身份边界：**前端决定 ids**。一次 toggle 用的 ids 必须是「当前 logical 的所有 presences」——身份模型保证 logical 不会混入无关 skill，因此批量翻是安全的。后端绝不自己聚合、绝不按 name 扫描。

---

## 已知行为

- **插件 skill 和它的 user 同步副本是两条 logical**。用户在 UI 看到两行——一行带 `@marketplace` 标签，一行是 user。删插件会级联删 hash 匹配的 user 副本，漂移（hash 不同）的不动。
- **`LogicalSkill.presences` 的 id 集合随 `content_hash` 变化**。外部编辑 SKILL.md 后必须 `reload()` 才能让前端重新分组。
- **分裂的边界情况**：如果用户 sync 后手改了其中一侧，新的 grouping 仍视为同一 logical（pluginId + name 一致）但 `drifted=true`。push-to-all 可一键拉齐。
- **project scope 当前不处理**——`listSkills` 的 `projectDir` 始终传 null；sidebar/grouping 里 project 分支是 dead path，未来引入再说。
- **同步的目标始终是 user scope**（不能同步到另一个插件里）。
- **FNV-1a 非对抗**：恶意构造哈希碰撞可能绕过级联删判定，但影响面只是本地 skill 目录，不跨安全边界。

---

## 不动后端身份的红线

`src-tauri/src/skills.rs` 的 `id_for` / `content_hash` / `toggle_skills` 纯执行器语义是稳定合约。前端重构身份分组时**不要**动后端——后端已经是正确的，所有跨 logical 的聚合判断留在前端做。
