import { useEffect, useState } from "react";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { useGitProxy } from "../model/use-git-proxy";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function GitProxyDialog({ open, onOpenChange }: Props) {
  const { state, save, reload } = useGitProxy();
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // 每次打开都从磁盘拉一次：用户可能在其它地方改过 state.json。
  useEffect(() => {
    if (!open) return;
    setSaveError(null);
    void reload();
  }, [open, reload]);

  useEffect(() => {
    if (state.kind === "ready") setDraft(state.proxy);
  }, [state]);

  const handleSave = async () => {
    if (saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      await save(draft);
      onOpenChange(false);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Git HTTPS 代理</DialogTitle>
          <DialogDescription>
            App 内 git 命令（导入、刷新 marketplace）会走此代理。
            留空表示直连。终端的 HTTPS_PROXY 不会被继承，需在此显式配置。
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-2">
          <Input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="http://127.0.0.1:7890"
            disabled={state.kind === "loading" || saving}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSave();
            }}
            autoFocus
          />
          {state.kind === "error" && (
            <p className="text-[12px] text-destructive">
              读取失败：{state.message}
            </p>
          )}
          {saveError && (
            <p className="text-[12px] text-destructive">保存失败：{saveError}</p>
          )}
        </div>

        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={saving}
          >
            取消
          </Button>
          <Button
            onClick={handleSave}
            disabled={state.kind === "loading" || saving}
          >
            {saving ? "保存中…" : "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
