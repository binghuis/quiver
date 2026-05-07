# Quiver Roadmap

产品讨论的沉淀。所有需求——真、伪、待验证——都先列在这里，做不做后面再拍。

---

## 定位

跨 Claude / Codex / Gemini 的 skill 分发台。**只管不写**——正文交给 IDE，Quiver 负责装、分发、启停、查看、清理。

判断标准：**用户除了写 SKILL.md 正文之外的任何动作，都不应该再开终端、再 `cd ~/.claude/skills`、再手动 `git clone`**。如果还会，就是某条没补。

---

## 基础模型缺口（阻塞项）

这条不是新功能，是**当前数据模型的洞**。优先级在所有 feature 之前——基础不补，后面同步 / diff / 批量都建在沙上。

### 多文件 skill

当前隐含假设：`SKILL.md = 整条 skill`。但实际 skill 可以带 `interaction-skills/`、`domain-skills/`、`helpers.py` 这类 sibling 文件（参考 `~/Developer/browser-harness/`）。

需要解的硬问题：

- 跨生态同步要不要把整个目录搬过去？子目录里的二进制 / 大文件怎么办
- `delete_skill_presence` 当前删整目录，但用户心智可能是"只想关掉这条 skill"，要不要区分
- `content_hash` 当前只哈希 `SKILL.md` 本身，多文件 skill 的漂移识别完全失效
- 冲突检测的边界要不要扩展到 sibling 文件

---

## 已实现（baseline）

- 三生态扫描合并（按 `(plugin, name)` 聚合 logical）
- GitHub URL → marketplace clone
- 启停（`SKILL.md` ↔ `.disabled` + Claude 插件级开关）
- 跨生态同步（push to user scope）
- 内容漂移识别（content_hash）
- 冲突检测（同生态同名 ≥2 份红色置顶）
- 删除走系统回收站
- 命令面板（⌘K）
- 复制 SKILL.md 路径
- 写盘期间 reload gate（防扫到半成品）

---

## 真需求候选

按 lifecycle 分组。每条标了我的判断，**待用户裁决**。

### 装（Onboard）

| 功能 | 判断 | 说明 |
|---|---|---|
| **新建骨架 + "在 IDE 中打开"** | 真，高优 | 用户每次新建都要 mkdir + touch + 抄 frontmatter 模板。Quiver 不做这条永远是只读管理器。lifecycle 起点不能空 |
| 导出 / 导入 zip 备份 | 真 | 替代被砍掉的"跨机同步"，最轻的方式答"换机器怎么办" |

### 分发（Distribute）

| 功能 | 判断 | 说明 |
|---|---|---|
| **drift diff 三方视图** | 真，高优 | 当前漂移只显示"不一样"。三方 diff 是"推送到全部"敢按的前提 |

### 跑（Run & Tune）

| 功能 | 判断 | 说明 |
|---|---|---|
| **批量操作** | 真 | 多选启停 / 删除 / 同步。skill 量上去后必然要 |
| description 命中模拟器 | 真但要小心 | Quiver 真正的差异化。但 agent 调度是黑盒，模拟会有偏差，**MVP 别假装跑真匹配**——先做"列 description + 用户输入"让人肉判断就够。否则会被骂"不准" |
| AI 优化 description | 真，低优 | 用户写完后改的频率不高。但是 IDE 不会替它做的事——保留 |

### 维护（Maintain）

| 功能 | 判断 | 说明 |
|---|---|---|
| **外部改动红条**（IDE 改名导致冲突立刻飘红） | 真 | onFocus reload 已有，差冲突态的红条提示。低成本高回报 |
| **frontmatter lint** | 真 | description 太短/太长 / name 不合法 / type 错。写 skill 的常见坑，UI 直接告警是 Quiver 能干的事 |
| **"在 IDE / Finder 中打开"按钮** | 真 | 复制路径已有但要粘一遍，按钮一键发起省一步 |
| 引用断链检测（`@~/path` 失效） | 真但低优 | `@` 引用实际占比可能 <5%。先看数据再做 |

### 入口

| 功能 | 判断 | 说明 |
|---|---|---|
| 全文搜索（不止 name，包括正文 / description） | 真 | skill 量上去后 name 搜索不够 |
| 首次打开 onboarding | 真 | scope / plugin / marketplace / drift 概念新用户会懵。5 步导览 |

---

## 伪需求（已建议砍）

砍的理由也记下，免得以后有人想反提的时候忘了当时为什么不做。

| 功能 | 砍的理由 |
|---|---|
| 内置 Markdown 编辑器 | 用户在 Cursor / VSCode + AI 写正文比 Quiver 自建的永远顺。Quiver 干这事永远是个差版本 |
| 拖文件吸入单 SKILL.md | 低频。GitHub 装已覆盖 99% 来源；剩下的"别人发我一个 SKILL.md"几乎没场景 |
| 跨机器同步（iCloud / Git remote） | 重复造 Dropbox。Git 是用户已知方案，iCloud 是系统方案。Quiver 跑同步引擎不值——降级为"导出 zip 备份" |
| 反向自动推 GitHub marketplace | 写 skill 的都是用 git 的开发者，自己 `git init` 推上去比 Quiver 包一层更顺 |
| 导入历史血缘（每条 skill 记录从哪来） | 用户基本不会回查。包管理器思维硬塞 |
| 使用统计 hook agent 日志 | 三家日志格式不同 + 侵入用户终端，工程量大。"没命中过 ≠ 该删"结论也不准 |
| project scope 支持 | 项目内 skill 是 git 的事，不是 Quiver 的事。一个项目通常只针对一个 agent，"跨生态同步"在 project 上无场景 |

---

## 待验证（不确定真伪）

| 想法 | 待验证什么 |
|---|---|
| Enabled profile / presets | "写代码启 A/B/C，写文档启 D/E/F 一键切换"——可能是真需求也可能脑补。需看用户实际是否会主动想分场景。Raycast 的 quicklink group 是参考 |

---

## 定位决策（不是功能）

| 选择 | 含义 |
|---|---|
| menu bar 常驻 vs 普通窗口 app | 如果"分发台"是日常常驻定位，是否做成 menubar app + 主窗口？影响整体形态 |

---

## 推荐做的顺序

如果按上面所有"真，高优"做，建议顺序：

1. **多文件 skill 模型补全** —— 阻塞项
2. **新建骨架 + "在 IDE 打开"** —— 补 lifecycle 起点
3. **drift diff** —— 同步闭环
4. **frontmatter lint + 外部改动红条** —— 低成本高回报，可顺手
5. **批量操作**
6. **"在 IDE / Finder 中打开"按钮** —— 顺手
7. **命中模拟器（保守 MVP）**
8. 其他
