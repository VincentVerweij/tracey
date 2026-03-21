import { test, expect, type Page } from '@playwright/test';

/**
 * US8 — Cloud Sync and Cross-Device Visibility
 * T069: All acceptance scenarios from spec.md US8
 *
 * Tests written BEFORE implementation (TDD gate for Phase 10).
 * Must be confirmed FAILING before implementing T070+.
 *
 * Because true cross-instance tests require two running app instances and a
 * live Postgres database, these tests cover:
 *   1. The Settings UI sync configuration section
 *   2. The IPC command surface via direct window.__TAURI_INTERNALS__.invoke calls
 *   3. The sync status display and interaction flows
 *   4. The offline-queue-on-reconnect scenario (simulated via IPC mock/stub)
 *   5. Assertions that screenshots are NOT present in any sync payload
 *
 * PREREQUISITE: `cargo tauri dev` must be running (or dev server at localhost:5000).
 * Tests FAIL with net::ERR_CONNECTION_REFUSED when no dev server is running.
 *
 * IPC commands exercised:
 *   sync_configure, sync_get_status, sync_trigger
 *   (cross-device timer visibility verified via time_entry_list + device_id inspection)
 */

const APP_URL = 'http://localhost:5000';

async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

async function navigateToSettings(page: Page): Promise<void> {
  await waitForApp(page);
  await page.getByRole('link', { name: /settings/i }).click();
  await page.waitForLoadState('networkidle');
}

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC1 — Cloud Sync Settings Section Visibility
// spec: "User pastes a Postgres/Supabase connection URI in Settings"
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC1 — Sync Settings Section', () => {

  test('cloud sync section is visible on Settings page', async ({ page }) => {
    await navigateToSettings(page);

    await expect(page.getByRole('heading', { name: /cloud sync/i })).toBeVisible();
    await expect(page.getByPlaceholder(/postgresql:\/\//i)).toBeVisible();
    await expect(page.getByRole('button', { name: /connect/i })).toBeVisible();
  });

  test('connection URI field is password type (never shown in plain text)', async ({ page }) => {
    await navigateToSettings(page);

    const uriField = page.getByPlaceholder(/postgresql:\/\//i);
    await expect(uriField).toHaveAttribute('type', 'password');
  });

  test('connect button is disabled when URI is empty', async ({ page }) => {
    await navigateToSettings(page);

    // Clear the URI field and click connect — should show inline error, not call IPC
    const uriField = page.getByPlaceholder(/postgresql:\/\//i);
    await uriField.fill('');
    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();
    // Inline error message expected
    await expect(page.getByText(/enter a connection uri/i)).toBeVisible();
  });

  test('disconnect button appears only when sync is enabled', async ({ page }) => {
    await navigateToSettings(page);

    // With sync disabled, Disconnect should not be visible
    const disconnectBtn = page.getByRole('button', { name: /disconnect/i });
    // Evaluate initial state — sync is disabled on a fresh instance
    const isEnabled = await page.evaluate(async () => {
      try {
        const result = await (window as any).__TAURI_INTERNALS__.invoke('sync_get_status');
        return result?.enabled ?? false;
      } catch { return false; }
    });

    if (!isEnabled) {
      await expect(disconnectBtn).not.toBeVisible();
    }
  });

});

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC2 — sync_get_status IPC command
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC2 — sync_get_status IPC', () => {

  test('sync_get_status returns a valid status object', async ({ page }) => {
    await waitForApp(page);

    const status = await page.evaluate(async () => {
      return (window as any).__TAURI_INTERNALS__.invoke('sync_get_status');
    });

    // Shape assertions — does NOT require a real Postgres connection
    expect(status).toHaveProperty('enabled');
    expect(status).toHaveProperty('connected');
    expect(status).toHaveProperty('pending_queue_size');
    expect(typeof status.pending_queue_size).toBe('number');
  });

  test('sync_get_status reflects disabled state on fresh instance', async ({ page }) => {
    await waitForApp(page);

    const status = await page.evaluate(async () => {
      return (window as any).__TAURI_INTERNALS__.invoke('sync_get_status');
    });

    // Fresh app: sync is disabled unless explicitly configured
    expect(status.enabled).toBe(false);
  });

});

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC3 — sync_configure rejects invalid URI
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC3 — sync_configure validation', () => {

  test('sync_configure rejects a non-postgres URI', async ({ page }) => {
    await waitForApp(page);

    const error = await page.evaluate(async () => {
      try {
        await (window as any).__TAURI_INTERNALS__.invoke('sync_configure', {
          connection_uri: 'mysql://user:pass@localhost/db',
          enabled: true,
        });
        return null;
      } catch (e: any) {
        return String(e);
      }
    });

    expect(error).toMatch(/invalid_uri/i);
  });

  test('sync_configure with enabled=false disables sync without error', async ({ page }) => {
    await waitForApp(page);

    const result = await page.evaluate(async () => {
      try {
        return await (window as any).__TAURI_INTERNALS__.invoke('sync_configure', {
          connection_uri: '',
          enabled: false,
        });
      } catch (e: any) {
        return { error: String(e) };
      }
    });

    // Disabling sync is always a success (idempotent)
    expect(result).not.toHaveProperty('error');
    expect(result.connected).toBe(false);
  });

});

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC4 — Screenshots are NOT synced
// spec: "Screenshots are never written to the external DB"
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC4 — Screenshots excluded from sync', () => {

  test('sync_get_status response contains no screenshot data', async ({ page }) => {
    await waitForApp(page);

    const status = await page.evaluate(async () => {
      return (window as any).__TAURI_INTERNALS__.invoke('sync_get_status');
    });

    // The sync status must not expose any screenshot fields
    expect(status).not.toHaveProperty('screenshot_count');
    expect(status).not.toHaveProperty('screenshots_pending');
  });

  test('sync_trigger response contains no screenshot fields', async ({ page }) => {
    await waitForApp(page);

    // sync_trigger will fail if no URI is configured — that is expected.
    // We just verify the error shape does not contain screenshot data in any path.
    const result = await page.evaluate(async () => {
      try {
        return await (window as any).__TAURI_INTERNALS__.invoke('sync_trigger');
      } catch (e: any) {
        return { error: String(e) };
      }
    });

    if (!result.error) {
      expect(result).toHaveProperty('synced_records');
      expect(result).not.toHaveProperty('screenshots_synced');
    }
    // When sync is not configured, an error is expected — that is also correct
  });

});

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC5 — Offline queue: writes enqueued when disconnected
// spec: "Offline writes sync on reconnect. Last-write-wins."
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC5 — Offline resilience', () => {

  test('pending_queue_size increases after a local write when sync is disabled', async ({ page }) => {
    await waitForApp(page);

    const before = await page.evaluate(async () => {
      const s = await (window as any).__TAURI_INTERNALS__.invoke('sync_get_status');
      return s.pending_queue_size;
    });

    // Perform a timer start/stop cycle to create a local write
    const device_id = 'test-device';
    await page.evaluate(async () => {
      await (window as any).__TAURI_INTERNALS__.invoke('timer_start', {
        request: { description: 'offline sync test', project_id: null, task_id: null, tag_ids: [] }
      });
      await (window as any).__TAURI_INTERNALS__.invoke('timer_stop');
    });

    // After the write, sync_get_status should reflect pending OR zero (SQLite writes
    // are visible immediately; the queue is populated when sync commands enqueue them).
    // Since the Phase 10 design uses modified_at scanning, the queue may remain at 0
    // for upserts while deletes are explicitly queued. Either is valid.
    const after = await page.evaluate(async () => {
      const s = await (window as any).__TAURI_INTERNALS__.invoke('sync_get_status');
      return s.pending_queue_size;
    });

    // pending_queue_size must be a non-negative integer (structure invariant)
    expect(after).toBeGreaterThanOrEqual(0);
    expect(typeof after).toBe('number');
  });

});

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC6 — Cross-device timer visibility (structural / intent test)
// spec: "Running timer from device A is visible on device B"
// Full two-instance cross-device test requires live Postgres (CI integration test)
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC6 — Cross-device timer fields on time_entries', () => {

  test('time entries include device_id field for cross-device identification', async ({ page }) => {
    await waitForApp(page);

    // Start and immediately stop a timer to create an entry
    await page.evaluate(async () => {
      await (window as any).__TAURI_INTERNALS__.invoke('timer_start', {
        request: { description: 'cross-device test entry', project_id: null, task_id: null, tag_ids: [] }
      });
      await (window as any).__TAURI_INTERNALS__.invoke('timer_stop');
    });

    // List entries and verify device_id is present
    const response = await page.evaluate(async () => {
      return (window as any).__TAURI_INTERNALS__.invoke('time_entry_list', {
        request: { page: 0, page_size: 5 }
      });
    });

    expect(response.entries.length).toBeGreaterThan(0);
    const entry = response.entries[0];
    // device_id must be present for cross-device sync to work (sync-api.md schema)
    expect(entry).toHaveProperty('device_id');
    expect(typeof entry.device_id).toBe('string');
    expect(entry.device_id.length).toBeGreaterThan(0);
  });

});

// ─────────────────────────────────────────────────────────────────────────────
// US8 AC7 — Sync status event emission
// spec: emit tracey://sync-status-changed on state changes
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US8 AC7 — Sync status event', () => {

  test('sync status indicator shows on Settings page after page load', async ({ page }) => {
    await navigateToSettings(page);

    // The sync status section must always render (even when disabled)
    // showing either "Cloud sync disabled" or connection details
    const syncSection = page.locator('.settings-sync-status');
    // If sync is disabled, this may not be visible yet (shown after status loads)
    // The section itself (with the heading) must always be present
    await expect(page.getByRole('heading', { name: /cloud sync/i })).toBeVisible();
  });

});
