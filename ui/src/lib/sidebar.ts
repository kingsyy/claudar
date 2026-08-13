export const SIDEBAR_COLLAPSED_STORAGE_KEY = "claudar-sidebar-collapsed";

export function normalizeSidebarCollapsed(value: unknown): boolean {
  return value === "true";
}

export function readSidebarCollapsed(): boolean {
  try {
    return normalizeSidebarCollapsed(localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY));
  } catch {
    return false;
  }
}

export function saveSidebarCollapsed(collapsed: boolean) {
  try {
    localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, String(collapsed));
  } catch {
    // The session state still updates if storage is unavailable.
  }
}
