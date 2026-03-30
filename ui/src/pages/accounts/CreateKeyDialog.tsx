import React, { useEffect, useMemo, useState } from "react";
import { Copy, KeyRound } from "lucide-react";
import { toast } from "sonner";

import type { RemoteGroupOption, RemoteKey, Sub2ApiRemoteAccount } from "@/api";
import { createRemoteAccountKey, listRemoteAccountGroups } from "@/api";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { humanizeApiError } from "@/lib/error";
import { useI18n } from "@/lib/i18n";

import { formatGroupLabel, resolveAccountDisplayName } from "./shared";

type CreateKeyDialogProps = {
  open: boolean;
  target: Sub2ApiRemoteAccount | null;
  onOpenChange: (open: boolean) => void;
};

type CreateKeyDraft = {
  name: string;
  group_id: number | null;
};

const EMPTY_DRAFT: CreateKeyDraft = {
  name: "",
  group_id: null,
};

export function CreateKeyDialog({ open, target, onOpenChange }: CreateKeyDialogProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<CreateKeyDraft>(EMPTY_DRAFT);
  const [groups, setGroups] = useState<RemoteGroupOption[]>([]);
  const [loadingGroups, setLoadingGroups] = useState(false);
  const [creating, setCreating] = useState(false);
  const [createdKey, setCreatedKey] = useState<RemoteKey | null>(null);
  const selectedGroup = useMemo(
    () => groups.find((group) => group.id === draft.group_id) ?? null,
    [draft.group_id, groups]
  );

  useEffect(() => {
    if (!open || !target) return;
    let alive = true;
    setDraft(EMPTY_DRAFT);
    setCreatedKey(null);
    setGroups([]);
    setLoadingGroups(true);
    void listRemoteAccountGroups(target.id)
      .then((items) => {
        if (!alive) return;
        setGroups(items);
      })
      .catch((e) => {
        if (!alive) return;
        toast.error(t("accounts.toast.loadGroupsFail"), { description: humanizeApiError(e, t) });
      })
      .finally(() => {
        if (alive) setLoadingGroups(false);
      });
    return () => {
      alive = false;
    };
  }, [open, target, t]);

  async function handleCreate() {
    if (!target) return;
    const name = draft.name.trim();
    if (!name) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.createKey.nameRequired") });
      return;
    }
    setCreating(true);
    try {
      const key = await createRemoteAccountKey(target.id, {
        name,
        group_id: draft.group_id,
      });
      setCreatedKey(key);
      toast.success(t("accounts.toast.createKeyOk"));
    } catch (e) {
      toast.error(t("accounts.toast.createKeyFail"), { description: humanizeApiError(e, t) });
    } finally {
      setCreating(false);
    }
  }

  async function handleCopy() {
    if (!createdKey) return;
    try {
      await navigator.clipboard.writeText(createdKey.key);
      toast.success(t("accounts.toast.copyKeyOk"));
    } catch {
      toast.error(t("accounts.toast.copyKeyFail"));
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{t("accounts.createKey.title")}</DialogTitle>
          <DialogDescription>
            {t("accounts.createKey.description", {
              name: target ? resolveAccountDisplayName(target) : "",
            })}
          </DialogDescription>
        </DialogHeader>

        {createdKey ? (
          <div className="space-y-4">
            <div className="rounded-lg border bg-muted/20 p-4 space-y-3">
              <div className="flex items-center gap-2 text-sm font-medium">
                <KeyRound className="h-4 w-4" />
                {createdKey.name}
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.createKey.keyValue")}</label>
                <Input readOnly value={createdKey.key} />
              </div>
              <div className="text-xs text-muted-foreground">
                {t("accounts.createKey.keyMeta", {
                  status: createdKey.status,
                  group: selectedGroup?.name || t("accounts.createKey.noGroup"),
                })}
              </div>
            </div>
          </div>
        ) : (
          <div className="space-y-4 py-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.createKey.name")}</label>
              <Input
                value={draft.name}
                onChange={(e) => setDraft((current) => ({ ...current, name: e.target.value }))}
                placeholder="default"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.createKey.group")}</label>
              <Select
                value={draft.group_id === null ? "__none__" : String(draft.group_id)}
                onValueChange={(value) => {
                  setDraft((current) => ({
                    ...current,
                    group_id: value === "__none__" ? null : Number(value),
                  }));
                }}
                disabled={loadingGroups}
              >
                <SelectTrigger>
                  <SelectValue placeholder={t("accounts.createKey.groupPlaceholder")} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">{t("accounts.createKey.noGroup")}</SelectItem>
                  {groups.filter((group) => group.id !== null).map((group) => (
                    <SelectItem key={group.id} value={String(group.id)}>
                      <div className="min-w-0 flex flex-col pr-2">
                        <span className="truncate">{formatGroupLabel(group)}</span>
                        {[group.platform, group.description].filter(Boolean).join(" · ") ? (
                          <span className="text-xs text-muted-foreground">
                            {[group.platform, group.description].filter(Boolean).join(" · ")}
                          </span>
                        ) : null}
                      </div>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={creating}>
            {t("common.cancel")}
          </Button>
          {createdKey ? (
            <Button onClick={() => void handleCopy()}>
              <Copy className="h-4 w-4 mr-2" />
              {t("accounts.createKey.copy")}
            </Button>
          ) : (
            <Button onClick={() => void handleCreate()} disabled={creating || !target}>
              {t("accounts.createKey.create")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
