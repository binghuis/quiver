---
name: raycast-design
description: Raycast 风格设计系统 — 写、改 Quiver 任何 UI 组件（src/ 下的 .tsx、.css、Tailwind class）、设计新页面/交互、调整视觉（颜色、字号、间距、圆角、快捷键）时必须遵守。管理 Claude skill 的 Mac 工具 App，目标观感是「Raycast + Finder 三栏骨架 + shadcn 组件底」。
---

# Raycast 设计风格规范（Quiver）

本项目 UI 必须严格对齐 Raycast 的视觉和交互语言。**键盘优先、信息密度高、零装饰、工具感。**

---

## 1. 颜色

### 深色主题（默认）
| 用途 | Token | 值 |
|---|---|---|
| 窗口背景 | `--background` | `zinc-950` (#09090B) |
| 面板/卡片 | `--card` | `zinc-900` (#18181B) |
| 分隔线/边框 | `--border` | `zinc-800` (#27272A) |
| 次级背景（hover） | `--muted` | `zinc-800/50` |
| 正文文字 | `--foreground` | `zinc-100` (#F4F4F5) |
| 次级文字 | `--muted-foreground` | `zinc-400` (#A1A1AA) |
| 强调色（品牌） | `--primary` | `#FF6363`（Raycast 红） |
| 危险 | `--destructive` | `#F87171` |

### 浅色主题
镜像深色，底色 `zinc-50` / `white`，文字 `zinc-900`，边框 `zinc-200`。

### 强制约束
- **只允许一个强调色** `#FF6363`，不要引入蓝/绿/紫等辅助色
- **禁止渐变、禁止阴影**（除 `Dialog` / `Popover` 的 `shadow-2xl`）
- **禁止使用纯黑 #000 或纯白 #FFF**，永远用 zinc 系

---

## 2. 字体与排版

```css
font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
font-family-mono: "SF Mono", "JetBrains Mono", Menlo, monospace;
```

| 场景 | 字号 | 字重 | 行高 |
|---|---|---|---|
| 页面标题 | 15px | 600 | 1.4 |
| 区块标题 | 13px | 600 | 1.4 |
| 正文 / 列表项 | 13px | 400 | 1.5 |
| 次级文字 / 描述 | 12px | 400 | 1.5 |
| 快捷键提示 | 11px | 500 | 1 |
| 代码 / frontmatter | 12px mono | 400 | 1.5 |

**Tailwind 对应**：正文用 `text-[13px]`，次级 `text-xs`（12px），标题 `text-[15px] font-semibold`。**不要用 `text-sm` 作为正文**（默认 14px，偏大）。

---

## 3. 间距与尺寸

- **基础单位 4px**，所有 padding/margin/gap 必须是 4 的倍数
- **圆角统一 6px**（`--radius: 0.375rem`），按钮/输入框/卡片一致。**Dialog 可用 8px**，其他一律 6px
- **列表项高度 32px**（密集）或 40px（含副标题），不要超过 48px
- **侧栏宽度 240px**，可拖拽范围 `200-320px`
- **详情面板最小宽度 400px**

---

## 4. 布局骨架（Finder 三栏）

```
┌─────────────┬──────────────────────┬──────────────────────┐
│  Sidebar    │   List               │   Detail / Editor    │
│  240px      │   flex-1 min-w-280   │   flex-1 min-w-400   │
│  vibrancy   │                      │                      │
│             │                      │                      │
│             │                      │                      │
├─────────────┴──────────────────────┴──────────────────────┤
│  Action Bar  ↵ 编辑   ⌘E 启用/禁用   ⌘D 删除   ⌘N 新建    │  高度 36px
└────────────────────────────────────────────────────────────┘
```

- **Sidebar**：`NSVisualEffectView` vibrancy 半透明（Tauri `window-vibrancy` crate），分组标题 11px uppercase tracking-wider
- **List**：中间列，列表项左侧 16px icon，右侧可选快捷键提示
- **Detail**：右侧，`ScrollArea` 包裹，顶部固定 title bar
- **Action Bar**：底部常驻，展示当前上下文的快捷键，这是 Raycast 的灵魂

---

## 5. 交互规范（非协商项）

### 必须支持的快捷键
| 快捷键 | 行为 |
|---|---|
| `⌘K` | 打开全局命令面板（`Command` 组件） |
| `⌘N` | 新建 skill |
| `⌘F` | 聚焦搜索框 |
| `⌘E` | 切换启用/禁用 |
| `⌘D` | 删除（带确认） |
| `↵` | 打开/进入 |
| `Esc` | 关闭 Dialog / 返回上一级 |
| `↑↓` | 列表导航（必须支持键盘滚动） |
| `⌘,` | 打开设置 |

### 视觉反馈
- **hover**：背景变 `zinc-800/50`，**不要** transform/scale
- **selected**：背景 `zinc-800`，左侧 2px `--primary` 色 indicator
- **focus-visible**：1px `--primary` 外描边，**禁用 Tailwind 默认 ring**
- **过渡**：`transition-colors duration-100`，**不要** `duration-300` 及以上

### 快捷键提示渲染
使用 `<kbd>` 标签，样式：
```tsx
<kbd className="px-1.5 py-0.5 text-[11px] font-medium text-zinc-400 bg-zinc-800 border border-zinc-700 rounded">⌘K</kbd>
```

---

## 6. shadcn/ui 组件使用约定

### 必用组件
- `Command`（⌘K 面板）— 这是 Raycast 风的核心
- `Sidebar` + `ResizablePanel`
- `ScrollArea`（所有可滚动区域必须套，不要裸 `overflow-auto`）
- `Tooltip`（任何带快捷键的按钮都必须有 tooltip）
- `Sonner`（toast 反馈）

### 禁用/避免
- **禁用** `Card` 默认的 `shadow` 和大圆角 — 需覆盖为 `rounded-md border-0`
- **禁用** `Button` 默认的 `size="default"`，统一用 `size="sm"`（h-8）
- **避免** `Accordion`、`Carousel` — 这些是消费型产品组件，工具类 App 不需要
- **避免** `Badge` 的彩色 variant — 只用 `outline` 或 `secondary`

### Markdown 编辑器
- 编辑器：**CodeMirror 6**（非 Monaco，Monaco 在 Tauri webview 里偏重）
- 主题：深色用 `@uiw/codemirror-theme-github` dark，浅色用 light
- 预览：`react-markdown` + `rehype-highlight` + Tailwind `prose prose-invert prose-sm`

---

## 7. 图标

- **仅用 [Lucide React](https://lucide.dev/)**，统一 `size={14}`（小）或 `size={16}`（默认）
- 禁止混用 Heroicons、Feather、emoji 作图标
- 彩色图标仅限 skill 自带的 icon（用户内容），UI 图标必须 `currentColor`

---

## 8. 动画

**默认：没有动画。** 只在以下场景允许：
- `Dialog` / `Sheet` 进出：`150ms ease-out`
- 列表项 hover 背景切换：`100ms`
- 侧栏展开/收起：`200ms ease-in-out`

**禁止**：弹跳、spring、parallax、loading spinner 转圈（用 skeleton 或 pulsing dot 替代）。

---

## 9. 写代码时的自检清单

改完任何 UI，在提交前问自己：

- [ ] 是否只用了 zinc 色系 + 唯一强调色 `#FF6363`？
- [ ] 所有间距是否 4 的倍数？
- [ ] 圆角是否统一 6px？
- [ ] 正文字号是 13px 而非 14px？
- [ ] 新增的可点击元素是否绑定了快捷键？
- [ ] 是否加了 tooltip 显示快捷键？
- [ ] 是否能纯键盘操作（不用鼠标）？
- [ ] 有没有引入不必要的动画？
- [ ] 看起来像 Raycast，还是像 SaaS Dashboard？如果是后者，重做。

---

## 10. 参考

- [Raycast](https://www.raycast.com/) — 主要对标
- [Linear](https://linear.app/) — 深色极简的另一个范本
- **反例**：Notion、Figma、任何带渐变插图的 SaaS 落地页 — 不要向它们看齐
