import { useState } from "react";
import { Search, Sparkles, FolderOpen, Settings } from "lucide-react";
import { cn } from "@/lib/utils";

type Skill = {
  id: string;
  name: string;
  desc: string;
  scope: "user" | "project";
  enabled: boolean;
};

const SKILLS: Skill[] = [
  {
    id: "1",
    name: "raycast-design",
    desc: "Raycast 风格设计系统 — 写任何 UI 组件时必须遵守",
    scope: "project",
    enabled: true,
  },
  {
    id: "2",
    name: "react-patterns",
    desc: "React/TSX 最佳实践：key 重置、禁 any、联合类型消灭非法状态",
    scope: "user",
    enabled: true,
  },
  {
    id: "3",
    name: "fsd",
    desc: "Feature-Sliced Design 层级与导入规则",
    scope: "user",
    enabled: false,
  },
];

function App() {
  const [selectedId, setSelectedId] = useState("1");
  const [query, setQuery] = useState("");

  const filtered = SKILLS.filter((s) =>
    s.name.toLowerCase().includes(query.toLowerCase()),
  );
  const active = SKILLS.find((s) => s.id === selectedId) ?? SKILLS[0];

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside className="flex w-[240px] shrink-0 flex-col gap-4 border-r bg-background/40 px-2 py-3">
          <div className="flex items-center gap-2 px-2">
            <div className="size-2 rounded-sm bg-primary" />
            <span className="text-[15px] font-semibold">Quiver</span>
          </div>

          <nav className="space-y-0.5">
            <SidebarItem icon={<Sparkles size={14} />} label="全部 Skills" count={SKILLS.length} active />
            <SidebarItem icon={<FolderOpen size={14} />} label="项目" count={SKILLS.filter(s => s.scope === "project").length} />
            <SidebarItem icon={<Settings size={14} />} label="已禁用" count={SKILLS.filter(s => !s.enabled).length} />
          </nav>
        </aside>

        {/* List */}
        <section className="flex min-w-[280px] flex-1 flex-col border-r">
          <header className="flex h-10 items-center gap-2 border-b px-3">
            <Search size={14} className="text-muted-foreground" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索 skill…"
              className="flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
            />
            <kbd>⌘F</kbd>
          </header>

          <div className="flex-1 overflow-y-auto p-1">
            {filtered.map((s) => (
              <button
                key={s.id}
                onClick={() => setSelectedId(s.id)}
                className={cn(
                  "relative flex h-10 w-full flex-col justify-center rounded-md px-2 text-left transition-colors duration-100",
                  s.id === selectedId ? "bg-muted" : "hover:bg-muted/50",
                  !s.enabled && "opacity-50",
                )}
              >
                {s.id === selectedId && (
                  <span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-primary" />
                )}
                <div className="font-medium">{s.name}</div>
                <div className="truncate text-xs text-muted-foreground">{s.desc}</div>
              </button>
            ))}
          </div>
        </section>

        {/* Detail */}
        <section className="flex min-w-[400px] flex-1 flex-col">
          <header className="flex h-10 items-center justify-between border-b px-4">
            <h1 className="text-[15px] font-semibold">{active.name}</h1>
            <span className="text-xs text-muted-foreground">
              {active.scope === "project" ? "项目级" : "用户级"}
            </span>
          </header>

          <div className="flex-1 overflow-y-auto p-4">
            <p className="text-muted-foreground">{active.desc}</p>
            <pre className="mt-4 rounded-md border bg-card p-3 font-mono text-xs leading-relaxed">
{`---
name: ${active.name}
description: ${active.desc}
---

# ${active.name}

(skill body preview goes here)`}
            </pre>
          </div>
        </section>
      </div>

      {/* Action Bar */}
      <footer className="flex h-9 items-center gap-4 border-t px-3 text-xs text-muted-foreground">
        <ActionHint k="↵" label="编辑" />
        <ActionHint k="⌘N" label="新建" />
        <ActionHint k="⌘E" label="启用/禁用" />
        <ActionHint k="⌘D" label="删除" />
        <div className="ml-auto flex items-center gap-4">
          <ActionHint k="⌘K" label="命令面板" />
        </div>
      </footer>
    </div>
  );
}

function SidebarItem({
  icon,
  label,
  count,
  active,
}: {
  icon: React.ReactNode;
  label: string;
  count?: number;
  active?: boolean;
}) {
  return (
    <button
      className={cn(
        "flex h-8 w-full items-center gap-2 rounded-md px-2 transition-colors duration-100",
        active ? "bg-muted" : "hover:bg-muted/50",
      )}
    >
      <span className="text-muted-foreground">{icon}</span>
      <span>{label}</span>
      {count !== undefined && (
        <span className="ml-auto text-xs text-muted-foreground">{count}</span>
      )}
    </button>
  );
}

function ActionHint({ k, label }: { k: string; label: string }) {
  return (
    <div className="flex items-center gap-1.5">
      <kbd>{k}</kbd>
      <span>{label}</span>
    </div>
  );
}

export default App;
