import { afterEach, describe, expect, it, vi } from "vitest";

import { getCliToolsStatus } from "@/api";
import { cliStatus } from "@/test/fixtures/cli-tools";

afterEach(() => vi.unstubAllGlobals());

describe("CLI status API", () => {
  it.each([
    [undefined, "/api/tools/status"],
    [{ refresh: false }, "/api/tools/status"],
    [{ refresh: true }, "/api/tools/status?refresh=true"],
  ] as const)("uses the correct URL for %o", async (options, url) => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify(cliStatus())));
    vi.stubGlobal("fetch", fetchMock);

    expect(await getCliToolsStatus(options)).toEqual(cliStatus());
    expect(fetchMock).toHaveBeenCalledWith(url, expect.objectContaining({ method: "GET" }));
  });
});
