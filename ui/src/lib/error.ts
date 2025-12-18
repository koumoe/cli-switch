export function extractErrorMessage(payload: unknown): string | null {
  if (!payload) return null;
  if (typeof payload === "string") return payload;

  if (typeof payload === "object") {
    const obj = payload as any;
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
