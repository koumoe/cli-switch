import * as React from "react";
import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-react";
import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  type Row,
  type SortingState,
  useReactTable,
} from "@tanstack/react-table";

import { PaginationBar } from "@/components/PaginationBar";
import {
  Skeleton,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { cn } from "@/lib/utils";

export type DataTableColumnMeta = {
  headerClassName?: string;
  cellClassName?: string;
  skeletonClassName?: string;
};

export type DataTablePagination = {
  page: number;
  pageSize: number;
  total?: number;
  pageSizeOptions?: number[];
  disabled?: boolean;
  manual?: boolean;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
};

type DataTableProps<TData> = {
  columns: Array<ColumnDef<TData, unknown>>;
  data: TData[];
  loading?: boolean;
  loadingRowCount?: number;
  getRowId?: (originalRow: TData, index: number) => string;
  emptyState?: React.ReactNode;
  pagination?: DataTablePagination;
  containerClassName?: string;
  tableClassName?: string;
  headerClassName?: string;
  className?: string;
  stickyHeader?: boolean;
  initialSorting?: SortingState;
  rowClassName?: (row: Row<TData>) => string | undefined;
  isRowExpanded?: (row: Row<TData>) => boolean;
  renderExpandedRow?: (row: Row<TData>) => React.ReactNode;
  expandedRowCellClassName?: string;
};

function getColumnMeta<TData>(column: ColumnDef<TData, unknown>): DataTableColumnMeta {
  return (column.meta ?? {}) as DataTableColumnMeta;
}

function SortIcon({ sorting }: { sorting: false | "asc" | "desc" }) {
  if (sorting === "asc") {
    return <ArrowUp className="h-3.5 w-3.5" />;
  }
  if (sorting === "desc") {
    return <ArrowDown className="h-3.5 w-3.5" />;
  }
  return <ArrowUpDown className="h-3.5 w-3.5 opacity-60" />;
}

export function DataTable<TData>({
  columns,
  data,
  loading = false,
  loadingRowCount,
  getRowId,
  emptyState = null,
  pagination,
  containerClassName,
  tableClassName,
  headerClassName,
  className,
  stickyHeader = true,
  initialSorting = [],
  rowClassName,
  isRowExpanded,
  renderExpandedRow,
  expandedRowCellClassName,
}: DataTableProps<TData>) {
  const [sorting, setSorting] = React.useState<SortingState>(initialSorting);

  const table = useReactTable({
    data,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getRowId,
  });

  const rows = table.getRowModel().rows;
  const leafColumns = table.getVisibleLeafColumns();
  const manualPagination = pagination?.manual ?? false;
  const total = pagination
    ? manualPagination
      ? pagination.total ?? data.length
      : rows.length
    : rows.length;
  const totalPages = pagination ? Math.max(1, Math.ceil(total / pagination.pageSize)) : 1;
  const currentPage = pagination ? Math.min(Math.max(1, pagination.page), totalPages) : 1;

  const visibleRows = React.useMemo(() => {
    if (!pagination) {
      return rows;
    }
    if (manualPagination) {
      return rows;
    }
    const start = (currentPage - 1) * pagination.pageSize;
    return rows.slice(start, start + pagination.pageSize);
  }, [currentPage, manualPagination, pagination, rows]);

  const showLoadingRows = loading && data.length === 0;
  const skeletonRowCount = Math.min(loadingRowCount ?? pagination?.pageSize ?? 5, 6);

  return (
    <div className={cn("flex min-h-0 flex-1 flex-col", className)}>
      <div className="flex-1 min-h-0 overflow-hidden">
        <Table
          containerClassName={cn("h-full overflow-y-auto", containerClassName)}
          className={tableClassName}
        >
          <TableHeader
            className={cn(
              stickyHeader && "sticky top-0 z-10 bg-white dark:bg-slate-900",
              headerClassName
            )}
          >
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const meta = getColumnMeta(header.column.columnDef);
                  const sorted = header.column.getIsSorted();
                  const canSort = header.column.getCanSort();

                  return (
                    <TableHead key={header.id} className={meta.headerClassName}>
                      {header.isPlaceholder ? null : canSort ? (
                        <button
                          type="button"
                          className="inline-flex items-center gap-1 transition-colors hover:text-slate-900 dark:hover:text-slate-100"
                          onClick={header.column.getToggleSortingHandler()}
                        >
                          {flexRender(header.column.columnDef.header, header.getContext())}
                          <SortIcon sorting={sorted} />
                        </button>
                      ) : (
                        flexRender(header.column.columnDef.header, header.getContext())
                      )}
                    </TableHead>
                  );
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {showLoadingRows ? (
              Array.from({ length: skeletonRowCount }).map((_, rowIndex) => (
                <TableRow key={`loading-${rowIndex}`}>
                  {leafColumns.map((column) => {
                    const meta = getColumnMeta(column.columnDef);
                    return (
                      <TableCell
                        key={`${column.id}-${rowIndex}`}
                        className={meta.cellClassName}
                      >
                        <Skeleton className={cn("h-4 w-full", meta.skeletonClassName)} />
                      </TableCell>
                    );
                  })}
                </TableRow>
              ))
            ) : visibleRows.length > 0 ? (
              visibleRows.map((row) => (
                <React.Fragment key={row.id}>
                  <TableRow className={rowClassName?.(row)} data-row-id={row.id}>
                    {row.getVisibleCells().map((cell) => {
                      const meta = getColumnMeta(cell.column.columnDef);
                      return (
                        <TableCell key={cell.id} className={meta.cellClassName}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </TableCell>
                      );
                    })}
                  </TableRow>
                  {renderExpandedRow && isRowExpanded?.(row) ? (
                    <TableRow
                      data-expanded-for={row.id}
                      className="bg-blue-50/20 hover:bg-blue-50/20 dark:bg-slate-800/20 dark:hover:bg-slate-800/20"
                    >
                      <TableCell
                        colSpan={Math.max(leafColumns.length, 1)}
                        className={cn("p-4 text-left", expandedRowCellClassName)}
                      >
                        {renderExpandedRow(row)}
                      </TableCell>
                    </TableRow>
                  ) : null}
                </React.Fragment>
              ))
            ) : (
              <TableRow>
                <TableCell
                  colSpan={Math.max(leafColumns.length, 1)}
                  className="py-8 text-center text-slate-500 dark:text-slate-400"
                >
                  {emptyState}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>

      {pagination ? (
        <PaginationBar
          page={currentPage}
          total={total}
          totalPages={totalPages}
          pageSize={pagination.pageSize}
          pageSizeOptions={pagination.pageSizeOptions}
          disabled={pagination.disabled}
          onPageChange={pagination.onPageChange}
          onPageSizeChange={pagination.onPageSizeChange}
        />
      ) : null}
    </div>
  );
}
