import React from "react";
import { Plus, X } from "lucide-react";
import { Button, Input, Badge, Textarea } from "@/components/ui";
import { useI18n } from "@/lib/i18n";

// ============ Endpoint 编辑器（列表式）============

export type SavedItem = {
  type: "saved";
  id: string;
  maskedValue: string;
};

export type NewItem = {
  type: "new";
  tempId: string;
  value: string;
};

export type ListItem = SavedItem | NewItem;

type ItemListEditorProps = {
  items: ListItem[];
  onChange: (items: ListItem[]) => void;
  placeholder?: string;
  addLabel: string;
};

let tempIdCounter = 0;
export function generateTempId(): string {
  return `__temp_${Date.now()}_${++tempIdCounter}`;
}

/**
 * 列表式编辑器，用于 Endpoints
 */
export function ItemListEditor({ items, onChange, placeholder, addLabel }: ItemListEditorProps) {
  const { t } = useI18n();

  function addNew() {
    const newItem: NewItem = {
      type: "new",
      tempId: generateTempId(),
      value: "",
    };
    onChange([...items, newItem]);
  }

  function removeItem(index: number) {
    const next = [...items];
    next.splice(index, 1);
    onChange(next);
  }

  function updateNewItem(index: number, value: string) {
    const next = [...items];
    const item = next[index];
    if (item?.type === "new") {
      next[index] = { ...item, value };
      onChange(next);
    }
  }

  return (
    <div className="space-y-2">
      {items.map((item, idx) => (
        <div
          key={item.type === "saved" ? item.id : item.tempId}
          className="flex items-center gap-2"
        >
          {item.type === "saved" ? (
            <Input
              value={item.maskedValue}
              readOnly
              className="flex-1 font-mono text-sm bg-muted cursor-not-allowed select-none"
              tabIndex={-1}
            />
          ) : (
            <Input
              value={item.value}
              onChange={(e) => updateNewItem(idx, e.target.value)}
              placeholder={placeholder}
              className="flex-1 font-mono text-sm"
              autoComplete="off"
            />
          )}
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="shrink-0 h-8 w-8"
            onClick={() => removeItem(idx)}
          >
            <X className="h-4 w-4 text-muted-foreground hover:text-destructive" />
          </Button>
        </div>
      ))}
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={addNew}
        className="w-full"
      >
        <Plus className="h-4 w-4 mr-2" />
        {addLabel}
      </Button>
    </div>
  );
}

// ============ Key 编辑器（混合模式：列表 + Textarea）============

export type SavedKey = {
  id: string;
  maskedValue: string;
  enabled: boolean;
  autoDisabledUntilMs: number;
};

type KeyListEditorProps = {
  savedKeys: SavedKey[];
  onSavedKeysChange: (keys: SavedKey[]) => void;
  newKeysText: string;
  onNewKeysTextChange: (text: string) => void;
  nowMs: number;
};

/**
 * 计算剩余封禁分钟数
 */
function remainingMinutes(untilMs: number, nowMs: number): number | null {
  if (untilMs <= nowMs) return null;
  return Math.max(1, Math.ceil((untilMs - nowMs) / 60000));
}

/**
 * 混合模式编辑器，用于 API Keys
 * - 已保存的 Keys：列表展示，显示状态（正常/封禁 x 分钟）
 * - 新增 Keys：Textarea 批量输入
 */
export function KeyListEditor({
  savedKeys,
  onSavedKeysChange,
  newKeysText,
  onNewKeysTextChange,
  nowMs,
}: KeyListEditorProps) {
  const { t } = useI18n();

  function removeKey(id: string) {
    onSavedKeysChange(savedKeys.filter((k) => k.id !== id));
  }

  return (
    <div className="space-y-3">
      {/* 已保存的 Keys */}
      {savedKeys.length > 0 && (
        <div className="space-y-2">
          <div className="text-xs text-muted-foreground">
            {t("channels.modal.savedKeysLabel")}
          </div>
          {savedKeys.map((key) => {
            const remaining = remainingMinutes(key.autoDisabledUntilMs, nowMs);
            const isDisabled = !key.enabled || remaining !== null;
            return (
              <div key={key.id} className="flex items-center gap-2">
                <Input
                  value={key.maskedValue}
                  readOnly
                  className="flex-1 font-mono text-sm bg-muted cursor-not-allowed select-none"
                  tabIndex={-1}
                />
                {isDisabled ? (
                  <Badge variant="warning" className="text-xs shrink-0">
                    {remaining !== null
                      ? t("channels.modal.keyBanned", { minutes: remaining })
                      : t("channels.modal.keyDisabled")}
                  </Badge>
                ) : (
                  <Badge variant="success" className="text-xs shrink-0">
                    {t("channels.modal.keyNormal")}
                  </Badge>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="shrink-0 h-8 w-8"
                  onClick={() => removeKey(key.id)}
                >
                  <X className="h-4 w-4 text-muted-foreground hover:text-destructive" />
                </Button>
              </div>
            );
          })}
        </div>
      )}

      {/* 新增 Keys - Textarea */}
      <div className="space-y-2">
        <div className="text-xs text-muted-foreground">
          {t("channels.modal.newKeysLabel")}
        </div>
        <Textarea
          value={newKeysText}
          onChange={(e) => onNewKeysTextChange(e.target.value)}
          placeholder={t("channels.modal.newKeysPlaceholder")}
          className="font-mono text-sm min-h-[80px]"
          autoComplete="off"
        />
      </div>
    </div>
  );
}

// ============ 工具函数 ============

/**
 * 从后端返回的 ChannelEndpoint 列表转换为 ListItem[]
 */
export function endpointsToItems(
  endpoints: Array<{ id: string; base_url: string }>
): ListItem[] {
  return endpoints.map((e) => ({
    type: "saved" as const,
    id: e.id,
    maskedValue: e.base_url,
  }));
}

/**
 * 从后端返回的 ChannelKey 列表转换为 SavedKey[]
 */
export function keysToSavedKeys(
  keys: Array<{
    id: string;
    auth_ref_masked: string;
    enabled: boolean;
    auto_disabled_until_ms: number;
  }>
): SavedKey[] {
  return keys.map((k) => ({
    id: k.id,
    maskedValue: k.auth_ref_masked,
    enabled: k.enabled,
    autoDisabledUntilMs: k.auto_disabled_until_ms,
  }));
}

/**
 * 解析多行文本为数组（去重去空）
 */
export function parseLines(raw: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const line of raw.split(/\r?\n/g)) {
    const s = line.trim();
    if (!s) continue;
    if (!seen.has(s)) {
      seen.add(s);
      out.push(s);
    }
  }
  return out;
}

/**
 * 将 SavedKey[] 和新增文本转换为提交给后端的 auth_refs 数组
 */
export function buildAuthRefs(savedKeys: SavedKey[], newKeysText: string): string[] {
  const savedRefs = savedKeys.map((k) => `__KEEP__:${k.id}`);
  const newRefs = parseLines(newKeysText);
  return [...savedRefs, ...newRefs];
}

/**
 * 将 ListItem[] 转换为提交给后端的 base_urls 数组
 */
export function itemsToBaseUrls(items: ListItem[]): string[] {
  return items
    .map((item) => {
      if (item.type === "saved") {
        return item.maskedValue.trim();
      } else {
        return item.value.trim();
      }
    })
    .filter(Boolean);
}
