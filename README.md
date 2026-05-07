# Quiver

跨 agent 生态的 **Skill 管理桌面 App**。把散落在 Claude / Codex / Gemini 三套目录里的 `SKILL.md`，集中到一个 Raycast 风格的三栏界面里管理。

## 它做什么

- **统一列表**：扫描 `~/.claude/skills`、`~/.codex/skills`、`~/.gemini/skills`，把同名同作用域的 skill 合并成一条逻辑记录展示，按生态、插件、作用域筛选。
- **跨生态同步**：把 user 作用域下的 skill 一键复制到另一个生态；多份内容漂移时支持「推送到全部」一键拉齐。
- **启停切换**：通过 `SKILL.md` ↔ `SKILL.md.disabled` 改名实现，状态写回 Quiver state 与 Claude 插件级开关（`enabledPlugins`）。
- **GitHub 导入**：粘一个 GitHub URL 直接 `git clone` 成本地 marketplace，里面的插件 skill 自动入库；后续可一键 `git pull` 刷新。
- **冲突提示**：同生态、同 plugin、同 frontmatter `name` 出现 ≥2 份时红色置顶，提示删除或重命名。
- **快捷工具**：详情页支持复制 `SKILL.md` 绝对路径；删除一律走系统回收站，可还原。

## 界面骨架

三栏布局，Raycast 视觉 + Finder 三栏交互 + shadcn 组件底：

- 左：sidebar（生态 / 插件 / 作用域 / 标签筛选）
- 中：skill 列表（含冲突告警条）
- 右：详情（frontmatter、`SKILL.md` 渲染、生态标签、操作按钮）

`⌘K` 唤起命令面板。

## 技术栈

- **桌面壳**：Tauri 2（Rust）
- **前端**：React 19 + Vite + TypeScript
- **UI**：Tailwind v4 + shadcn + Radix
- **架构**：FSD（`shared / entities / features / widgets / pages`）

## 开发

```bash
pnpm install
pnpm tauri dev      # 开发
pnpm tauri build    # 打包
```

## 文档

- [CLAUDE.md](CLAUDE.md) — 身份模型、同步/删除/切换命令矩阵、冲突语义
- [CHANGELOG.md](CHANGELOG.md) — 变更日志
