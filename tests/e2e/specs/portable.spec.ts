import { test, expect, type Page } from '@playwright/test';
import { hasTauriAvailable } from './tauri-helpers';

/**
 * US9 — Run as Portable Application Without Admin Rights
 * T076: Playwright E2E tests verifying portable execution behavior
 *
 * These tests cover the behavioral guarantees of portable execution:
 *   (a) App initializes cleanly from first launch (DB seeded, dirs created)
 *   (b) Full timer cycle completes without permission errors
 *   (c) Data persists across navigation (no session-only storage)
 *
 * NOTE: The binary-in-tempdir test (copy exe to temp folder, run as restricted user)
 * is covered by the GitHub Actions CI job (T079) which runs as a restricted user
 * on a Windows runner. That test requires a built binary; this spec covers
 * in-process behavioral verification.
 *
 * IPC commands exercised:
 *   health_get, preferences_get, timer_start, timer_stop, time_entry_list
 */

const APP_URL = 'http://localhost:5000';

/**
 * Navigate to the app and wait for Blazor WASM to hydrate.
 * Blazor WASM loads the .NET runtime asynchronously — networkidle ensures
 * the app shell has rendered before any interaction.
 */
async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

// ─────────────────────────────────────────────────────────────────────────────
// IPC Fixture Helpers
// ─────────────────────────────────────────────────────────────────────────────

async function startTimer(page: Page, description: string): Promise<void> {
  await page.evaluate(async (desc) => {
    await (window as any).__TAURI_INTERNALS__.invoke('timer_start', {
      request: { description, project_id: null, task_id: null, tag_ids: [] },
    });
  }, description);
}

async function stopTimer(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await (window as any).__TAURI_INTERNALS__.invoke('timer_stop', {});
  });
}

async function listEntries(page: Page): Promise<any[]> {
  return page.evaluate(async () => {
    return await (window as any).__TAURI_INTERNALS__.invoke('time_entry_list', {
      request: { page: 1, page_size: 20 },
    });
  });
}

async function getActiveTimer(page: Page): Promise<any> {
  return page.evaluate(async () => {
    return await (window as any).__TAURI_INTERNALS__.invoke('timer_get_active', {});
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Cleanup: stop any running timer after each test to avoid cross-test leakage
// ─────────────────────────────────────────────────────────────────────────────

test.afterEach(async ({ page }) => {
  try {
    await stopTimer(page);
  } catch {
    // No timer running — safe to ignore
  }
});

// ─────────────────────────────────────────────────────────────────────────────
// US9 — Portable Execution Behavioral Tests
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US9 — Portable Execution Behavioral Guarantees', () => {

  test.beforeEach(async ({ page }) => {
    if (!(await hasTauriAvailable(page))) {
      test.skip(true, 'Requires Tauri bridge — run with tauri-driver for IPC tests');
    }
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Test 1: First-launch initialization — preferences seeded with defaults
  // Portable guarantee: first-launch init ran cleanly (DB created in exe dir
  // and seeded). If init silently failed (e.g., a missing-write-permission
  // error), preferences_get would return an error or empty row.
  // ───────────────────────────────────────────────────────────────────────────

  test('first launch — preferences are seeded with defaults', async ({ page }) => {
    await waitForApp(page);

    const prefs = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('preferences_get');
    });

    // Core portable defaults — these values are set by T012 first-launch init.
    // If the DB was not created (e.g., no write access to exe directory), this
    // call would throw. If migration 001 seeding failed, values would be null/0.
    expect(prefs).toBeTruthy();
    expect(prefs.inactivity_timeout_seconds).toBe(300);
    expect(prefs.screenshot_interval_seconds).toBe(300);

    // Retention default (14 days per spec)
    expect(typeof prefs.screenshot_retention_days).toBe('number');
    expect(prefs.screenshot_retention_days).toBeGreaterThan(0);

    // Timezone is seeded (non-empty string)
    expect(typeof prefs.local_timezone).toBe('string');
    expect(prefs.local_timezone.length).toBeGreaterThan(0);
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Test 2: No-elevation operation — full timer cycle without permission errors
  // Portable guarantee: timer_start/timer_stop/time_entry_list all complete
  // without throwing. A "permission denied" or "readonly database" error from
  // SQLite would surface as a rejected IPC promise.
  // ───────────────────────────────────────────────────────────────────────────

  test('full timer cycle completes without permission errors', async ({ page }) => {
    await waitForApp(page);

    // Start timer — writes a row to time_entries with ended_at = NULL
    await expect(
      page.evaluate(async () => {
        return await (window as any).__TAURI_INTERNALS__.invoke('timer_start', {
          request: {
            description: 'Portable test — no elevation required',
            project_id: null,
            task_id: null,
            tag_ids: [],
          },
        });
      })
    ).resolves.not.toThrow();

    // Confirm the timer is active
    const active = await getActiveTimer(page);
    expect(active).not.toBeNull();
    expect(active.description).toBe('Portable test — no elevation required');

    // Stop timer — updates ended_at on the row
    await expect(
      page.evaluate(async () => {
        return await (window as any).__TAURI_INTERNALS__.invoke('timer_stop', {});
      })
    ).resolves.not.toThrow();

    // Verify the completed entry appears in the list (DB write persisted)
    const entries = await listEntries(page);
    const found = entries.find((e: any) => e.description === 'Portable test — no elevation required');
    expect(found).toBeTruthy();
    expect(found.ended_at).not.toBeNull();
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Test 3: Data survives page navigation
  // Portable guarantee: data is stored in SQLite on disk (exe directory), not
  // in memory or sessionStorage. Navigating away and back must not lose state.
  // ───────────────────────────────────────────────────────────────────────────

  test('data survives page navigation', async ({ page }) => {
    await waitForApp(page);

    // Start a timer
    await startTimer(page, 'Portable persistence check');

    // Confirm timer is running
    const activeBefore = await getActiveTimer(page);
    expect(activeBefore).not.toBeNull();
    expect(activeBefore.description).toBe('Portable persistence check');

    // Navigate away (Tags page)
    await page.goto(`${APP_URL}/tags`);
    await page.waitForLoadState('networkidle');

    // Navigate back to Dashboard
    await page.goto(APP_URL);
    await page.waitForLoadState('networkidle');

    // Timer must still be active — stored in SQLite, not in-memory only
    const activeAfter = await getActiveTimer(page);
    expect(activeAfter).not.toBeNull();
    expect(activeAfter.description).toBe('Portable persistence check');
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Test 4: Health check reports DB open and no active errors
  // Portable guarantee: the app is running cleanly — DB opened successfully
  // (if it had been opened read-only or in a path lacking write permission,
  // the WAL preamble write would fail and be captured in active_errors).
  // ───────────────────────────────────────────────────────────────────────────

  test('health check reports db open and no active errors', async ({ page }) => {
    await waitForApp(page);

    const health = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('health_get');
    });

    // running: true means the Rust backend initialised successfully
    expect(health.running).toBe(true);

    // active_errors is an array; must be empty for a clean portable launch
    expect(Array.isArray(health.active_errors)).toBe(true);
    expect(health.active_errors.length).toBe(0);
  });

});
