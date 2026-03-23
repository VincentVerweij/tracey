import { test, expect, type Page } from '@playwright/test';

/**
 * Timeline Bug Regression Tests
 *
 * Tests written BEFORE implementation (TDD gate).
 * Tests cover:
 *   - Bug 3: Timeline doesn't auto-update after new screenshots
 *            TauriEventService.Listen<T> is a TODO stub; InitializeAsync never called.
 *            Timeline.razor doesn't subscribe to OnScreenshotCaptured.
 *   - Bug 6: Window Change screenshots show broken image icon
 *            GetImgSrc generates wrong URL format.
 *            Tauri v2 / WebView2 expects https://asset.localhost/C%3A/path/to/file.jpg
 *            but current code produces asset://localhost/C:/path/to/file.jpg
 *
 * Tests FAIL with:
 *   net::ERR_CONNECTION_REFUSED  — when no dev server is running (current state)
 *   TimeoutError                 — when dev server runs but UI not yet implemented
 *
 * IPC commands exercised:
 *   screenshot_list, preferences_update
 *
 * Build requirement: Bug 3 capture-dependent test requires --features test build
 * (activates GDI test double — writes a pre-canned JPEG instead of calling Win32).
 * Tests skip gracefully if no screenshots are produced.
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
// IPC Fixture Helpers
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

function secondsAgo(n: number): string {
  return new Date(Date.now() - n * 1000).toISOString();
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

test.describe('Timeline Bug Regressions', () => {

  test.describe.configure({ mode: 'serial' });

  // ───────────────────────────────────────────────────────────────────────────
  // Bug 3 — Timeline doesn't auto-update after new screenshots
  // ───────────────────────────────────────────────────────────────────────────

  test('timeline shows new screenshot without page navigation', async ({ page }) => {
    // Bug 3: TauriEventService.Listen<T> is a TODO stub — InitializeAsync is never called.
    //        Timeline.razor doesn't subscribe to OnScreenshotCaptured, so no reactive update.
    // BEFORE FIX: user must navigate away and back to see new screenshots
    // AFTER FIX:  screenshot items appear reactively while the user stays on /timeline
    //
    // NOTE: requires --features test build (GDI test double).
    //       Skips gracefully if no screenshots produced within 8 seconds.
    await waitForApp(page);

    // Set a very short interval so the test double fires quickly
    try {
      await setScreenshotInterval(page, 3);
    } catch {
      test.skip(true, 'IPC unavailable — app not running or not built with --features test');
      return;
    }

    // Navigate to /timeline BEFORE any screenshot is captured and stay on the page
    await goToTimeline(page);

    // Wait for capture cycle(s) to fire (interval=3s, wait=5s → at least one capture)
    await page.waitForTimeout(5000);

    // Guard: verify via IPC whether anything was actually captured
    const shots = await getScreenshots(page, secondsAgo(30), new Date().toISOString()).catch(() => []);

    if (!Array.isArray(shots) || shots.length === 0) {
      // GDI test double is not active — skip rather than false-fail
      test.skip(true, 'No screenshots captured after 8s wait; GDI test double may not be active');
      return;
    }

    // BEFORE FIX: empty state still showing (no reactive update without navigation)
    // AFTER FIX:  screenshot item visible WITHOUT navigating away and back
    const screenshotItem =
      page.locator('[data-testid="screenshot-item"]')
        .or(page.locator('[data-testid="screenshot-card"]'))
        .first();

    await expect(screenshotItem).toBeVisible({ timeout: 5000 });
  });

  // ───────────────────────────────────────────────────────────────────────────
  // Bug 6 — Screenshot images show broken icon (wrong asset URL scheme)
  // ───────────────────────────────────────────────────────────────────────────

  test('screenshot image loads without broken icon', async ({ page }) => {
    // Bug 6: GetImgSrc builds the wrong URL scheme for Tauri v2 on Windows/WebView2.
    //        Current (broken): asset://localhost/C:/path/to/file.jpg
    //        Required (fixed):  https://asset.localhost/C%3A/path/to/file.jpg
    // BEFORE FIX: img naturalWidth === 0 (browser cannot resolve the asset:// scheme)
    // AFTER FIX:  img naturalWidth > 0  (https://asset.localhost/ loads correctly)
    //
    // NOTE: requires at least one screenshot to exist.
    //       Skips gracefully if screenshot_list returns empty.
    await goToTimeline(page);

    // Guard: check whether any screenshots exist to test against
    const shots = await getScreenshots(page, secondsAgo(3600), new Date().toISOString()).catch(() => []);

    if (!Array.isArray(shots) || shots.length === 0) {
      test.skip(true, 'No screenshots available; run with --features test and wait for a capture cycle');
      return;
    }

    // The screenshot card / item must render an <img> element for the preview thumbnail.
    // Root may use .screenshot-img or an <img> nested inside the card element.
    const screenshotImg =
      page.locator('[data-testid="screenshot-item"] img, [data-testid="screenshot-card"] img')
        .or(page.locator('.screenshot-img'))
        .first();

    await expect(screenshotImg).toBeVisible({ timeout: 5000 });

    // Evaluate naturalWidth — browser sets this to 0 when the image URL fails to load
    const naturalWidth = await screenshotImg.evaluate(
      (el: HTMLImageElement) => el.naturalWidth
    ).catch(() => -1);

    // BEFORE FIX: naturalWidth === 0 (asset:// URL not resolved under WebView2)
    // AFTER FIX:  naturalWidth  > 0 (https://asset.localhost/C%3A/... loads correctly)
    expect(naturalWidth).toBeGreaterThan(0);
  });

});
