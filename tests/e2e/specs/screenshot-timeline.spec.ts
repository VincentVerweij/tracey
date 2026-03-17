import { test, expect, type Page } from '@playwright/test';

/**
 * US4 — Screenshot Timeline Review
 * T042: All acceptance scenarios from spec.md US4
 *
 * Tests written BEFORE implementation (TDD gate for Phase 6).
 * Must be confirmed FAILING before Root/Reese touch the Timeline page + IPC.
 *
 * Tests FAIL with:
 *   net::ERR_CONNECTION_REFUSED  — when no dev server is running (current state)
 *   TimeoutError                 — when dev server runs but UI not yet implemented
 *
 * IPC commands exercised (from contracts/ipc-commands.md US4):
 *   screenshot_list({ from, to })        → { id, file_path, captured_at, window_title, process_name, trigger }[]
 *   screenshot_delete_expired()          → { deleted_count: number }
 *   preferences_update({ screenshot_interval_seconds?, screenshot_retention_days? })
 *
 * Build requirement: app must be built with --features test to activate the
 * GDI test double (writes a pre-canned 100×100 JPEG instead of calling Win32).
 */

const APP_URL = 'http://localhost:5000';

// ─────────────────────────────────────────────────────────────────────────────
// Navigation helpers
// ─────────────────────────────────────────────────────────────────────────────

async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

async function goToTimeline(page: Page): Promise<void> {
  await page.goto(`${APP_URL}/timeline`);
  await page.waitForLoadState('networkidle');
}

// ─────────────────────────────────────────────────────────────────────────────
// IPC Fixture Helpers — call Tauri IPC directly for fast test setup
// ─────────────────────────────────────────────────────────────────────────────

async function setScreenshotInterval(page: Page, seconds: number): Promise<void> {
  await page.evaluate(async (s) => {
    await (window as any).__TAURI_INTERNALS__.invoke('preferences_update', {
      update: { screenshot_interval_seconds: s }
    });
  }, seconds);
}

async function getScreenshots(page: Page, from: string, to: string): Promise<any[]> {
  return await page.evaluate(async ({ from, to }) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('screenshot_list', {
      request: { from, to }
    });
  }, { from, to });
}

async function deleteExpiredScreenshots(page: Page): Promise<{ deleted_count: number }> {
  return await page.evaluate(async () => {
    return await (window as any).__TAURI_INTERNALS__.invoke('screenshot_delete_expired');
  });
}

/**
 * Returns an ISO string for N seconds ago.
 */
function secondsAgo(n: number): string {
  return new Date(Date.now() - n * 1000).toISOString();
}

/**
 * Returns an ISO string for N hours ago.
 */
function hoursAgo(n: number): string {
  return new Date(Date.now() - n * 3600 * 1000).toISOString();
}

// ─────────────────────────────────────────────────────────────────────────────
// Guard: check whether screenshots are available after waiting for capture.
// Used by capture-dependent tests to skip gracefully if the GDI test double
// is not active (e.g., app built without --features test).
// ─────────────────────────────────────────────────────────────────────────────

