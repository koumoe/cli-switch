export const UPDATE_READY_SHOWN_KEY_PREFIX = "cliswitch-update-ready-shown:";

export function updateReadyShownKey(version: string): string {
  return `${UPDATE_READY_SHOWN_KEY_PREFIX}${version}`;
}

export function isUpdateReadyShown(version: string): boolean {
  try {
    return localStorage.getItem(updateReadyShownKey(version)) === "true";
  } catch {
    return false;
  }
}

export function markUpdateReadyShown(version: string): void {
  try {
    localStorage.setItem(updateReadyShownKey(version), "true");
  } catch {
    // ignore
  }
}

export function clearUpdateReadyShown(version: string): void {
  try {
    localStorage.removeItem(updateReadyShownKey(version));
  } catch {
    // ignore
  }
}

