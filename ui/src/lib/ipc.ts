export function postIpc(payload: unknown) {
  const anyWindow = window as any;
  const fn = anyWindow?.ipc?.postMessage as ((msg: string) => void) | undefined;
  if (fn) fn(JSON.stringify(payload));
}

