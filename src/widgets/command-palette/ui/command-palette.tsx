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
          onBack={() => setView({ kind: "list" })}
          onDone={() => onOpenChange(false)}
        />
      )}
    </CommandDialog>
  );
}

function InputView({
  item,
  onBack,
  onDone,
}: {
  item: CommandInputItem;
  onBack: () => void;
  onDone: () => void;
}) {
  const [value, setValue] = useState("");
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const t = setTimeout(() => inputRef.current?.focus(), 0);
    return () => clearTimeout(t);
  }, []);

  const submit = async () => {
    const trimmed = value.trim();
    if (!trimmed || loading) return;
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
    <div className="flex h-12 items-center gap-3 border-b px-4">
      <Icon size={16} className="shrink-0 text-muted-foreground" />
      <input
        ref={inputRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") submit();
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            onBack();
          }
        }}
        placeholder={item.input.placeholder}
        disabled={loading}
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
  );
}
