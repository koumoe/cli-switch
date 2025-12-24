import React from "react";
import { Plus, X } from "lucide-react";
import { Button, Input, Badge } from "@/components/ui";
import { useI18n } from "@/lib/i18n";

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

type Props = {
  items: ListItem[];
  onChange: (items: ListItem[]) => void;
  placeholder?: string;
  addLabel: string;
  isMasked?: boolean;
};

let tempIdCounter = 0;
export function generateTempId(): string {
  return `__temp_${Date.now()}_${++tempIdCounter}`;
}

export function ItemListEditor({ items, onChange, placeholder, addLabel, isMasked = false }: Props) {
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
            <>
              <Input
                value={item.maskedValue}
                readOnly
                className="flex-1 font-mono text-sm bg-muted cursor-not-allowed select-none"
                tabIndex={-1}
              />
              <Badge variant="secondary" className="text-xs shrink-0">
                {t("channels.modal.savedBadge")}
              </Badge>
            </>
          ) : (
            <>
              <Input
                value={item.value}
                onChange={(e) => updateNewItem(idx, e.target.value)}
                placeholder={placeholder}
                className="flex-1 font-mono text-sm"
                type={isMasked ? "password" : "text"}
                autoComplete="off"
              />
              <Badge variant="outline" className="text-xs shrink-0">
                {t("channels.modal.newBadge")}
              </Badge>
            </>
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
 * 从后端返回的 ChannelKey 列表转换为 ListItem[]
 */
export function keysToItems(
  keys: Array<{ id: string; auth_ref_masked: string }>
): ListItem[] {
  return keys.map((k) => ({
    type: "saved" as const,
    id: k.id,
    maskedValue: k.auth_ref_masked,
  }));
}

/**
 * 将 ListItem[] 转换为提交给后端的字符串数组
 * 已保存的项使用 __KEEP__:id 格式，新项直接使用值
 */
export function itemsToAuthRefs(items: ListItem[]): string[] {
  return items
    .map((item) => {
      if (item.type === "saved") {
        return `__KEEP__:${item.id}`;
      } else {
        return item.value.trim();
      }
    })
    .filter(Boolean);
}

/**
 * 将 ListItem[] 转换为提交给后端的 base_urls 数组
 * Endpoint 不需要 __KEEP__ 前缀，因为它们不是敏感数据
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
