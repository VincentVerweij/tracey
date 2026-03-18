import { test, expect, type Page } from '@playwright/test';

/**
 * US2 — Idle Detection and On-Return Prompt
 * T031 Phase 4: Full acceptance scenario coverage
 *
 * Tests drive the full Tauri + Blazor WASM stack via Chromium at the Blazor dev URL.
 * Idle detection is triggered by genuinely waiting > inactivity_timeout_seconds.
 * Threshold is set to 5 s via IPC to keep test duration manageable (Option A).
 *
 * Event bridge uses Tauri plugin:event|listen — DOM dispatchEvent does NOT reach
 * the C# RouteEvent handler, so direct event injection (Option B) is not viable.
 *
 * IPC commands exercised:
 *   preferences_update, timer_start, timer_stop, timer_get_active, time_entry_list
 */

const APP_URL = 'http://localhost:5000';

/** Navigate to the app root and wait for Blazor WASM to hydrate. */
async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

/** Set inactivity_timeout_seconds via preferences_update IPC. */
async function setInactivityTimeout(page: Page, seconds: number): Promise<void> {
  await page.evaluate(async (s) => {
    await (window as any).__TAURI_INTERNALS__.invoke('preferences_update', {
      update: { inactivity_timeout_seconds: s },
    });
  }, seconds);
}

/** Start a timer via IPC. Returns the new entry id. */
async function startTimer(page: Page, description: string): Promise<string> {
  const result = await page.evaluate(async (desc) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('timer_start', {
      request: { description: desc, project_id: null, task_id: null, tag_ids: [] },
    });
  }, description);
  return result.id;
}

/** Stop any running timer, silently ignoring "no_active_timer" errors. */
async function stopAnyTimer(page: Page): Promise<void> {
  try {
    await page.evaluate(async () => {
      await (window as any).__TAURI_INTERNALS__.invoke('timer_stop');
    });
  } catch { /* no active timer — expected */ }
}

/** Wait long enough for idle to be detected (threshold + poll interval). */
const IDLE_THRESHOLD_SECONDS = 5;
const WAIT_FOR_IDLE_MS = (IDLE_THRESHOLD_SECONDS + 5) * 1_000; // 10 s

