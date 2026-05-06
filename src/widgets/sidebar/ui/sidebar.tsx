import { forwardRef, useMemo, useState } from "react";
import {
  Ban,
  ChevronRight,
  Library,
  ListFilter,
  Package,
  Wand2,
} from "lucide-react";
import { cn } from "@/lib/utils";
import {
  groupByMarketplace,
  logicalPrimaryScope,
  pluginAccent,
  toLogicalSkills,
  type Skill,
  type SkillScope,
} from "@/entities/skill";

export type SidebarFilter =
  | { kind: "all" }
  | { kind: "scope"; scope: SkillScope }
  | { kind: "marketplace"; marketplace: string }
  | { kind: "plugin"; plugin: string }
  | { kind: "disabled" };

type Props = {
  skills: Skill[];
  filter: SidebarFilter;
  onFilterChange: (f: SidebarFilter) => void;
};

export function Sidebar({ skills, filter, onFilterChange }: Props) {
  const marketplaces = useMemo(() => groupByMarketplace(skills), [skills]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggleCollapsed = (key: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });
  };

  // 计数以 logical skill 为单位，和主列表、filter 口径一致：
  // plugin 原生的同步副本不会再被当成独立"用户级 skill"去计数。冲突条目不计入。
  const { logicals } = useMemo(() => toLogicalSkills(skills), [skills]);
  // 「已禁用」口径 = Claude Code 实际不会加载的：文件名是 SKILL.md.disabled，
  // 或宿主插件在 settings.json 被 gate 掉。两条任一 hit 就算 disabled。
  const count = {
    all: logicals.length,
    user: logicals.filter((l) => logicalPrimaryScope(l) === "user").length,
    disabled: logicals.filter((l) =>
      l.presences.some((p) => !p.skill.enabled || !p.skill.plugin_enabled),
    ).length,
  };

  const isActive = (f: SidebarFilter): boolean => {
    if (filter.kind !== f.kind) return false;
    if (filter.kind === "scope" && f.kind === "scope")
      return filter.scope === f.scope;
    if (filter.kind === "marketplace" && f.kind === "marketplace")
      return filter.marketplace === f.marketplace;
    if (filter.kind === "plugin" && f.kind === "plugin")
      return filter.plugin === f.plugin;
    return true;
  };

  return (
    <aside className="flex h-full w-full min-w-0 flex-col overflow-hidden bg-background/40">
      <div className="flex-1 overflow-y-auto px-1.5 py-2">
        <Row
          icon={<Library size={14} />}
          label="全部"
          count={count.all}
          active={isActive({ kind: "all" })}
          onClick={() => onFilterChange({ kind: "all" })}
        />
        <Row
          icon={<Wand2 size={14} />}
          label="我的 Skill"
          count={count.user}
          active={isActive({ kind: "scope", scope: "user" })}
          onClick={() => onFilterChange({ kind: "scope", scope: "user" })}
        />
        <Row
          icon={<Ban size={14} />}
          label="已禁用"
          count={count.disabled}
          active={isActive({ kind: "disabled" })}
          onClick={() => onFilterChange({ kind: "disabled" })}
        />

        {marketplaces.map((m) => {
          const isCollapsed = collapsed.has(m.marketplace);
          return (
            <div key={m.marketplace} className="mt-3">
              <SectionLabel
                count={m.total}
                collapsed={isCollapsed}
                active={isActive({
                  kind: "marketplace",
                  marketplace: m.marketplace,
                })}
                onClick={() => toggleCollapsed(m.marketplace)}
                action={{
                  label: `只看 ${m.marketplace}`,
                  onClick: () =>
                    onFilterChange({
                      kind: "marketplace",
                      marketplace: m.marketplace,
                    }),
                }}
              >
                {m.marketplace}
              </SectionLabel>
              {!isCollapsed &&
                m.plugins.map((p) => (
                  <Row
                    key={p.plugin}
                    icon={<Package size={14} />}
                    accentColor={pluginAccent(p.plugin)}
                    label={p.name}
                    count={p.total}
                    // 整体置灰条件：宿主插件被 gate（settings.json 里 false）
                    // 或所有 skill 都被文件级禁用了。两种情形 Claude Code 都
                    // 不会加载它。
                    dim={!p.plugin_enabled || p.enabled === 0}
                    active={isActive({
                      kind: "plugin",
                      plugin: p.plugin,
                    })}
                    onClick={() =>
                      onFilterChange({
                        kind: "plugin",
                        plugin: p.plugin,
                      })
                    }
                  />
                ))}
            </div>
          );
        })}
      </div>
    </aside>
  );
}

