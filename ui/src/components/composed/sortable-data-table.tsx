import * as React from "react";
import {
  closestCenter,
  DndContext,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  type Row,
  useReactTable,
} from "@tanstack/react-table";

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

import { type DataTableColumnMeta } from "./data-table";

type SortableDataTableProps<TData> = {
  columns: Array<ColumnDef<TData, unknown>>;
  data: TData[];
  getRowId: (row: TData) => string;
  onReorder: (next: TData[]) => void;
  onReorderCommit?: (next: TData[]) => void | Promise<void>;
  loading?: boolean;
  loadingRowCount?: number;
  disabled?: boolean;
  emptyState?: React.ReactNode;
  className?: string;
  tableClassName?: string;
  containerClassName?: string;
  rowClassName?: (
    row: Row<TData>,
    state: { isDragging: boolean },
  ) => string | undefined;
};

type SortableHandleContextValue = {
  attributes: Record<string, unknown>;
  listeners: Record<string, ((event: Event) => void) | undefined>;
  setActivatorNodeRef: (element: HTMLElement | null) => void;
  disabled: boolean;
};

const SortableHandleContext =
  React.createContext<SortableHandleContextValue | null>(null);

function getColumnMeta<TData>(
  column: ColumnDef<TData, unknown>,
): DataTableColumnMeta {
  return (column.meta ?? {}) as DataTableColumnMeta;
}

type SortableRowProps<TData> = {
  row: Row<TData>;
  disabled: boolean;
  rowClassName?: (
    row: Row<TData>,
    state: { isDragging: boolean },
  ) => string | undefined;
};

function SortableRow<TData>({
  row,
  disabled,
  rowClassName,
}: SortableRowProps<TData>) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: row.id,
    disabled,
  });

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <SortableHandleContext.Provider
      value={{
        attributes: attributes as unknown as Record<string, unknown>,
        listeners: (listeners ?? {}) as Record<
          string,
          ((event: Event) => void) | undefined
        >,
        setActivatorNodeRef,
        disabled,
      }}
    >
      <TableRow
        ref={setNodeRef}
        style={style}
        className={cn(
          isDragging && "relative z-10 opacity-70",
          rowClassName?.(row, { isDragging }),
        )}
      >
        {row.getVisibleCells().map((cell) => {
          const meta = getColumnMeta(cell.column.columnDef);
          return (
            <TableCell key={cell.id} className={meta.cellClassName}>
              {flexRender(cell.column.columnDef.cell, cell.getContext())}
            </TableCell>
          );
        })}
      </TableRow>
    </SortableHandleContext.Provider>
  );
}

export function SortableDataTableHandle({
  children,
  className,
  title,
}: {
  children: React.ReactNode;
  className?: string;
  title?: string;
}) {
  const context = React.useContext(SortableHandleContext);

  if (!context) {
    return null;
  }

  return (
    <button
      type="button"
      ref={context.setActivatorNodeRef}
      className={cn(
        "cursor-grab text-muted-foreground hover:text-foreground active:cursor-grabbing disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      title={title}
      disabled={context.disabled}
      {...context.attributes}
      {...context.listeners}
    >
      {children}
    </button>
  );
}

export function SortableDataTable<TData>({
  columns,
  data,
  getRowId,
  onReorder,
  onReorderCommit,
  loading = false,
  loadingRowCount,
  disabled = false,
  emptyState = null,
  className,
  tableClassName,
  containerClassName,
  rowClassName,
}: SortableDataTableProps<TData>) {
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 6 },
    }),
  );

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (row) => getRowId(row),
  });

  const items = React.useMemo(
    () => data.map((item) => getRowId(item)),
    [data, getRowId],
  );
  const leafColumns = table.getVisibleLeafColumns();
  const showLoadingRows = loading && data.length === 0;
  const skeletonRowCount = Math.min(loadingRowCount ?? 5, 6);

  async function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;

    if (!over || active.id === over.id) {
      return;
    }

    const oldIndex = data.findIndex((item) => getRowId(item) === active.id);
    const newIndex = data.findIndex((item) => getRowId(item) === over.id);

    if (oldIndex < 0 || newIndex < 0 || oldIndex === newIndex) {
      return;
    }

    const next = arrayMove(data, oldIndex, newIndex);
    onReorder(next);
    await onReorderCommit?.(next);
  }

  return (
    <div className={cn("flex min-h-0 flex-1 flex-col", className)}>
      <div className="flex-1 min-h-0 overflow-hidden">
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragEnd={(event) => {
            void handleDragEnd(event);
          }}
        >
          <Table
            containerClassName={cn("overflow-auto", containerClassName)}
            className={tableClassName}
          >
            <TableHeader className="sticky top-0 z-10 bg-card">
              {table.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id}>
                  {headerGroup.headers.map((header) => {
                    const meta = getColumnMeta(header.column.columnDef);
                    return (
                      <TableHead
                        key={header.id}
                        className={meta.headerClassName}
                      >
                        {header.isPlaceholder
                          ? null
                          : flexRender(
                              header.column.columnDef.header,
                              header.getContext(),
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
                          <Skeleton
                            className={cn("h-4 w-full", meta.skeletonClassName)}
                          />
                        </TableCell>
                      );
                    })}
                  </TableRow>
                ))
              ) : data.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={Math.max(leafColumns.length, 1)}
                    className="py-8 text-center text-muted-foreground"
                  >
                    {emptyState}
                  </TableCell>
                </TableRow>
              ) : (
                <SortableContext
                  items={items}
                  strategy={verticalListSortingStrategy}
                >
                  {table.getRowModel().rows.map((row) => (
                    <SortableRow
                      key={row.id}
                      row={row}
                      disabled={disabled}
                      rowClassName={rowClassName}
                    />
                  ))}
                </SortableContext>
              )}
            </TableBody>
          </Table>
        </DndContext>
      </div>
    </div>
  );
}
