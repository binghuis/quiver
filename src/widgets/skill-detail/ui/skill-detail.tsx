import { useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Ban,
  BookOpen,
  Check,
  CloudUpload,
  Code2,
  Copy,
  Eye,
  FolderOpen,
  Info,
  Loader2,
  RefreshCw,
  Trash,
} from "lucide-react";
import { cn } from "@/lib/utils";
import {
  ECOSYSTEMS,
  EcosystemIcon,
  ecosystemColor,
  ecosystemLabel,
  parsePluginId,
  revealInFinder,
  type EcosystemPresence,
  type Skill,
  type SkillEcosystem,
} from "@/entities/skill";
import { Button } from "@/shared/ui/button";

type Mode = "rendered" | "source";

type SyncFailureMap = Partial<Record<SkillEcosystem, string>>;

type Props = {
  skill: Skill | null;
  presences: EcosystemPresence[];
  drifted: boolean;
  /** 「最近一次同步尝试失败」的 eco → 错误消息。成功后由上层清除。 */
  syncFailures?: SyncFailureMap;
  onToggle: (id: string, enabled: boolean) => void;
  onSyncTo: (
    sourceId: string,
    target: SkillEcosystem,
    overwrite?: boolean,
  ) => Promise<void>;
  onPushToAll: (sourceId: string) => Promise<void>;
  /** 顶栏"删除"按钮：级联删同逻辑 skill 的所有 user/project 副本。 */
  onDeleteLogical: (anyPresenceId: string) => Promise<void>;
  /** 生态切换行里的单个生态删除：只摘一份物理副本。 */
  onDeletePresence: (presenceId: string) => Promise<void>;
};

export function SkillDetail({
  skill,
  presences,
  drifted,
  syncFailures,
  onToggle,
  onSyncTo,
  onPushToAll,
  onDeleteLogical,
  onDeletePresence,
}: Props) {
  const [mode, setMode] = useState<Mode>("rendered");

  if (!skill) return <EmptyState />;

  // 只要任一份 presence 来自 plugin，就按 plugin 口径展示——user-scope 的同步副本
  // 只是分发结果，身份仍属于原插件，不能回标成「用户级」。
  const pluginPresence = presences.find((p) => p.skill.scope === "plugin");
  const parsed = parsePluginId(pluginPresence?.skill.plugin ?? skill.plugin);
  const breadcrumb =
    pluginPresence && parsed
      ? `${parsed.marketplace} › ${parsed.name}`
      : null;

  return (
    <section className="flex h-full w-full flex-col overflow-hidden">
      <DetailHeader
        skill={skill}
        presences={presences}
        drifted={drifted}
        syncFailures={syncFailures}
        breadcrumb={breadcrumb}
        mode={mode}
        onModeChange={setMode}
        onToggle={onToggle}
        onSyncTo={onSyncTo}
        onPushToAll={onPushToAll}
        onDeleteLogical={onDeleteLogical}
        onDeletePresence={onDeletePresence}
      />

      <div className="flex-1 overflow-y-auto">
        <BodyView skill={skill} mode={mode} />
      </div>
    </section>
  );
}

