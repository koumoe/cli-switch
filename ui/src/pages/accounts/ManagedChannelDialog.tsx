import React from "react";

import type { Protocol, RemoteAccount, RemoteGroupOption } from "@/api";
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
import { useI18n } from "@/lib/i18n";

import { defaultManagedName, formatGroupLabel, type ManagedChannelDraft } from "./shared";

type ManagedChannelDialogProps = {
  open: boolean;
  target: RemoteAccount | null;
  draft: ManagedChannelDraft | null;
  groups: RemoteGroupOption[];
  loadingGroups: boolean;
  creating: boolean;
  onOpenChange: (open: boolean) => void;
  setDraft: React.Dispatch<React.SetStateAction<ManagedChannelDraft | null>>;
  onCreate: () => void | Promise<void>;
};

export function ManagedChannelDialog({
  open,
  target,
  draft,
  groups,
  loadingGroups,
  creating,
  onOpenChange,
  setDraft,
  onCreate,
}: ManagedChannelDialogProps) {
  const { t } = useI18n();
  const selectedGroup = draft ? groups.find((group) => group.name === draft.group_name) ?? null : null;
  const selectedGroupLabel = selectedGroup ? formatGroupLabel(selectedGroup) : "";
  const selectedGroupAddedLabel = selectedGroup && selectedGroup.managed_channel_count > 0
    ? t("accounts.managed.groupAdded", { count: selectedGroup.managed_channel_count })
    : "";
  const selectedGroupDescription = selectedGroup?.description?.trim() ?? "";
  const selectedGroupMeta = [selectedGroupDescription, selectedGroupAddedLabel]
    .filter((value) => !!value)
    .join(" · ");
  const selectedGroupTitle = selectedGroupMeta
    ? `${selectedGroupLabel}\n${selectedGroupMeta}`
    : selectedGroupLabel;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle>{t("accounts.managed.title")}</DialogTitle>
          <DialogDescription>
            {t("accounts.managed.description", { name: target?.base_url ?? "" })}
          </DialogDescription>
        </DialogHeader>
        {draft ? (
          <div className="flex-1 min-h-0 space-y-4 py-2 overflow-y-auto pr-1">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.managed.name")}</label>
                <Input
                  value={draft.name}
                  onChange={(e) => setDraft((current) => (current ? { ...current, name: e.target.value } : current))}
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.managed.protocol")}</label>
                <Select
                  value={draft.protocol ?? ""}
                  onValueChange={(value) => {
                    setDraft((current) => {
                      if (!current) return current;
                      const protocol = value as Protocol;
                      if (!target) return { ...current, protocol };
                      const oldAuto = defaultManagedName(target, current.protocol);
                      const nextName = current.name === oldAuto ? defaultManagedName(target, protocol) : current.name;
                      return { ...current, protocol, name: nextName };
                    });
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t("accounts.managed.protocolPlaceholder")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="openai">OpenAI</SelectItem>
                    <SelectItem value="anthropic">Anthropic</SelectItem>
                    <SelectItem value="gemini">Gemini</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.managed.group")}</label>
              <Select
                value={draft.group_name}
                onValueChange={(value) => {
                  setDraft((current) => (current ? { ...current, group_name: value } : current));
                }}
                disabled={loadingGroups}
              >
                <SelectTrigger
                  title={selectedGroupTitle || undefined}
                  className="h-auto min-h-[3.5rem] py-2 [&>span]:whitespace-normal"
                >
                  <SelectValue
                    placeholder={t("accounts.managed.groupPlaceholder")}
                    aria-label={selectedGroupTitle || undefined}
                  >
                    {selectedGroup ? (
                      <div className="min-w-0">
                        <div className="truncate">{selectedGroupLabel}</div>
                        {selectedGroupMeta ? (
                          <div className="line-clamp-1 text-xs text-muted-foreground">{selectedGroupMeta}</div>
                        ) : null}
                      </div>
                    ) : undefined}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent
                  side="bottom"
                  align="start"
                  avoidCollisions={false}
                  collisionPadding={16}
                  className="max-w-[min(32rem,calc(100vw-2rem))]"
                >
                  {groups.map((group) => (
                    <SelectItem key={group.name} value={group.name}>
                      <div className="min-w-0 flex flex-col pr-2">
                        <span className="truncate">{formatGroupLabel(group)}</span>
                        {[group.description, group.managed_channel_count > 0
                          ? t("accounts.managed.groupAdded", { count: group.managed_channel_count })
                          : "",
                        ].filter((value) => !!value).join(" · ") ? (
                          <span className="text-xs text-muted-foreground">
                            {[group.description, group.managed_channel_count > 0
                              ? t("accounts.managed.groupAdded", { count: group.managed_channel_count })
                              : "",
                            ].filter((value) => !!value).join(" · ")}
                          </span>
                        ) : null}
                      </div>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.managed.baseUrlOverride")}</label>
              <Input
                value={draft.base_url_override}
                onChange={(e) => {
                  setDraft((current) => (current ? { ...current, base_url_override: e.target.value } : current));
                }}
                placeholder={target?.api_url || target?.base_url || "https://api.example.com/v1"}
              />
              <p className="text-xs text-muted-foreground">{t("accounts.managed.baseUrlOverrideHint")}</p>
            </div>
          </div>
        ) : null}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={creating}>
            {t("common.cancel")}
          </Button>
          <Button onClick={() => void onCreate()} disabled={creating || !draft}>
            {t("accounts.managed.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
