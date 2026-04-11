import { useState } from "react";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ColumnDef } from "@tanstack/react-table";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";

import { DataTable } from "./data-table";

type DemoRow = {
  id: string;
  name: string;
};

const rows: DemoRow[] = [
  { id: "alpha", name: "Alpha" },
  { id: "beta", name: "Beta" },
  { id: "gamma", name: "Gamma" },
];

const columns: Array<ColumnDef<DemoRow>> = [
  {
    accessorKey: "name",
    header: "Name",
  },
];

function PaginatedTable() {
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(2);

  return (
    <DataTable
      columns={columns}
      data={rows}
      getRowId={(row) => row.id}
      pagination={{
        page,
        pageSize,
        onPageChange: setPage,
        onPageSizeChange: setPageSize,
      }}
    />
  );
}

describe("DataTable", () => {
  it("pages through rows with PaginationBar", async () => {
    const user = userEvent.setup();

    renderWithProviders(<PaginatedTable />);

    expect(screen.getByText("Alpha")).toBeInTheDocument();
    expect(screen.getByText("Beta")).toBeInTheDocument();
    expect(screen.queryByText("Gamma")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "下一页" }));

    expect(screen.queryByText("Alpha")).not.toBeInTheDocument();
    expect(screen.queryByText("Beta")).not.toBeInTheDocument();
    expect(screen.getByText("Gamma")).toBeInTheDocument();
  });
});
