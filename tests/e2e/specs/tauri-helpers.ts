import type { Page } from '@playwright/test';

/**
 * Returns true when window.__TAURI_INTERNALS__ is present in the page context.
 * Use this to skip IPC-dependent tests gracefully in devserver mode (no Tauri bridge).
 */
export async function hasTauriAvailable(page: Page): Promise<boolean> {
  return page.evaluate(
    () => typeof (window as any).__TAURI_INTERNALS__ !== 'undefined'
  );
}
