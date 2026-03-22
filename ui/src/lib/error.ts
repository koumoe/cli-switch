export function extractErrorMessage(payload: unknown): string | null {
  if (!payload) return null;
  if (typeof payload === "string") return payload;

  if (typeof payload === "object") {
    const obj = payload as any;
    const issue = obj?.issue;
    if (issue && typeof issue === "object") {
      if (typeof issue.message === "string") return issue.message;
      if (typeof issue.detail === "string") return issue.detail;
    }
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
    const issue = obj?.issue;
    if (issue && typeof issue === "object" && typeof issue.code === "string") return issue.code;
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
  readonly issue: UserFacingIssueLike | null;
  readonly status: number;
  readonly method: string;
  readonly path: string;

  constructor(opts: {
    code: string | null;
    message: string | null;
    issue?: UserFacingIssueLike | null;
    status: number;
    method: string;
    path: string;
  }) {
    super(opts.code ?? opts.message ?? `${opts.method} ${opts.path} failed: ${opts.status}`);
    this.name = "ApiRequestError";
    this.code = opts.code;
    this.serverMessage = opts.message;
    this.issue = opts.issue ?? null;
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

export type UserFacingIssueLike = {
  code?: string | null;
  message?: string | null;
  args?: Record<string, string> | null;
  detail?: string | null;
};

function translateErrorCode(t: TFunction, code: string): string | null {
  const direct = t(code);
  if (direct !== code) return direct;
  const fallbackKey = `errors.${code}`;
  const fallback = t(fallbackKey);
  return fallback === fallbackKey ? null : fallback;
}

export function humanizeIssue(issue: UserFacingIssueLike | null | undefined, t: TFunction): string | undefined {
  if (!issue) return undefined;
  const args = issue.args ?? undefined;
  if (issue.code) {
    const direct = t(issue.code, args);
    if (direct !== issue.code) return direct;
    const fallbackKey = `errors.${issue.code}`;
    const fallback = t(fallbackKey, args);
    if (fallback !== fallbackKey) return fallback;
  }
  if (issue.message) return issue.message;
  if (issue.detail) return issue.detail;
  return undefined;
}

export function humanizeApiError(err: unknown, t: TFunction): string {
  if (isApiRequestError(err)) {
    const issueText = humanizeIssue(err.issue, t);
    if (issueText) return issueText;
    if (err.code) {
      return translateErrorCode(t, err.code) ?? err.serverMessage ?? err.message;
    }
    return err.serverMessage ?? err.message;
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
