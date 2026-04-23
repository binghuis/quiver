import { AlertTriangle, FolderOpen, Search } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  ECOSYSTEMS,
  EcosystemIcon,
  ecosystemLabel,
  revealInFinder,
  type ConflictGroup,
  type LogicalSkill,
  type SkillEcosystem,
} from "@/entities/skill";

type Props = {
  logicals: LogicalSkill[];
  conflicts: ConflictGroup[];
  selectedId: string | null;
  onSelect: (id: string) => void;
};

export function SkillList({ logicals, conflicts, selectedId, onSelect }: Props) {
  const isEmpty = logicals.length === 0 && conflicts.length === 0;
  return (
    <section className="flex h-full w-full min-w-0 flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto p-1.5">
        {isEmpty ? (
          <div className="mx-2 mt-8 flex flex-col items-center gap-1.5 rounded-md border border-dashed py-10 text-[12px] text-muted-foreground">
            <Search size={16} className="opacity-60" />
            没有匹配的 skill
          </div>
        ) : (
          <>
            {conflicts.map((c) => (
              <ConflictRow key={c.key} conflict={c} />
            ))}
            {logicals.map((l) => {
              const primary = l.presences[0].skill;
              const isSelected = l.presences.some(
                (p) => p.skill.id === selectedId,
              );
              return (
                <SkillRow
                  key={l.key}
                  logical={l}
                  selected={isSelected}
                  onSelect={() => onSelect(primary.id)}
                />
              );
            })}
          </>
        )}
      </div>
    </section>
  );
}

function SkillRow({
  logical,
  selected,
  onSelect,
}: {
  logical: LogicalSkill;
  selected: boolean;
  onSelect: () => void;
}) {
  const primary = logical.presences[0].skill;
  const present = new Set(logical.presences.map((p) => p.ecosystem));

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      className={cn(
        "relative flex w-full cursor-pointer items-center gap-2.5 rounded-md px-2 py-2 text-left transition-colors duration-100",
        selected ? "bg-muted" : "hover:bg-muted/50",
        !primary.enabled && "opacity-55",
      )}
    >
      {selected && (
        <span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-primary" />
      )}

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="min-w-0 flex-1 truncate text-[13px] font-medium leading-5">
            {logical.name}
          </span>
          <EcosystemBadgeStack present={present} drifted={logical.drifted} />
        </div>
        <div className="line-clamp-2 text-[11.5px] leading-4 text-muted-foreground">
          {logical.description || "（无描述）"}
        </div>
      </div>
    </div>
  );
}

function EcosystemBadgeStack({
  present,
  drifted,
}: {
  present: Set<SkillEcosystem>;
  drifted: boolean;
}) {
  const installed = ECOSYSTEMS.filter((eco) => present.has(eco));
  return (
    <div className="flex shrink-0 items-center">
      {installed.map((eco, i) => (
        <span
          key={eco}
          title={ecosystemLabel(eco)}
          className={cn(
            "flex size-4 items-center justify-center rounded-full bg-card ring-1 ring-border",
            i > 0 && "-ml-1",
          )}
          style={{ zIndex: installed.length - i }}
        >
          <EcosystemIcon eco={eco} className="size-3" aria-hidden />
        </span>
      ))}
      {drifted && (
        <span
          title="各生态版本内容已漂移"
          className="ml-1 text-[10px] font-bold leading-none text-amber-500"
        >
          !
        </span>
      )}
    </div>
  );
}

function ConflictRow({ conflict }: { conflict: ConflictGroup }) {
  const scopeLabel = conflict.pluginId
    ? `插件 ${conflict.pluginId}`
    : "用户级";

  return (
    <div className="mb-1 rounded-md border border-destructive/50 bg-destructive/8 px-2.5 py-2">
      <div className="flex items-center gap-2">
        <AlertTriangle
          size={13}
          className="shrink-0 text-destructive"
          aria-hidden
        />
        <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium text-destructive">
          同名冲突：{conflict.name}
        </span>
        <span className="shrink-0 text-[10.5px] text-destructive/80">
          {ecosystemLabel(conflict.ecosystem)} · {scopeLabel}
        </span>
      </div>
      <p className="mt-1 text-[11px] leading-4 text-destructive/85">
        同一命名空间下存在 {conflict.skills.length} 份同名
        skill，agent 调用时无法区分。请删除或重命名多余副本。
      </p>
      <ul className="mt-1.5 space-y-0.5">
        {conflict.skills.map((s) => (
          <li
            key={s.id}
            className="flex items-center gap-1.5 text-[11px] text-muted-foreground"
          >
            <span className="min-w-0 flex-1 truncate font-mono">{s.path}</span>
            <button
              type="button"
              title="在 Finder 中显示"
              onClick={() => {
                void revealInFinder(s.path).catch(() => undefined);
              }}
              className="flex size-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
            >
              <FolderOpen size={11} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
