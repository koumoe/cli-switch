import { createMemoryHistory, createRouter } from "@tanstack/react-router";

import { routeTree } from "./routeTree.gen";

const ROUTER_STATE_KEY = "cliswitch-router-href";
const ROUTE_PREFIXES = [
  "/channels",
  "/accounts",
  "/prompts",
  "/monitor",
  "/logs",
  "/settings",
] as const;

function isAppHref(href: string | null | undefined): href is string {
  if (typeof href !== "string") return false;
  const trimmed = href.trim();
  if (!trimmed.startsWith("/") || trimmed.startsWith("//")) return false;

  const [pathname] = trimmed.split(/[?#]/);
  if (!pathname || pathname.endsWith(".html")) return false;
  if (pathname === "/") return true;

  return ROUTE_PREFIXES.some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));
}

function readPersistedHref(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(ROUTER_STATE_KEY);
  } catch {
    return null;
  }
}

function writePersistedHref(href: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(ROUTER_STATE_KEY, href);
  } catch {
    // ignore storage failures and keep the in-memory router usable
  }
}

function readBrowserHref(): string | null {
  if (typeof window === "undefined") return null;
  const href = `${window.location.pathname}${window.location.search}${window.location.hash}`;
  return isAppHref(href) ? href : null;
}

function resolveInitialHref(): string {
  const browserHref = readBrowserHref();
  if (browserHref && browserHref !== "/") {
    return browserHref;
  }

  const persistedHref = readPersistedHref();
  if (isAppHref(persistedHref)) {
    return persistedHref;
  }

  return browserHref ?? "/";
}

const history = createMemoryHistory({
  initialEntries: [resolveInitialHref()],
});

export const router = createRouter({
  history,
  routeTree,
  defaultPreload: "intent",
  scrollRestoration: true,
});

if (typeof window !== "undefined") {
  writePersistedHref(history.location.href || "/");
  history.subscribe(({ location }) => {
    const href = `${location.pathname}${location.search}${location.hash}` || "/";
    if (isAppHref(href)) {
      writePersistedHref(href);
    }
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
