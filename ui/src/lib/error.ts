export function extractErrorMessage(payload: unknown): string | null {
  if (!payload) return null;
  if (typeof payload === "string") return payload;

  if (typeof payload === "object") {
    const obj = payload as any;
    const message = obj?.message;
    if (typeof message === "string") return message;
    const err = obj?.error;
    if (typeof err === "string") return err;
    if (err && typeof err === "object") {
      if (typeof err.message === "string") return err.message;
      if (typeof err.error?.message === "string") return err.error.message;
    }

    if (typeof obj?.message === "string") return obj.message;
    if (typeof obj?.detail === "string") return obj.detail;
  }

  return null;
}

export function extractErrorCode(payload: unknown): string | null {
  if (!payload) return null;
  if (typeof payload === "object") {
    const obj = payload as any;
    const code = obj?.code;
    if (typeof code === "string") return code;
    const err = obj?.error;
    if (err && typeof err === "object" && typeof err.code === "string") return err.code;
  }
  return null;
}

export class ApiRequestError extends Error {
  readonly code: string | null;
  readonly serverMessage: string | null;
  readonly status: number;
  readonly method: string;
  readonly path: string;

  constructor(opts: {
    code: string | null;
    message: string | null;
    status: number;
    method: string;
    path: string;
  }) {
    super(opts.code ?? opts.message ?? `${opts.method} ${opts.path} failed: ${opts.status}`);
    this.name = "ApiRequestError";
    this.code = opts.code;
    this.serverMessage = opts.message;
    this.status = opts.status;
    this.method = opts.method;
    this.path = opts.path;
  }
}

export function isApiRequestError(e: unknown): e is ApiRequestError {
  return e instanceof ApiRequestError;
}

export function humanizeErrorText(s: string): string {
  const t = s.trim();
  if (!t.startsWith("{") && !t.startsWith("[")) return s;
  try {
    const parsed = JSON.parse(t);
    return extractErrorMessage(parsed) ?? s;
  } catch {
    return s;
  }
}

type TFunction = (key: string, vars?: Record<string, string | number>) => string;

function translateErrorCode(t: TFunction, code: string): string | null {
  const key = `errors.${code}`;
  const translated = t(key);
  return translated === key ? null : translated;
}

export function humanizeApiError(err: unknown, t: TFunction): string {
  if (isApiRequestError(err) && err.code) {
    return translateErrorCode(t, err.code) ?? err.serverMessage ?? err.message;
  }

  if (err instanceof Error) {
    const code = (err as any)?.code;
    if (typeof code === "string") {
      return translateErrorCode(t, code) ?? err.message;
    }
    const msg = err.message || String(err);
    const translated = translateErrorCode(t, msg);
    return translated ?? msg;
  }

  const s = String(err);
  return translateErrorCode(t, s) ?? s;
}
