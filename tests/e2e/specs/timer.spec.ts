import { test, expect, type Page } from '@playwright/test';

/**
 * US1 — Start Tracking Time on a Task
 * T018: All acceptance scenarios from spec.md US1
 *
 * Tests written BEFORE implementation (TDD gate for Phase 3).
 * Must be confirmed FAILING before Reese/Root touch T020+.
 *
 * PREREQUISITE: `cargo tauri dev` must be running, which starts the Blazor
 * WASM dev server at http://localhost:5000 inside the Tauri WebView2 window.
 * These Playwright tests drive Chromium directly at the Blazor dev URL for
 * structural/intent testing. The full native E2E fixture (tauri-driver CDP
 * connection to WebView2) is a CI pipeline concern (Fusco — Phase 3+).
 *
 * Tests FAIL with:
 *   net::ERR_CONNECTION_REFUSED  — when no dev server is running (current state)
 *   TimeoutError                 — when dev server runs but UI not yet implemented
 *
 * IPC commands exercised by these tests (from contracts/ipc-commands.md):
 *   timer_start, timer_stop, timer_get_active, time_entry_list
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

test.describe('US1 — Start Tracking Time on a Task', () => {

  // ─────────────────────────────────────────────────────────────────────────
  // Quick Entry Bar — initial state on launch
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('Quick Entry Bar — Visibility & Focus', () => {

    test('quick-entry bar is visible and focused on launch', async ({ page }) => {
      // Spec: quick-entry bar persistent at top of entry list; focused on launch
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await expect(quickEntry).toBeVisible();
      await expect(quickEntry).toBeFocused();
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // US1 AC1 — Fuzzy Match Dropdown
  // spec: "fuzzy-matches the project and task in real time"
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('AC1 — Fuzzy Match Dropdown', () => {

    test('typing partial project name shows live fuzzy dropdown sorted by match strength', async ({ page }) => {
      // AC1: type project/task/description → live dropdown appears sorted by match strength
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Trc'); // partial, fuzzy match for any project name

      const dropdown = page.getByRole('listbox');
      await expect(dropdown).toBeVisible();
      // At least one option present, sorted by strength (first is best match)
      await expect(dropdown.getByRole('option').first()).toBeVisible();
    });

    test('dropdown matching is case-insensitive and tolerant of minor typos', async ({ page }) => {
      // AC1: "case-insensitively and tolerant of minor spelling differences"
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('tracey'); // fully lowercase

      const dropdown = page.getByRole('listbox');
      await expect(dropdown).toBeVisible();
    });

    test('typing slash after project name locks project segment and shows task suggestions', async ({ page }) => {
      // Spec US5 AC2: first slash → locks project segment, begins fuzzy matching tasks
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('ProjectName/');

      // Tasks dropdown (second-segment matching) appears
      const dropdown = page.getByRole('listbox');
      await expect(dropdown).toBeVisible();

      // Locked project chip visible in entry bar as a confirmed segment
      // (Implementation note: locked chip rendered as role="group" with aria-label="project segment")
      const lockedSegment = page.getByRole('group', { name: /project segment/i })
        .or(page.locator('[aria-label*="project" i][data-locked]'));
      await expect(lockedSegment).toBeVisible();
    });

    test('three-segment input (project/task/description) is parsed unambiguously', async ({ page }) => {
      // Spec US5 AC3: three segments → (project, task, description), no ambiguity
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('ClientA/Bug Fix/Investigate login crash');

      // Both project and task segment chips must be shown
      await expect(page.getByRole('group', { name: /project segment/i })).toBeVisible();
      await expect(page.getByRole('group', { name: /task segment/i })).toBeVisible();
    });

    test('two-segment input (project/description) assigns no task', async ({ page }) => {
      // Spec US5 AC4: two segments → (project, description), task remains null
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('ClientA/Write summary report');

      // No task segment chip present
      await expect(page.getByRole('group', { name: /task segment/i })).not.toBeVisible();
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // US1 AC2 — Segment Confirmation with Tab / Enter
  // spec: "Tab or Enter confirms the segment and cursor moves to next segment"
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('AC2 — Segment Confirmation (Tab / Enter)', () => {

    test('Tab on highlighted dropdown match confirms project segment and dismisses dropdown', async ({ page }) => {
      // AC2: Tab on match → segment confirmed, cursor moves to next
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Trc');

      await expect(page.getByRole('listbox')).toBeVisible();
      await quickEntry.press('Tab');

      await expect(page.getByRole('listbox')).not.toBeVisible();
    });

    test('Enter on highlighted dropdown match confirms segment and dismisses dropdown', async ({ page }) => {
      // AC2: Enter on match → segment confirmed (same outcome as Tab)
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Trc');

      const dropdown = page.getByRole('listbox');
      await expect(dropdown).toBeVisible();

      await dropdown.getByRole('option').first().press('Enter');

      await expect(dropdown).not.toBeVisible();
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // US1 AC3 — Timer Start / Auto-Stop Running Timer
  // spec: "new timer starts; previously running timer automatically stopped and saved"
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('AC3 — Timer Start and Auto-Stop', () => {

    test('pressing Enter on complete entry starts timer — elapsed display shows 0:00', async ({ page }) => {
      // AC3: complete entry → timer starts; role="timer" element appears
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Writing unit tests');
      await quickEntry.press('Enter');

      const timerDisplay = page.getByRole('timer');
      await expect(timerDisplay).toBeVisible();
      await expect(timerDisplay).toContainText(/0:00/);
    });

    test('starting a second entry auto-stops and saves the first running timer', async ({ page }) => {
      // AC3: "previously running timer is automatically stopped and saved"
      // IPC: timer_start "Automatically stops and saves any currently running timer"
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('First task');
      await quickEntry.press('Enter');

      await expect(page.getByRole('timer')).toBeVisible();
      await page.waitForTimeout(1100); // ensure first entry gets a non-zero duration

      await quickEntry.fill('Second task');
      await quickEntry.press('Enter');

      // Navigate to Timeline — first entry must appear saved
      await page.getByRole('link', { name: /timeline/i }).click();
      await expect(page.getByText('First task')).toBeVisible();
    });

    test('timer elapsed display updates every second', async ({ page }) => {
      // Spec: "Timer shows elapsed time updating every second"
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Elapsed tick check');
      await quickEntry.press('Enter');

      const timerDisplay = page.getByRole('timer');
      const initialText = await timerDisplay.textContent();

      await page.waitForTimeout(2100); // allow ≥ 2 tick cycles (tracey://timer-tick events)

      const updatedText = await timerDisplay.textContent();
      expect(updatedText).not.toBe(initialText);
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // US1 AC4 — Timer Stop & Time Entry Persistence
  // spec: "saved with correct start and end datetimes in UTC, displayed in local timezone"
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('AC4 — Timer Stop and TimeEntry Persistence', () => {

    test('stopping the timer creates a TimeEntry visible in Timeline', async ({ page }) => {
      // AC4: stop → TimeEntry saved with correct start/end UTC datetimes
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Review PR for auth fix');
      await quickEntry.press('Enter');

      await page.waitForTimeout(1100);
      await page.getByRole('button', { name: /stop/i }).click();

      await page.getByRole('link', { name: /timeline/i }).click();
      await expect(page.getByText('Review PR for auth fix')).toBeVisible();
    });

    test('stopped entry card shows a non-zero duration', async ({ page }) => {
      // AC4: start/end datetimes differ → duration displayed as human-readable string
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Duration display check');
      await quickEntry.press('Enter');

      await page.waitForTimeout(1100);
      await page.getByRole('button', { name: /stop/i }).click();

      await page.getByRole('link', { name: /timeline/i }).click();

      // Entry card renders duration in a <time> element or labelled element
      const entryCard = page.getByRole('article').filter({ hasText: 'Duration display check' });
      const durationEl = entryCard.getByRole('time').or(entryCard.getByLabel(/duration/i));

      await expect(durationEl.first()).toBeVisible();
      await expect(durationEl.first()).not.toContainText(/^0:00$/);
    });

    test('stopped entry time display is in local timezone — not raw UTC string', async ({ page }) => {
      // AC4: "displayed in the user's configured local timezone"
      // The raw ISO UTC string (e.g. "2026-03-15T09:00:00Z") must NEVER be shown
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Timezone format check');
      await quickEntry.press('Enter');

      await page.waitForTimeout(500);
      await page.getByRole('button', { name: /stop/i }).click();

      await page.getByRole('link', { name: /timeline/i }).click();

      const entryCard = page.getByRole('article').filter({ hasText: 'Timezone format check' });
      const timeDisplay = entryCard.getByRole('time').first();

      await expect(timeDisplay).toBeVisible();
      // Must NOT render raw UTC ISO 8601 string to the user
      await expect(timeDisplay).not.toContainText(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z/);
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // US1 AC5 — Continue (restart a past entry)
  // spec: "new timer starts from current time; description/project/task copied"
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('AC5 — Continue Past Entry', () => {

    test('clicking Continue on a past entry starts a new timer from current time', async ({ page }) => {
      // AC5: Continue → new timer starts using current time as start datetime
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Past task to continue');
      await quickEntry.press('Enter');

      await page.waitForTimeout(500);
      await page.getByRole('button', { name: /stop/i }).click();

      await page.getByRole('link', { name: /timeline/i }).click();
      await page.getByRole('button', { name: /continue/i }).first().click();

      // New timer is running; elapsed resets to 0:00
      const timerDisplay = page.getByRole('timer');
      await expect(timerDisplay).toBeVisible();
      await expect(timerDisplay).toContainText(/0:00/);
    });

    test('continued timer copies description from the original entry', async ({ page }) => {
      // AC5: "same description, project, and task copied from the original entry"
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Task to copy on continue');
      await quickEntry.press('Enter');

      await page.waitForTimeout(500);
      await page.getByRole('button', { name: /stop/i }).click();

      await page.getByRole('link', { name: /timeline/i }).click();
      await page.getByRole('button', { name: /continue/i }).first().click();

      // Quick-entry bar reflects the copied description from the continued entry
      await expect(quickEntry).toHaveValue('Task to copy on continue');
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // Keyboard Shortcuts — Ctrl+Space
  // spec: "Keyboard shortcut Ctrl+Space starts/stops timer"
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('Keyboard Shortcuts — Ctrl+Space', () => {

    test('Ctrl+Space with no running timer focuses the quick-entry bar', async ({ page }) => {
      // Spec US1: no timer running → Ctrl+Space focuses/opens the quick-entry bar
      await waitForApp(page);

      await page.keyboard.press('Control+Space');

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await expect(quickEntry).toBeFocused();
    });

    test('Ctrl+Space stops a running timer', async ({ page }) => {
      // Spec US1: timer running → Ctrl+Space stops it
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Keyboard shortcut stop test');
      await quickEntry.press('Enter');

      const timerDisplay = page.getByRole('timer');
      await expect(timerDisplay).toBeVisible();

      await page.keyboard.press('Control+Space');

      // Timer indicator disappears when stopped
      await expect(timerDisplay).not.toBeVisible();
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // Running Timer State — UI during active tracking
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('Running Timer State UI', () => {

    test('description input shows current description and is editable while timer is running', async ({ page }) => {
      // Spec: "If timer is running, description input shows current description (editable)"
      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Original description');
      await quickEntry.press('Enter');

      // Quick-entry bar must reflect the running timer's description
      await expect(quickEntry).toHaveValue('Original description');

      // User can edit description inline while the timer is running
      await quickEntry.fill('Edited mid-run description');
      await expect(quickEntry).toHaveValue('Edited mid-run description');
    });

  });

  // ─────────────────────────────────────────────────────────────────────────
  // IPC Error Handling
  // (intent documented; automated form requires Phase 3+ IPC override fixture)
  // ─────────────────────────────────────────────────────────────────────────

  test.describe('IPC Error Handling', () => {

    test('timer_start IPC failure shows accessible error indicator near quick-entry bar', async ({ page }) => {
      // MANUAL TEST — intent documentation only.
      // Requires a Playwright IPC override fixture to inject a failing Tauri invoke.
      // Automate once the tauri-driver CDP fixture is wired up (Fusco — Phase 3+ CI).
      //
      // Expected behaviour: when timer_start returns { error: "invalid_description" },
      // an accessible error element (role="alert") appears in or adjacent to the
      // quick-entry bar so keyboard-only users are informed.
      test.skip(true, [
        'Requires tauri-driver IPC override fixture (Phase 3+ CI concern).',
        'Manual step: mock window.__TAURI_INTERNALS__.invoke to reject with',
        '{ error: "invalid_description" }, then verify role="alert" is visible.',
      ].join(' '));

      await waitForApp(page);

      await page.addInitScript(() => {
        (window as any).__TAURI_INTERNALS__ = {
          invoke: async (_cmd: string, _args: unknown) => {
            throw { error: 'invalid_description' };
          },
        };
      });

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill(''); // empty description → invalid_description IPC error
      await quickEntry.press('Enter');

      await expect(page.getByRole('alert')).toBeVisible();
    });

  });

});
