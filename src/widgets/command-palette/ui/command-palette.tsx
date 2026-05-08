import { Fragment, useEffect, useRef, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { toast } from "sonner";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/shared/ui/command";
import { Kbd } from "@/shared/ui/kbd";

export type CommandInputConfig = {
  placeholder: string;
  onSubmit: (value: string) => Promise<void> | void;
  /** 进入输入态时拉一次现有值并 prefill。配置型命令（如 git proxy）用得上。 */
  initialValue?: () => Promise<string> | string;
  /** 输入条下方一行小字，写格式提示 / 例子。 */
  helperText?: string;
  /** 允许提交空字符串。默认 false（防止误触）；配置项要清除时打开。 */
  allowEmpty?: boolean;
};

type CommonFields = {
  id: string;
  label: string;
  icon: LucideIcon;
  hint?: string;
  disabled?: boolean;
};

type CommandActionItem = CommonFields & {
  onRun: () => void;
  input?: undefined;
};

type CommandInputItem = CommonFields & {
  input: CommandInputConfig;
  onRun?: undefined;
};

export type CommandPaletteItem = CommandActionItem | CommandInputItem;

export type CommandPaletteGroup = {
  heading: string;
  items: CommandPaletteItem[];
};

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  groups: CommandPaletteGroup[];
};

type View =
  | { kind: "list" }
  | { kind: "input"; item: CommandInputItem };

export function CommandPalette({ open, onOpenChange, groups }: Props) {
  const [view, setView] = useState<View>({ kind: "list" });

  useEffect(() => {
    if (!open) setView({ kind: "list" });
  }, [open]);

  const handleItemSelect = (item: CommandPaletteItem) => {
    if (item.input) {
      setView({ kind: "input", item });
    } else {
      item.onRun();
      onOpenChange(false);
    }
  };

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="命令面板"
      description="选择一条命令执行"
      showCloseButton={false}
      className="top-[18vh] w-140 max-w-[90vw] translate-y-0 gap-0 p-0"
    >
      {view.kind === "list" ? (
        <>
          <CommandInput placeholder="输入命令…" />
          <CommandList>
            <CommandEmpty>无匹配命令</CommandEmpty>
            {groups.map((group, i) => (
              <Fragment key={group.heading}>
                {i > 0 && <CommandSeparator />}
                <CommandGroup heading={group.heading}>
                  {group.items.map((item) => (
                    <CommandItem
                      key={item.id}
                      value={item.label}
                      disabled={item.disabled}
                      onSelect={() => handleItemSelect(item)}
                    >
                      <item.icon />
                      <span>{item.label}</span>
                      {item.hint && (
                        <CommandShortcut>{item.hint}</CommandShortcut>
                      )}
                    </CommandItem>
                  ))}
                </CommandGroup>
              </Fragment>
            ))}
          </CommandList>
        </>
      ) : (
        <InputView
          item={view.item}
          onDone={() => onOpenChange(false)}
        />
      )}
    </CommandDialog>
  );
}

function InputView({
  item,
  onDone,
}: {
  item: CommandInputItem;
  onDone: () => void;
}) {
  const [value, setValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [prefilling, setPrefilling] = useState(
    typeof item.input.initialValue === "function",
  );
  const inputRef = useRef<HTMLInputElement>(null);

  // 拉 prefill。同步返回直接拿；异步返回等一下，期间 input 禁用避免用户在
  // 旧空值上抢跑提交。组件卸载后忽略结果，避免 setState on unmounted。
  useEffect(() => {
    const provider = item.input.initialValue;
    if (!provider) return;
    let cancelled = false;
    void Promise.resolve(provider())
      .then((v) => {
        if (!cancelled) setValue(v);
      })
      .catch(() => {
        // prefill 失败就当空——仍然允许用户输入新值。
      })
      .finally(() => {
        if (!cancelled) setPrefilling(false);
      });
    return () => {
      cancelled = true;
    };
  }, [item.input]);

  useEffect(() => {
    if (prefilling) return;
    const t = setTimeout(() => inputRef.current?.focus(), 0);
    return () => clearTimeout(t);
  }, [prefilling]);

  const submit = async () => {
    if (loading || prefilling) return;
    const trimmed = value.trim();
    if (!trimmed && !item.input.allowEmpty) return;
    setLoading(true);
    try {
      await item.input.onSubmit(trimmed);
      onDone();
    } catch (e) {
      toast.error(`${item.label}失败`, {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setLoading(false);
    }
  };

  const Icon = item.icon;

  return (
    <div>
      <div className="flex h-12 items-center gap-3 border-b px-4">
        <Icon size={16} className="shrink-0 text-muted-foreground" />
        <input
          ref={inputRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
          }}
          placeholder={item.input.placeholder}
          disabled={loading || prefilling}
          className="min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-muted-foreground disabled:opacity-60"
        />
        {loading ? (
          <span className="flex shrink-0 items-center gap-1.5 text-[11px] text-muted-foreground">
            <span className="size-1.5 animate-pulse rounded-full bg-primary" />
            执行中…
          </span>
        ) : (
          <div className="flex shrink-0 items-center gap-1">
            <Kbd>↵</Kbd>
            <Kbd>Esc</Kbd>
          </div>
        )}
      </div>
      {item.input.helperText && (
        <div className="px-4 py-2 text-[11px] text-muted-foreground">
          {item.input.helperText}
        </div>
      )}
    </div>
  );
}
