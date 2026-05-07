import { useEffect, useMemo, useRef, useState } from "react";
import {
  CloudDownload,
  Package,
  Plus,
  RefreshCw,
  Settings,
  Store,
  Trash,
  X,
} from "lucide-react";
import { Sidebar, type SidebarFilter } from "@/widgets/sidebar";
import { SkillList } from "@/widgets/skill-list";
import { SkillDetail } from "@/widgets/skill-detail";
import {
  CommandPalette,
  type CommandPaletteGroup,
} from "@/widgets/command-palette";
import { SearchInput } from "@/features/skill-search";
import { Button } from "@/shared/ui/button";
import { Switch } from "@/shared/ui/switch";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/shared/ui/resizable";
import {
  deleteMarketplace,
  deleteSkillPresence,
  importFromGithub,
  logicalPrimaryScope,
  refreshClaudeMarketplace,
  syncSkillToEcosystem,
  togglePlugin,
  toggleSkills,
  toLogicalSkills,
  type ImportResult,
  type Skill,
  type SkillEcosystem,
} from "@/entities/skill";
import { useSkills } from "../model/useSkills";

export function SkillsPage() {
  const { state, updateLocal, upsertLocal, reload, mutate } = useSkills();
  const [filter, setFilter] = useState<SidebarFilter>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [cmdOpen, setCmdOpen] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  // 每条 logical skill 下「最近一次同步尝试失败」的 eco → 错误消息。
  // 下一次同步成功才清除；reload 不抹，否则用户失焦一次红点就没了，等于又回到静默。
  const [syncFailures, setSyncFailures] = useState<
    Record<string, Partial<Record<SkillEcosystem, string>>>
  >({});
  const searchRef = useRef<HTMLInputElement>(null);

  const skills = state.kind === "ready" ? state.skills : [];

  // 先把物理文件聚合成逻辑 skill + 冲突组，再对 logical 做过滤。
  // 这样即便过滤维度是单生态的（比如 marketplace 插件只在 Claude 扫描），
  // 被同步到 Codex/Gemini 的 presences 也不会被剪掉，行上的生态图标才是全的。
  const { logicals: allLogicals, conflicts } = useMemo(
    () => toLogicalSkills(skills),
    [skills],
  );

  const logicals = useMemo(() => {
    const byFilter = allLogicals.filter((l) => {
      switch (filter.kind) {
        case "all":
          return true;
        case "disabled":
          // Claude Code 实际不会加载的两种情形：文件名 SKILL.md.disabled，
          // 或宿主插件在 settings.json 被 gate 掉。
          return l.presences.some(
            (p) => !p.skill.enabled || !p.skill.plugin_enabled,
          );
        case "scope":
          // 按「主来源」判定，避免 plugin skill 的 user-scope 同步副本
          // 跑到「用户级」篮子里重复出现。
          return logicalPrimaryScope(l) === filter.scope;
        case "marketplace":
          return l.presences.some(
            (p) =>
              p.skill.scope === "plugin" &&
              p.skill.plugin?.endsWith("@" + filter.marketplace),
          );
        case "plugin":
          return l.presences.some(
            (p) =>
              p.skill.scope === "plugin" && p.skill.plugin === filter.plugin,
          );
      }
    });
    if (!query.trim()) return byFilter;
    const q = query.toLowerCase();
    return byFilter.filter(
      (l) =>
        l.name.toLowerCase().includes(q) ||
        l.description.toLowerCase().includes(q) ||
        l.presences.some((p) => p.skill.plugin?.toLowerCase().includes(q)),
    );
  }, [allLogicals, filter, query]);

  const visibleIds = useMemo(
    () => new Set(logicals.flatMap((l) => l.presences.map((p) => p.skill.id))),
    [logicals],
  );

  useEffect(() => {
    if (selectedId && visibleIds.has(selectedId)) return;
    setSelectedId(logicals[0]?.presences[0].skill.id ?? null);
  }, [logicals, visibleIds, selectedId]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      if (e.key === "k") {
        e.preventDefault();
        setCmdOpen((v) => !v);
      } else if (e.key === "f") {
        e.preventDefault();
        const el = searchRef.current;
        if (!el) return;
        el.focus();
        el.select();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const handleToggle = async (id: string, enabled: boolean) => {
    // 前端决定一次 toggle 应翻哪些副本——就是当前 logical 的所有 presences。
    // 身份模型保证 logical 不会跨 pluginId 混入无关 skill，后端纯执行 ids 列表。
    const logical = logicals.find((l) =>
      l.presences.some((p) => p.skill.id === id),
    );
    const ids = logical
      ? logical.presences.map((p) => p.skill.id)
      : [id];
    for (const pid of ids) updateLocal(pid, { enabled });

    try {
      await toggleSkills(ids, enabled);
    } catch (e) {
      setActionError(e instanceof Error ? e.message : String(e));
      await reload();
    }
  };

  const handleImported = async (result: ImportResult) => {
    if (result.kind === "skill") {
      upsertLocal(result.skill);
      setSelectedId(result.skill.id);
    } else {
      // Marketplace 整批装进来，local store 没法一条条 upsert；mutate 退栈时
      // 自动 reload 拉全部。importFromGithub 本身是慢命令（git clone），
      // 期间任何 onFocus / HMR 触发的 reload 都被压住，避免扫到半成品状态。
      setFilter({ kind: "marketplace", marketplace: result.name });
    }
  };

  /**
   * 删除 plugin / marketplace 时，派生到其他生态的 user-scope 同步副本也必须
   * 跟着走。后端 delete_plugin / delete_marketplace 只处理 ~/.claude/plugins
   * 下的文件，管不到 ~/.codex / ~/.gemini 里的同步副本 —— 不级联就会留下一批
   * 看起来是「用户级」的幽灵 skill。
   *
   * 只认 content_hash 跟插件副本一致的 non-plugin presence：
   *   - sync 是整目录拷贝，刚同步过去的副本 hash 与插件源一致 → 命中，级联删
   *   - 用户自建的同名独立 skill，hash 不同 → 不动，避免误伤
   *   - 用户手改过的漂移副本，hash 也不同 → 不动，保留用户编辑，用户自己决定
   */
  const derivedPresenceIdsFor = (
    matchPlugin: (plugin: string) => boolean,
  ): string[] => {
    const ids: string[] = [];
    for (const l of allLogicals) {
      const pluginHashes = new Set(
        l.presences
          .filter(
            (p) =>
              p.skill.scope === "plugin" &&
              !!p.skill.plugin &&
              matchPlugin(p.skill.plugin),
          )
          .map((p) => p.skill.content_hash),
      );
      if (pluginHashes.size === 0) continue;
      for (const p of l.presences) {
        if (p.skill.scope === "plugin") continue;
        if (pluginHashes.has(p.skill.content_hash)) {
          ids.push(p.skill.id);
        }
      }
    }
    return ids;
  };

  // 任一份副本清理失败就停下、抛出——插件本体先不删，避免把失败静默掉留下孤儿
  // user-scope 同步副本。调用方负责 setActionError 并 reload 刷新 ID。
  const cascadeDeletePresences = async (ids: string[]) => {
    if (ids.length === 0) return;
    const results = await Promise.allSettled(
      ids.map((id) => deleteSkillPresence(id)),
    );
    const failures = results
      .map((r, i) => (r.status === "rejected" ? { id: ids[i], reason: r.reason } : null))
      .filter((x): x is { id: string; reason: unknown } => x !== null);
    if (failures.length > 0) {
      const detail = failures
        .map((f) => `${f.id}: ${f.reason instanceof Error ? f.reason.message : String(f.reason)}`)
        .join("; ");
      throw new Error(`级联清理 ${failures.length}/${ids.length} 份同步副本失败：${detail}`);
    }
  };

  const handleDeleteMarketplace = async (marketplace: string) => {
    setActionError(null);
    await mutate(async () => {
      try {
        const suffix = "@" + marketplace;
        const derivedIds = derivedPresenceIdsFor((p) => p.endsWith(suffix));
        await cascadeDeletePresences(derivedIds);
        await deleteMarketplace(marketplace);
        if (
          (filter.kind === "marketplace" && filter.marketplace === marketplace) ||
          (filter.kind === "plugin" &&
            filter.plugin.endsWith("@" + marketplace))
        ) {
          setFilter({ kind: "all" });
        }
      } catch (e) {
        setActionError(e instanceof Error ? e.message : String(e));
      }
    });
  };

  // 插件总开关：写 `~/.claude/settings.json` 的 `enabledPlugins[plugin_id]`，
  // **不动 SKILL.md 文件**——这是 Claude Code 真正的插件级 gate，false 时整个
  // 插件（skill / commands / hooks / agents）都不加载。
  //
  // 乐观更新：把该插件下所有 plugin-scope skill 的 `plugin_enabled` 标志同步
  // 翻一下，UI 立即反映；失败再 reload 回滚。
  const handleTogglePlugin = async (plugin: string, enabled: boolean) => {
    const affected = skills.filter(
      (s) => s.scope === "plugin" && s.plugin === plugin,
    );
    if (affected.length === 0) return;
    for (const s of affected) updateLocal(s.id, { plugin_enabled: enabled });
    try {
      await togglePlugin(plugin, enabled);
    } catch (e) {
      setActionError(e instanceof Error ? e.message : String(e));
      await reload();
    }
  };

  const handleDeletePresence = async (presenceId: string) => {
    await mutate(async () => {
      try {
        await deleteSkillPresence(presenceId);
        if (selectedId === presenceId) setSelectedId(null);
      } catch (e) {
        setActionError(e instanceof Error ? e.message : String(e));
      }
    });
  };

  // 顶栏"删除"按钮语义：这条逻辑 skill 在所有生态里的 user/project 副本一把删。
  // plugin-scope 副本（同步来源是插件本体）跳过，交给"删除整个插件"处理。
  const handleDeleteLogical = async (anyPresenceId: string) => {
    setActionError(null);
    const logical = allLogicals.find((l) =>
      l.presences.some((p) => p.skill.id === anyPresenceId),
    );
    if (!logical) return;
    const ids = logical.presences
      .filter(
        (p) => p.skill.scope === "user" || p.skill.scope === "project",
      )
      .map((p) => p.skill.id);
    await mutate(async () => {
      try {
        await cascadeDeletePresences(ids);
        if (selectedId && ids.includes(selectedId)) setSelectedId(null);
      } catch (e) {
        setActionError(e instanceof Error ? e.message : String(e));
      }
    });
  };

  const handleRefreshClaudeMarketplace = async (name: string) => {
    await mutate(async () => {
      try {
        await refreshClaudeMarketplace(name);
      } catch (e) {
        setActionError(e instanceof Error ? e.message : String(e));
      }
    });
  };

  const selected = skills.find((s) => s.id === selectedId) ?? null;
  const selectedLogical = useMemo(
    () =>
      selectedId
        ? logicals.find((l) =>
            l.presences.some((p) => p.skill.id === selectedId),
          ) ?? null
        : null,
    [logicals, selectedId],
  );

  // 红点内存：失败写入，成功/一致则移除；空对象自动从外层 map 里删掉，避免泄漏。
  const recordSyncResult = (
    logicalKey: string,
    eco: SkillEcosystem,
    error: string | null,
  ) => {
    setSyncFailures((prev) => {
      const entry = { ...(prev[logicalKey] ?? {}) };
      if (error) entry[eco] = error;
      else delete entry[eco];
      const next = { ...prev };
      if (Object.keys(entry).length === 0) delete next[logicalKey];
      else next[logicalKey] = entry;
      return next;
    });
  };

  const handleSyncTo = async (
    sourceId: string,
    target: SkillEcosystem,
    overwrite = false,
  ) => {
    const logical = allLogicals.find((l) =>
      l.presences.some((p) => p.skill.id === sourceId),
    );
    try {
      const synced = await syncSkillToEcosystem(sourceId, target, overwrite);
      upsertLocal(synced);
      if (logical) recordSyncResult(logical.key, target, null);
    } catch (e) {
      if (logical) {
        recordSyncResult(
          logical.key,
          target,
          e instanceof Error ? e.message : String(e),
        );
      }
      throw e; // 保留给调用方（skill-detail 的 triggerSync）的原有 catch 链路
    }
  };

  /** Push the currently-viewed skill's content to all other ecosystems,
   *  overwriting any drifted copies. */
  const handlePushToAll = async (sourceId: string) => {
    const logical = logicals.find((l) =>
      l.presences.some((p) => p.skill.id === sourceId),
    );
    if (!logical) return;
    const source = logical.presences.find((p) => p.skill.id === sourceId);
    if (!source) return;

    for (const eco of (["claude", "codex", "gemini"] as const)) {
      if (eco === source.ecosystem) continue;
      const existing = logical.presences.find((p) => p.ecosystem === eco);
      if (existing && existing.skill.content_hash === source.skill.content_hash) {
        recordSyncResult(logical.key, eco, null); // 已一致，把历史红点也清了
        continue;
      }
      try {
        const synced = await syncSkillToEcosystem(sourceId, eco, true);
        upsertLocal(synced);
        recordSyncResult(logical.key, eco, null);
      } catch (e) {
        recordSyncResult(
          logical.key,
          eco,
          e instanceof Error ? e.message : String(e),
        );
      }
    }
  };

  const commandGroups: CommandPaletteGroup[] = [
    {
      heading: "Skill",
      items: [
        {
          id: "import",
          label: "从 GitHub 导入",
          icon: CloudDownload,
          input: {
            placeholder: "GitHub 仓库 URL，例如 https://github.com/binghuis/claude-plugins.git",
            onSubmit: async (url) => {
              await mutate(async () => {
                const result = await importFromGithub(url);
                await handleImported(result);
              });
            },
          },
        },
        {
          id: "new",
          label: "新建 skill",
          icon: Plus,
          hint: "即将推出",
          disabled: true,
          onRun: () => {},
        },
      ],
    },
    {
      heading: "应用",
      items: [
        {
          id: "settings",
          label: "打开设置",
          icon: Settings,
          hint: "即将推出",
          disabled: true,
          onRun: () => {},
        },
      ],
    },
  ];

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-1 overflow-hidden">
        <ResizablePanelGroup orientation="horizontal">
          <ResizablePanel defaultSize="47%" minSize="320px" maxSize="70%">
            <div className="flex h-full flex-col">
              <div className="flex h-11 shrink-0 items-center border-b">
                <div
                  data-tauri-drag-region
                  className="h-full w-19 shrink-0"
                />
                <div className="flex flex-1 items-center pr-2">
                  {filter.kind === "marketplace" ||
                  filter.kind === "plugin" ? (
                    <SelectionBanner
                      filter={filter}
                      skills={skills}
                      onClear={() => setFilter({ kind: "all" })}
                      onDeleteMarketplace={handleDeleteMarketplace}
                      onTogglePlugin={handleTogglePlugin}
                      onRefreshMarketplace={handleRefreshClaudeMarketplace}
                    />
                  ) : (
                    <SearchInput
                      ref={searchRef}
                      query={query}
                      onQueryChange={setQuery}
                    />
                  )}
                </div>
              </div>
              <ResizablePanelGroup orientation="horizontal" className="flex-1">
                <ResizablePanel defaultSize="40%" minSize="160px" maxSize="55%">
                  <Sidebar
                    skills={skills}
                    filter={filter}
                    onFilterChange={setFilter}
                  />
                </ResizablePanel>
                <ResizableHandle withHandle />
                <ResizablePanel defaultSize="60%" minSize="260px">
                  <SkillList
                    logicals={logicals}
                    conflicts={filter.kind === "all" ? conflicts : []}
                    selectedId={selectedId}
                    onSelect={setSelectedId}
                  />
                </ResizablePanel>
              </ResizablePanelGroup>
            </div>
          </ResizablePanel>
          <ResizableHandle withHandle className="[&>div]:mt-11" />
          <ResizablePanel defaultSize="53%" minSize="360px">
            <SkillDetail
              skill={selected}
              presences={
                selectedLogical?.presences ??
                (selected
                  ? [{ ecosystem: selected.ecosystem, skill: selected }]
                  : [])
              }
              drifted={selectedLogical?.drifted ?? false}
              syncFailures={
                selectedLogical ? syncFailures[selectedLogical.key] : undefined
              }
              onToggle={handleToggle}
              onSyncTo={handleSyncTo}
              onPushToAll={handlePushToAll}
              onDeleteLogical={handleDeleteLogical}
              onDeletePresence={handleDeletePresence}
            />
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>

      {state.kind === "error" && (
        <div className="shrink-0 border-t border-destructive/40 bg-destructive/10 px-3 py-1.5 text-[12px] text-destructive">
          加载失败：{state.message}
          <Button
            variant="link"
            size="sm"
            onClick={reload}
            className="ml-2 h-auto px-0 text-destructive"
          >
            重试
          </Button>
        </div>
      )}

      {actionError && (
        <div className="flex shrink-0 items-start gap-2 border-t border-destructive/40 bg-destructive/10 px-3 py-1.5 text-[12px] text-destructive">
          <span className="flex-1 whitespace-pre-wrap wrap-break-word">{actionError}</span>
          <Button
            variant="link"
            size="sm"
            onClick={() => setActionError(null)}
            className="h-auto shrink-0 px-0 text-destructive"
          >
            关闭
          </Button>
        </div>
      )}

      <CommandPalette
        open={cmdOpen}
        onOpenChange={setCmdOpen}
        groups={commandGroups}
      />
    </div>
  );
}

function SelectionBanner({
  filter,
  skills,
  onClear,
  onDeleteMarketplace,
  onTogglePlugin,
  onRefreshMarketplace,
}: {
  filter:
    | { kind: "marketplace"; marketplace: string }
    | { kind: "plugin"; plugin: string };
  skills: Skill[];
  onClear: () => void;
  onDeleteMarketplace: (name: string) => void;
  onTogglePlugin: (plugin: string, enabled: boolean) => void;
  onRefreshMarketplace: (name: string) => Promise<void>;
}) {
  const isMarket = filter.kind === "marketplace";
  const items = skills.filter((s) => {
    if (s.scope !== "plugin") return false;
    if (isMarket) return s.plugin?.endsWith("@" + filter.marketplace);
    return s.plugin === filter.plugin;
  });
  // 插件总开关 = settings.json `enabledPlugins[plugin_id]`。同插件下每个 skill
  // 的 plugin_enabled 必然一致（同一来源），取第一项即可。空列表时按禁用展示。
  const pluginEnabled =
    !isMarket && items.length > 0 && items[0].plugin_enabled;

  const title = isMarket ? filter.marketplace : parsePluginName(filter.plugin);
  const meta = isMarket
    ? `${new Set(items.map((s) => s.plugin!)).size} 个插件 · ${items.length} 个 skill`
    : `${items.length} 个 skill`;

  const [refreshing, setRefreshing] = useState(false);
  const handleRefresh = async () => {
    if (!isMarket || refreshing) return;
    setRefreshing(true);
    try {
      await onRefreshMarketplace(filter.marketplace);
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <>
      <div className="flex h-7 min-w-0 flex-1 items-center gap-1.5 rounded-md bg-muted/40 px-1.5">
        <Button
          variant="ghost"
          size="icon-xs"
          title="返回全部"
          onClick={onClear}
          className="size-5 shrink-0"
        >
          <X />
        </Button>
        <span className="shrink-0 text-muted-foreground">
          {isMarket ? <Store size={12} /> : <Package size={12} />}
        </span>
        <span className="min-w-0 truncate text-[12.5px] font-medium">
          {title}
        </span>
        <span className="min-w-0 truncate text-[11px] text-muted-foreground">
          · {meta}
        </span>
      </div>
      <div className="ml-2 flex shrink-0 items-center gap-1">
        {isMarket && (
          <Button
            variant="ghost"
            size="icon-xs"
            title={
              refreshing
                ? "正在拉取..."
                : "从远端拉取最新并清缓存（git pull + 清 plugins/cache）"
            }
            onClick={handleRefresh}
            disabled={refreshing}
            className={
              refreshing
                ? "text-foreground disabled:opacity-100"
                : "text-muted-foreground"
            }
          >
            <RefreshCw className={refreshing ? "animate-spin" : undefined} />
          </Button>
        )}
        {!isMarket && (
          <Switch
            size="sm"
            checked={pluginEnabled}
            onCheckedChange={(v) => onTogglePlugin(filter.plugin, v)}
            title={pluginEnabled ? "禁用整个插件" : "启用整个插件"}
          />
        )}
        {isMarket && (
          <Button
            variant="ghost"
            size="icon-xs"
            title={`移除 ${filter.marketplace}`}
            onClick={() => onDeleteMarketplace(filter.marketplace)}
            className="text-muted-foreground hover:text-destructive"
          >
            <Trash />
          </Button>
        )}
      </div>
    </>
  );
}

function parsePluginName(raw: string): string {
  const idx = raw.lastIndexOf("@");
  return idx < 0 ? raw : raw.slice(0, idx);
}