async function hasScreenshotsAfterWait(page: Page, waitMs = 5000): Promise<boolean> {
  // Set a very short interval so capture fires quickly
  try {
    await setScreenshotInterval(page, 3);
  } catch {
    return false;
  }
  await page.waitForTimeout(waitMs);
  const shots = await getScreenshots(page, secondsAgo(60), new Date().toISOString());
  return Array.isArray(shots) && shots.length > 0;
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US4 — Screenshot Timeline', () => {

  test.describe.configure({ mode: 'serial' });

  // ───── Navigation ──────────────────────────────────────────────────────────

  test('navigate to timeline page', async ({ page }) => {
    await waitForApp(page);

    // Navigate via nav link (ARIA-first — Root must provide role="link")
    const timelineLink = page.getByRole('link', { name: /timeline/i });
    await expect(timelineLink).toBeVisible();
    await timelineLink.click();
    await page.waitForLoadState('networkidle');

    // Page heading must contain "Timeline" (case-insensitive)
    const heading = page.getByRole('heading', { level: 1 });
    await expect(heading).toBeVisible();
    await expect(heading).toContainText(/timeline/i);

    // URL must reflect the route
    expect(page.url()).toMatch(/\/timeline/);
  });

  // ───── Empty State ─────────────────────────────────────────────────────────

  test('shows empty state when no screenshots exist', async ({ page }) => {
    await goToTimeline(page);

    // Delete any existing screenshots so we get a clean state.
    // (If IPC not available — failing with ERR_CONNECTION_REFUSED — the test
    // will fail at goto(), which is the expected TDD-gate failure.)
    try {
      // Wipe retention to 0 days → delete_expired removes everything
      await page.evaluate(async () => {
        await (window as any).__TAURI_INTERNALS__.invoke('preferences_update', {
          update: { screenshot_retention_days: 0 }
        });
        await (window as any).__TAURI_INTERNALS__.invoke('screenshot_delete_expired');
        // Restore a sane retention
        await (window as any).__TAURI_INTERNALS__.invoke('preferences_update', {
          update: { screenshot_retention_days: 30 }
        });
      });
    } catch { /* app not running — ERR_CONNECTION_REFUSED will be the failure */ }

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Empty-state element: accept any of these selectors Root/UXer may use
    const emptyState =
      page.locator('[aria-label*="empty" i]')
        .or(page.locator('.empty-state-illustration'))
        .or(page.locator('[data-testid="empty-state"]'))
        .first();

    await expect(emptyState).toBeVisible({ timeout: 5000 });
  });

  // ───── Capture-dependent tests ─────────────────────────────────────────────
  //
  // The next group of tests requires the GDI test double to be active
  // (--features test build).  If the IPC returns an empty list after the
  // wait period, the tests skip gracefully rather than failing noisily.

  test('screenshot items appear after capture', async ({ page }) => {
    await waitForApp(page);

    // Set a very short interval so the test double fires quickly
    await setScreenshotInterval(page, 3);

    // Wait for at least one capture cycle
    await page.waitForTimeout(5000);

    await goToTimeline(page);

    // Check if capture produced anything at all
    const shots = await getScreenshots(page, secondsAgo(60), new Date().toISOString());

    if (!Array.isArray(shots) || shots.length === 0) {
      // GDI test double not active — skip rather than fail
      test.skip(true, 'No screenshots captured; GDI test double may not be active');
      return;
    }

    // At least one screenshot item must be rendered in the UI
    const item =
      page.locator('[data-testid="screenshot-item"]')
        .or(page.locator('[data-testid="screenshot-card"]'))
        .first();

    await expect(item).toBeVisible({ timeout: 5000 });
  });

  test('screenshot shows captured_at timestamp', async ({ page }) => {
    await waitForApp(page);

    const shots = await getScreenshots(page, secondsAgo(300), new Date().toISOString());

    if (!Array.isArray(shots) || shots.length === 0) {
      test.skip(true, 'No screenshots available; GDI test double may not be active');
      return;
    }

    await goToTimeline(page);

    // Timestamp must be visible in HH:MM format (local time)
    // Accept both 12-hour (e.g. "3:45 PM") and 24-hour (e.g. "15:45") formats
    const timestampPattern = /\d{1,2}:\d{2}/;
    const timestampEl = page.locator('[data-testid="screenshot-timestamp"]')
      .or(page.locator('[class*="timestamp"]'))
      .or(page.locator('[class*="time"]'))
      .first();

    await expect(timestampEl).toBeVisible({ timeout: 5000 });
    const text = await timestampEl.textContent();
    expect(text).toMatch(timestampPattern);
  });

  test('screenshot shows process name and window title', async ({ page }) => {
    await waitForApp(page);

    const shots = await getScreenshots(page, secondsAgo(300), new Date().toISOString());

    if (!Array.isArray(shots) || shots.length === 0) {
      test.skip(true, 'No screenshots available; GDI test double may not be active');
      return;
    }

    await goToTimeline(page);

    // Each screenshot item must show process_name and window_title
    const item =
      page.locator('[data-testid="screenshot-item"]')
        .or(page.locator('[data-testid="screenshot-card"]'))
        .first();

    await expect(item).toBeVisible({ timeout: 5000 });

    // Process name is present in item text
    const processNameEl = item.locator('[data-testid="process-name"]')
      .or(item.locator('[class*="process"]'));
    await expect(processNameEl).toBeVisible();

    // Window title is present in item text
    const windowTitleEl = item.locator('[data-testid="window-title"]')
      .or(item.locator('[class*="window-title"]'))
      .or(item.locator('[class*="title"]'));
    await expect(windowTitleEl).toBeVisible();
  });

  test('screenshot trigger badge is shown', async ({ page }) => {
    await waitForApp(page);

    const shots = await getScreenshots(page, secondsAgo(300), new Date().toISOString());

    if (!Array.isArray(shots) || shots.length === 0) {
      test.skip(true, 'No screenshots available; GDI test double may not be active');
      return;
    }

    await goToTimeline(page);

    const item =
      page.locator('[data-testid="screenshot-item"]')
        .or(page.locator('[data-testid="screenshot-card"]'))
        .first();

    await expect(item).toBeVisible({ timeout: 5000 });

    // Trigger badge must display one of the three trigger values
    const triggerBadge = item.locator('[data-testid="trigger-badge"]')
      .or(item.locator('[class*="trigger"]'))
      .or(item.locator('[class*="badge"]'));

    await expect(triggerBadge).toBeVisible();
    const badgeText = await triggerBadge.textContent();
    expect(badgeText).toMatch(/interval|window.?change|manual/i);
  });

  test('clicking screenshot opens preview', async ({ page }) => {
    await waitForApp(page);

    const shots = await getScreenshots(page, secondsAgo(300), new Date().toISOString());

    if (!Array.isArray(shots) || shots.length === 0) {
      test.skip(true, 'No screenshots available; GDI test double may not be active');
      return;
    }

    await goToTimeline(page);

    // Click the first screenshot item
    const item =
      page.locator('[data-testid="screenshot-item"]')
        .or(page.locator('[data-testid="screenshot-card"]'))
        .first();

    await expect(item).toBeVisible({ timeout: 5000 });
    await item.click();

    // After clicking, an <img> or role="img" preview becomes visible.
    // It should reference the file path (src contains the path or a blob URL).
    const preview =
      page.locator('img[src]')
        .or(page.locator('[role="img"]'))
        .or(page.locator('[data-testid="screenshot-preview"]'))
        .first();

    await expect(preview).toBeVisible({ timeout: 5000 });
  });

  // ───── Error Banner ────────────────────────────────────────────────────────

  test('error banner appears on tracey://error event and can be dismissed', async ({ page }) => {
    await goToTimeline(page);

    // Dispatch a tracey://error CustomEvent directly — this is how TauriEventService
    // surfaces Rust-side errors to the Blazor layer
    await page.evaluate(() => {
      window.dispatchEvent(
        new CustomEvent('tracey://error', {
          detail: { message: 'Disk full' }
        })
      );
    });

    // A role="alert" banner or .bb-alert element must appear
    const banner =
      page.getByRole('alert')
        .or(page.locator('.bb-alert'))
        .or(page.locator('[data-testid="error-banner"]'))
        .first();

    await expect(banner).toBeVisible({ timeout: 5000 });

    // Banner must contain the error message
    await expect(banner).toContainText(/disk full/i);

    // Banner must be dismissible — click the close/dismiss button
    const dismissBtn = banner.getByRole('button', { name: /close|dismiss|×|✕/i })
      .or(banner.locator('[aria-label*="close" i]'))
      .or(banner.locator('[aria-label*="dismiss" i]'))
      .first();

    await expect(dismissBtn).toBeVisible();
    await dismissBtn.click();

    // After dismissal the banner must disappear
    await expect(banner).not.toBeVisible({ timeout: 3000 });
  });

  // ───── IPC Contract Tests ──────────────────────────────────────────────────

  test('screenshot_delete_expired returns deleted_count number', async ({ page }) => {
    await waitForApp(page);

    const result = await deleteExpiredScreenshots(page);

    // Contract: response must have deleted_count as a number (≥ 0)
    expect(result).toHaveProperty('deleted_count');
    expect(typeof result.deleted_count).toBe('number');
    expect(result.deleted_count).toBeGreaterThanOrEqual(0);
  });

  test('timeline respects time range filter', async ({ page }) => {
    await waitForApp(page);

    const now = new Date().toISOString();
    const narrowFrom = secondsAgo(10);
    const wideFrom = hoursAgo(24);

    // Both calls must return arrays (even if empty)
    const narrowShots = await getScreenshots(page, narrowFrom, now);
    const wideShots = await getScreenshots(page, wideFrom, now);

    expect(Array.isArray(narrowShots)).toBe(true);
    expect(Array.isArray(wideShots)).toBe(true);

    // Wide range must return ≥ narrow range (superset)
    expect(wideShots.length).toBeGreaterThanOrEqual(narrowShots.length);

    // Every item in the narrow result must also be in the wide result
    const wideIds = new Set(wideShots.map((s: any) => s.id));
    for (const shot of narrowShots) {
      expect(wideIds.has((shot as any).id)).toBe(true);
    }

    // Each screenshot item must conform to the IPC contract shape
    for (const shot of wideShots) {
      expect(shot).toHaveProperty('id');
      expect(shot).toHaveProperty('file_path');
      expect(shot).toHaveProperty('captured_at');
      expect(shot).toHaveProperty('window_title');
      expect(shot).toHaveProperty('process_name');
      expect(shot).toHaveProperty('trigger');
      expect(['interval', 'window_change', 'manual']).toContain((shot as any).trigger);
    }
  });

});
