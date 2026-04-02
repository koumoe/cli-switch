export function postIpc(payload: unknown) {
  const anyWindow = window as any;
  const fn = anyWindow?.ipc?.postMessage as ((msg: string) => void) | undefined;
  if (fn) fn(JSON.stringify(payload));
}

type Sub2ApiDesktopAuthResult = {
  request_id: string;
  token?: string | null;
  refresh_token?: string | null;
  cancelled?: boolean;
  error?: string | null;
};

export type Sub2ApiDesktopAuthTokens = {
  bearerToken: string;
  refreshToken: string;
};

const SUB2API_AUTH_RESULT_EVENT = "cliswitch-sub2api-auth-result";

export function requestSub2ApiDesktopAuth(baseUrl: string): Promise<Sub2ApiDesktopAuthTokens | null> {
  const anyWindow = window as any;
  const fn = anyWindow?.ipc?.postMessage as ((msg: string) => void) | undefined;
  if (typeof fn !== "function") {
    return Promise.reject(new Error("sub2api_auth_unsupported"));
  }

  const requestId = globalThis.crypto?.randomUUID?.() ?? `sub2api-auth-${Date.now()}`;

  return new Promise<Sub2ApiDesktopAuthTokens | null>((resolve, reject) => {
    const cleanup = () => {
      window.removeEventListener(SUB2API_AUTH_RESULT_EVENT, onResult as EventListener);
    };

    const onResult = (event: Event) => {
      const detail = (event as CustomEvent<Sub2ApiDesktopAuthResult>).detail;
      if (!detail || detail.request_id !== requestId) return;
      cleanup();
      if (detail.cancelled) {
        resolve(null);
        return;
      }
      if (
        detail.token?.trim()
        && detail.refresh_token?.trim()
      ) {
        resolve({
          bearerToken: detail.token.trim(),
          refreshToken: detail.refresh_token.trim(),
        });
        return;
      }
      reject(new Error(detail.error?.trim() || "sub2api_auth_failed"));
    };

    window.addEventListener(SUB2API_AUTH_RESULT_EVENT, onResult as EventListener);
    try {
      fn(JSON.stringify({
        type: "request-sub2api-auth",
        request_id: requestId,
        base_url: baseUrl,
      }));
    } catch (error) {
      cleanup();
      reject(error instanceof Error ? error : new Error("sub2api_auth_failed"));
    }
  });
}