test.describe('US2 — Idle Detection and On-Return Prompt', () => {

  test.describe.configure({ mode: 'serial' });

  test.afterEach(async ({ page }) => {
    // Restore a safe timeout and clean up any running timer between tests.
    await setInactivityTimeout(page, 300).catch(() => {});
    await stopAnyTimer(page).catch(() => {});
  });

  // ─────────────────────────────────────────────────────────────────────────
  // No-timer guard (Decision 2026-03-15: no modal when no active timer)
  // ─────────────────────────────────────────────────────────────────────────

  test('idle modal does NOT appear when no timer is running', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);

    // Wait longer than the detection period
    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).not.toBeVisible();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // AS1 — Modal appears with all four option buttons
  // ─────────────────────────────────────────────────────────────────────────

  test('idle modal appears after inactivity with all four option buttons (AS1)', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Test coding session');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });
    await expect(modal.getByText("You're back")).toBeVisible();

    // All four option buttons present
    await expect(modal.getByRole('button', { name: /break/i })).toBeVisible();
    await expect(modal.getByRole('button', { name: /meeting/i })).toBeVisible();
    await expect(modal.getByRole('button', { name: /specify/i })).toBeVisible();
    await expect(modal.getByRole('button', { name: /keep/i })).toBeVisible();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // AS2 — "Keep": modal closed, original timer still running, no new entry
  // ─────────────────────────────────────────────────────────────────────────

  test('"Keep" dismisses modal and leaves the original timer running (AS2)', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Keep running task');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await modal.getByRole('button', { name: /keep/i }).click();

    await expect(modal).not.toBeVisible({ timeout: 3_000 });

    // Timer must still be active
    const active = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('timer_get_active');
    });
    expect(active).not.toBeNull();
    expect(active.description).toBe('Keep running task');
  });

  // ─────────────────────────────────────────────────────────────────────────
  // AS3 — "Break": stops timer at idle start, creates a Break entry
  // ─────────────────────────────────────────────────────────────────────────

  test('"Break" stops timer and creates a Break time entry (AS3)', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Deep work before break');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await modal.getByRole('button', { name: /break/i }).click();

    await expect(modal).not.toBeVisible({ timeout: 3_000 });

    // No active timer after break resolution
    const active = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('timer_get_active');
    });
    expect(active).toBeNull();

    // A "Break" entry must appear in the entry list (is_break: true)
    const list = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('time_entry_list', {
        request: { page: 1, page_size: 20 },
      });
    });
    const breakEntry = list.entries.find((e: any) => e.description === 'Break');
    expect(breakEntry).toBeDefined();
    expect(breakEntry.is_break).toBe(true);
  });

  // ─────────────────────────────────────────────────────────────────────────
  // AS4 — "Meeting": stops timer at idle start, creates a Meeting entry
  // ─────────────────────────────────────────────────────────────────────────

  test('"Meeting" stops timer and creates a Meeting time entry (AS4)', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Pre-meeting work');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await modal.getByRole('button', { name: /meeting/i }).click();

    await expect(modal).not.toBeVisible({ timeout: 3_000 });

    // No active timer
    const active = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('timer_get_active');
    });
    expect(active).toBeNull();

    // A "Meeting" entry must appear (is_break: false)
    const list = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('time_entry_list', {
        request: { page: 1, page_size: 20 },
      });
    });
    const meetingEntry = list.entries.find((e: any) => e.description === 'Meeting');
    expect(meetingEntry).toBeDefined();
    expect(meetingEntry.is_break).toBe(false);
  });

  // ─────────────────────────────────────────────────────────────────────────
  // AS5 — "Specify": inline input appears, saves custom-description entry
  // ─────────────────────────────────────────────────────────────────────────

  test('"Specify" shows inline input and saves a custom-description entry (AS5)', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Work before specifying');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    // Clicking "Specify" shows inline input — does NOT immediately resolve
    await modal.getByRole('button', { name: /specify/i }).click();

    const specifyInput = page.getByRole('textbox', { name: /what were you doing/i });
    await expect(specifyInput).toBeVisible({ timeout: 2_000 });

    await specifyInput.fill('Reviewing architecture docs');
    await page.getByRole('button', { name: 'Save' }).click();

    await expect(modal).not.toBeVisible({ timeout: 3_000 });

    // Custom entry must appear in the entry list with exact description
    const list = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('time_entry_list', {
        request: { page: 1, page_size: 20 },
      });
    });
    const customEntry = list.entries.find(
      (e: any) => e.description === 'Reviewing architecture docs',
    );
    expect(customEntry).toBeDefined();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // AS6 — Threshold respected: modal does NOT appear before threshold elapses
  // ─────────────────────────────────────────────────────────────────────────

  test('modal does not appear before the configured threshold elapses', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);

    // Five-minute threshold — modal must not appear in a 3-second wait
    await setInactivityTimeout(page, 300);
    await startTimer(page, 'Long-threshold task');

    await page.waitForTimeout(3_000);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).not.toBeVisible();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Specify sub-flow — Enter key submits and closes modal
  // ─────────────────────────────────────────────────────────────────────────

  test('"Specify" Enter key submits the description and closes the modal', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Work before specify-enter test');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await modal.getByRole('button', { name: /specify/i }).click();

    const specifyInput = page.getByRole('textbox', { name: /what were you doing/i });
    await expect(specifyInput).toBeVisible({ timeout: 2_000 });

    await specifyInput.fill('Design review via keyboard');
    await specifyInput.press('Enter');

    await expect(modal).not.toBeVisible({ timeout: 3_000 });

    // Entry must appear in the list with the typed description
    const list = await page.evaluate(async () => {
      return await (window as any).__TAURI_INTERNALS__.invoke('time_entry_list', {
        request: { page: 1, page_size: 20 },
      });
    });
    const entry = list.entries.find(
      (e: any) => e.description === 'Design review via keyboard',
    );
    expect(entry).toBeDefined();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Specify sub-flow — empty input shows validation error, modal stays open
  // ─────────────────────────────────────────────────────────────────────────

  test('"Specify" Save without text shows error and keeps modal open', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Work before empty-specify test');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await modal.getByRole('button', { name: /specify/i }).click();

    const specifyInput = page.getByRole('textbox', { name: /what were you doing/i });
    await expect(specifyInput).toBeVisible({ timeout: 2_000 });

    // Submit with empty input
    await modal.getByRole('button', { name: /save/i }).click();

    // Validation error must appear
    const errorMsg = modal.getByRole('alert');
    await expect(errorMsg).toBeVisible({ timeout: 2_000 });
    await expect(errorMsg).toHaveText(/please describe what you were doing/i);

    // Modal must still be visible — not dismissed
    await expect(modal).toBeVisible();
    await expect(specifyInput).toBeVisible();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Specify sub-flow — Back button returns to the four-option view
  // ─────────────────────────────────────────────────────────────────────────

  test('"Specify" Back button returns to the four-option view', async ({ page }) => {
    await waitForApp(page);
    await stopAnyTimer(page);
    await setInactivityTimeout(page, IDLE_THRESHOLD_SECONDS);
    await startTimer(page, 'Work before back-button test');

    await page.waitForTimeout(WAIT_FOR_IDLE_MS);

    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await modal.getByRole('button', { name: /specify/i }).click();

    // Specify input view is now shown
    const specifyInput = page.getByRole('textbox', { name: /what were you doing/i });
    await expect(specifyInput).toBeVisible({ timeout: 2_000 });

    // Click Back
    await modal.getByRole('button', { name: /back/i }).click();

    // Input view is gone; four option buttons reappear
    await expect(specifyInput).not.toBeVisible({ timeout: 2_000 });
    await expect(modal.getByRole('button', { name: /break/i })).toBeVisible();
    await expect(modal.getByRole('button', { name: /meeting/i })).toBeVisible();
    await expect(modal.getByRole('button', { name: /specify/i })).toBeVisible();
    await expect(modal.getByRole('button', { name: /keep/i })).toBeVisible();
  });

});