function DetailHeader({
  skill,
  presences,
  drifted,
  syncFailures,
  breadcrumb,
  mode,
  onModeChange,
  onToggle,
  onSyncTo,
  onPushToAll,
  onDeleteLogical,
  onDeletePresence,
}: {
  skill: Skill;
  presences: EcosystemPresence[];
  drifted: boolean;
  syncFailures?: SyncFailureMap;
  breadcrumb: string | null;
  mode: Mode;
  onModeChange: (m: Mode) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onSyncTo: (
    sourceId: string,
    target: SkillEcosystem,
    overwrite?: boolean,
  ) => Promise<void>;
  onPushToAll: (sourceId: string) => Promise<void>;
  onDeleteLogical: (anyPresenceId: string) => Promise<void>;
  onDeletePresence: (presenceId: string) => Promise<void>;
}) {
  const [copied, setCopied] = useState(false);
  const [syncBusy, setSyncBusy] = useState<SkillEcosystem | null>(null);
  const [pushBusy, setPushBusy] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);

  const copyBody = async () => {
    try {
      await navigator.clipboard.writeText(skill.body);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      /* ignore */
    }
  };
  const reveal = async () => {
    try {
      await revealInFinder(skill.path);
    } catch {
      /* ignore */
    }
  };

  const triggerSync = async (
    target: SkillEcosystem,
    overwrite: boolean,
  ) => {
    setSyncBusy(target);
    setSyncError(null);
    try {
      await onSyncTo(skill.id, target, overwrite);
    } catch (e) {
      setSyncError(e instanceof Error ? e.message : String(e));
    } finally {
      setSyncBusy(null);
    }
  };

  const triggerPushAll = async () => {
    setPushBusy(true);
    setSyncError(null);
    try {
      await onPushToAll(skill.id);
    } catch (e) {
      setSyncError(e instanceof Error ? e.message : String(e));
    } finally {
      setPushBusy(false);
    }
  };

  const isSource = mode === "source";
  // 顶栏"删除"按钮语义：把这条 logical 里所有 user/project 副本一把走系统
  // 垃圾桶（包括「插件派生到别的生态的 user-scope 副本」）。plugin 本体永远
  // 跳过——要删插件走插件面板。按钮只要存在可删的 user/project 副本就可见，
  // 有 plugin 副本不再整个藏按钮（否则派生副本无法清理）。
  const hasPluginOrigin = presences.some((p) => p.skill.scope === "plugin");
  const hasRemovablePresence = presences.some(
    (p) => p.skill.scope === "user" || p.skill.scope === "project",
  );

  return (
    <header className="shrink-0 ">
      <div
        data-tauri-drag-region
        className="flex h-11 items-center gap-2 px-5 border-b"
      >
        <EcosystemRow
          presences={presences}
          currentSkill={skill}
          drifted={drifted}
          syncBusy={syncBusy}
          syncFailures={syncFailures}
          onSync={triggerSync}
          onRemove={(id) => {
            void onDeletePresence(id);
          }}
        />
        <h1 className="pointer-events-none min-w-0 truncate text-[15px] font-semibold">
          {skill.name}
        </h1>
        {breadcrumb && (
          <span className="pointer-events-none min-w-0 truncate text-[11px] text-muted-foreground">
            · {breadcrumb}
          </span>
        )}
        <div className="ml-auto flex shrink-0 items-center gap-1.5">
          {drifted && (
            <Button
              variant="ghost"
              size="icon-xs"
              title="用当前版本覆盖所有生态"
              onClick={triggerPushAll}
              disabled={pushBusy}
              className={cn(pushBusy && "bg-accent text-accent-foreground")}
            >
              {pushBusy ? (
                <Loader2 className="animate-spin" />
              ) : (
                <CloudUpload />
              )}
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon-xs"
            title={isSource ? "查看渲染" : "查看源码"}
            onClick={() => onModeChange(isSource ? "rendered" : "source")}
            className={cn(isSource && "bg-accent text-accent-foreground")}
          >
            {isSource ? <Eye /> : <Code2 />}
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            title={copied ? "已复制" : "复制 Markdown 正文"}
            onClick={copyBody}
          >
            {copied ? <Check /> : <Copy />}
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            title="在 Finder 中显示"
            onClick={reveal}
          >
            <FolderOpen />
          </Button>
          {hasRemovablePresence && (
            <Button
              variant="ghost"
              size="icon-xs"
              title={
                hasPluginOrigin
                  ? "删除所有用户/项目副本（插件本体保留，移到系统垃圾桶）"
                  : "删除所有生态副本（移到系统垃圾桶）"
              }
              onClick={() => {
                void onDeleteLogical(skill.id);
              }}
              className="text-muted-foreground hover:text-destructive"
            >
              <Trash />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon-xs"
            title={skill.enabled ? "禁用此 skill" : "启用此 skill"}
            onClick={() => onToggle(skill.id, !skill.enabled)}
            className={cn(
              !skill.enabled &&
                "bg-destructive/15 text-destructive hover:bg-destructive/20 hover:text-destructive",
            )}
          >
            <Ban />
          </Button>
        </div>
      </div>

      {skill.description && (
        <div className="flex items-start gap-2 bg-muted/30 px-5 py-2.5">
          <Info
            size={12}
            strokeWidth={1.75}
            className="mt-0.75 shrink-0 text-muted-foreground/70"
            aria-hidden
          />
          <p className="min-w-0 text-[12px] leading-[1.6] text-muted-foreground">
            {skill.description}
          </p>
        </div>
      )}

      {syncError && (
        <p className="px-5 pb-2 text-[11px] text-destructive">
          同步失败：{syncError}
        </p>
      )}
    </header>
  );
}

function EcosystemRow({
  presences,
  currentSkill,
  drifted,
  syncBusy,
  syncFailures,
  onSync,
  onRemove,
}: {
  presences: EcosystemPresence[];
  currentSkill: Skill;
  drifted: boolean;
  syncBusy: SkillEcosystem | null;
  syncFailures?: SyncFailureMap;
  onSync: (target: SkillEcosystem, overwrite: boolean) => void;
  onRemove: (presenceId: string) => void;
}) {
  // 红点与漂移琥珀点共用右上角插槽；红点更紧急，压过琥珀点显示。
  const DotBadge = ({ kind }: { kind: "failure" | "drift" }) => (
    <span
      className={cn(
        "absolute -right-0.5 -top-0.5 size-1.5 rounded-full ring-1 ring-background",
        kind === "failure" ? "bg-destructive" : "bg-amber-500",
      )}
    />
  );

  return (
    <div className="flex shrink-0 items-center gap-1">
      {ECOSYSTEMS.map((eco) => {
        const presence = presences.find((p) => p.ecosystem === eco);
        const isSelf = eco === currentSkill.ecosystem;
        const isDriftedSibling =
          !!presence &&
          presence.skill.content_hash !== currentSkill.content_hash;
        const isBusy = syncBusy === eco;
        const failure = syncFailures?.[eco];
        const dot: "failure" | "drift" | null = failure
          ? "failure"
          : isDriftedSibling
            ? "drift"
            : null;

        // 当前查看版本所在的生态：不可操作自己
        if (isSelf) {
          return (
            <span
              key={eco}
              title={`${ecosystemLabel(eco)}（当前查看的版本）`}
              className="flex size-6 shrink-0 items-center justify-center"
            >
              <EcosystemIcon eco={eco} className="size-3.5" aria-hidden />
            </span>
          );
        }

        // 其他生态已存在：
        // - plugin scope 副本由插件本体管理，不能单独删 → 渲染成不可点 span，
        //   tooltip 直接说清楚去哪里处理，避免"点了没反应"的空转感。
        // - user/project 副本 → 可点，点击移到系统垃圾桶。
        if (presence) {
          if (presence.skill.scope === "plugin") {
            return (
              <span
                key={eco}
                title={
                  failure
                    ? `${ecosystemLabel(eco)}（上次同步失败：${failure}）`
                    : `${ecosystemLabel(eco)}（此副本随插件管理，请从插件/市场面板移除）`
                }
                className="relative flex size-6 shrink-0 cursor-default items-center justify-center opacity-55"
              >
                <EcosystemIcon eco={eco} className="size-3.5" aria-hidden />
                {dot && <DotBadge kind={dot} />}
              </span>
            );
          }
          return (
            <Button
              key={eco}
              variant="ghost"
              size="icon-xs"
              title={
                failure
                  ? `${ecosystemLabel(eco)}（上次同步失败：${failure}；点击删除此副本）`
                  : isDriftedSibling
                    ? `${ecosystemLabel(eco)}（内容不同，点击删除）`
                    : `${ecosystemLabel(eco)}（点击删除）`
              }
              onClick={() => !isBusy && onRemove(presence.skill.id)}
              disabled={isBusy}
              className="relative"
            >
              {isBusy ? (
                <Loader2 className="animate-spin" />
              ) : (
                <EcosystemIcon eco={eco} className="size-3.5" aria-hidden />
              )}
              {dot && <DotBadge kind={dot} />}
            </Button>
          );
        }

        // 其他生态不存在：可点击同步到此生态
        return (
          <Button
            key={eco}
            variant="ghost"
            size="icon-xs"
            title={
              failure
                ? `同步到 ${ecosystemLabel(eco)} 失败：${failure}（点击重试）`
                : `同步到 ${ecosystemLabel(eco)}`
            }
            onClick={() => !isBusy && onSync(eco, false)}
            disabled={isBusy}
            className="relative text-muted-foreground/50"
            style={{ color: isBusy ? ecosystemColor(eco) : undefined }}
          >
            {isBusy ? (
              <Loader2 className="animate-spin" />
            ) : (
              <EcosystemIcon
                eco={eco}
                variant="mono"
                className="size-3.5"
                aria-hidden
              />
            )}
            {dot && <DotBadge kind={dot} />}
          </Button>
        );
      })}
      {drifted && (
        <RefreshCw
          aria-hidden
          size={11}
          className="ml-0.5 text-amber-500"
          strokeWidth={2.5}
        />
      )}
    </div>
  );
}

function EmptyState() {
  return (
    <section className="flex flex-1 flex-col">
      <div data-tauri-drag-region className="h-11 shrink-0 border-b" />
      <div className="flex flex-1 items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="flex size-10 items-center justify-center rounded-md border bg-card text-muted-foreground">
            <BookOpen size={18} />
          </div>
          <div className="flex flex-col gap-1">
            <p className="text-[13px] text-foreground">未选中 skill</p>
            <p className="text-[11.5px] text-muted-foreground">
              从左侧列表选中一项，查看正文与元数据
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function BodyView({ skill, mode }: { skill: Skill; mode: Mode }) {
  const hasBody = (skill.body ?? "").trim().length > 0;

  if (!hasBody) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-[12px] text-muted-foreground">（SKILL.md 无正文）</p>
      </div>
    );
  }

  if (mode === "source") {
    return (
      <div className="mx-auto max-w-3xl px-6 py-6">
        <pre
          className="my-0 overflow-x-auto whitespace-pre-wrap font-mono text-[13px]"
          style={{ lineHeight: 1.55 }}
        >
          <code style={{ lineHeight: "inherit" }}>{skill.body}</code>
        </pre>
      </div>
    );
  }

  return (
    <article className="prose mx-auto max-w-3xl px-6 py-6">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {skill.body}
      </ReactMarkdown>
    </article>
  );
}

// 唯一保留的 override：外链强制走系统浏览器，否则在 Tauri webview 里会抢占当前页面。
const markdownComponents: Components = {
  a: ({ href, children, ...props }) => (
    <a {...props} href={href} target="_blank" rel="noreferrer">
      {children}
    </a>
  ),
};