function SectionLabel({
  children,
  className,
  count,
  collapsed,
  active,
  onClick,
  action,
}: {
  children: React.ReactNode;
  className?: string;
  count?: number;
  collapsed?: boolean;
  active?: boolean;
  onClick?: () => void;
  action?: { label: string; onClick: () => void };
}) {
  const interactive = !!onClick;
  return (
    <div
      role={interactive ? "button" : undefined}
      tabIndex={interactive ? 0 : undefined}
      onClick={onClick}
      onKeyDown={
        interactive
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onClick?.();
              }
            }
          : undefined
      }
      className={cn(
        "group/section flex items-center gap-1.5 rounded-md px-2 pb-1 pt-1 text-[11px] font-medium uppercase tracking-wider transition-colors duration-100",
        active
          ? "bg-muted/60 text-foreground"
          : collapsed
            ? "text-muted-foreground/45"
            : "text-muted-foreground/70",
        interactive && "cursor-pointer select-none",
        interactive && !active && "hover:text-foreground",
        className,
      )}
    >
      <ChevronRight
        size={10}
        className={cn(
          "shrink-0 transition-transform duration-150",
          active ? "text-foreground" : "text-muted-foreground/45",
          !collapsed && "rotate-90",
        )}
      />
      <span className="flex-1 truncate">{children}</span>
      <div className="relative flex items-center">
        {count !== undefined && (
          <span
            className={cn(
              "tabular-nums transition-opacity duration-100",
              action && active && "opacity-0",
              action && !active && "group-hover/section:opacity-0",
              active
                ? "text-foreground"
                : collapsed
                  ? "text-muted-foreground/40"
                  : "text-muted-foreground/60",
            )}
          >
            {count}
          </span>
        )}
        {action && (
          <button
            type="button"
            aria-label={action.label}
            onClick={(e) => {
              e.stopPropagation();
              action.onClick();
            }}
            className={cn(
              "absolute inset-y-0 right-0 flex items-center transition-opacity duration-100",
              active
                ? "text-foreground opacity-100"
                : "text-muted-foreground/55 opacity-0 hover:text-foreground group-hover/section:opacity-100 focus-visible:opacity-100",
            )}
          >
            <ListFilter size={11} />
          </button>
        )}
      </div>
    </div>
  );
}

type RowProps = {
  icon?: React.ReactNode;
  label: string;
  count?: number;
  active?: boolean;
  onClick?: () => void;
  accentColor?: string;
  dim?: boolean;
} & Omit<React.HTMLAttributes<HTMLDivElement>, "onClick">;

const Row = forwardRef<HTMLDivElement, RowProps>(function Row(
  { icon, label, count, active, onClick, accentColor, dim, className, ...rest },
  ref,
) {
  return (
    <div
      ref={ref}
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick?.();
        }
      }}
      className={cn(
        "flex h-7 w-full cursor-pointer items-center gap-1.5 rounded-md px-2 text-[13px] transition-colors duration-100",
        active
          ? "bg-muted text-foreground"
          : "text-foreground/85 hover:bg-muted/50",
        dim && "opacity-55",
        className,
      )}
      {...rest}
    >
      {icon && (
        <span
          className={cn(
            "flex size-3.5 items-center justify-center",
            active ? "text-foreground" : "text-muted-foreground",
          )}
          style={accentColor ? { color: accentColor } : undefined}
        >
          {icon}
        </span>
      )}
      <span className="flex-1 truncate text-left">{label}</span>
      {count !== undefined && (
        <span
          className={cn(
            "text-xs tabular-nums",
            active ? "text-foreground" : "text-muted-foreground/70",
          )}
        >
          {count}
        </span>
      )}
    </div>
  );
});

