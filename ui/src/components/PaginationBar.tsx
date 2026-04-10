import {
  Button,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { formatNumber } from "@/lib/format";
import { useI18n } from "@/hooks/use-i18n";

type PaginationBarProps = {
  page: number;
  total: number;
  totalPages: number;
  pageSize: number;
  pageSizeOptions?: number[];
  disabled?: boolean;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
};

export function PaginationBar({
  page,
  total,
  totalPages,
  pageSize,
  pageSizeOptions = [20, 50, 100, 200],
  disabled = false,
  onPageChange,
  onPageSizeChange,
}: PaginationBarProps) {
  const { t } = useI18n();
  const pages: number[] = [];
  const start = Math.max(1, page - 1);
  const end = Math.min(totalPages, start + 2);

  for (let current = Math.max(1, end - 2); current <= end; current += 1) {
    pages.push(current);
  }

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-3 py-2">
      <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
        <span>{t("common.pagination.total", { total: formatNumber(total) })}</span>
        <span>{t("common.pagination.page", { page, totalPages })}</span>
        <Select
          value={String(pageSize)}
          onValueChange={(value) => {
            const next = Number(value);
            if (Number.isFinite(next) && next > 0) {
              onPageSizeChange(next);
            }
          }}
          disabled={disabled}
        >
          <SelectTrigger className="h-7 w-[92px] rounded-md px-2 text-[11px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {pageSizeOptions.map((option) => (
              <SelectItem key={option} value={String(option)}>
                {option}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <span>{t("common.pagination.perPage")}</span>
      </div>

      <div className="flex items-center gap-1">
        <Button
          variant="outline"
          size="icon"
          onClick={() => onPageChange(Math.max(1, page - 1))}
          disabled={disabled || page <= 1}
          aria-label={t("common.pagination.prev")}
        >
          <span className="text-xs">‹</span>
        </Button>
        {pages.map((item) => (
          <Button
            key={item}
            variant={item === page ? "default" : "outline"}
            size="icon"
            onClick={() => onPageChange(item)}
            disabled={disabled}
            aria-current={item === page ? "page" : undefined}
          >
            <span className="text-[11px]">{item}</span>
          </Button>
        ))}
        <Button
          variant="outline"
          size="icon"
          onClick={() => onPageChange(Math.min(totalPages, page + 1))}
          disabled={disabled || page >= totalPages}
          aria-label={t("common.pagination.next")}
        >
          <span className="text-xs">›</span>
        </Button>
      </div>
    </div>
  );
}
