import {
  Button,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";

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

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t bg-background px-4 py-3 rounded-b-lg">
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        <span>{t("common.pagination.total", { total: total.toLocaleString() })}</span>
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
          <SelectTrigger className="h-8 w-[100px]">
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

      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={() => onPageChange(Math.max(1, page - 1))}
          disabled={disabled || page <= 1}
        >
          {t("common.pagination.prev")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => onPageChange(Math.min(totalPages, page + 1))}
          disabled={disabled || page >= totalPages}
        >
          {t("common.pagination.next")}
        </Button>
      </div>
    </div>
  );
}
