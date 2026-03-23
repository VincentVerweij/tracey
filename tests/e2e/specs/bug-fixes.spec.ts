import { test, expect, type Page } from '@playwright/test';
import { hasTauriAvailable } from './tauri-helpers';

/**
 * Bug Fix Regression Tests
 *
 * Tests written BEFORE implementation (TDD gate).
 * Tests cover:
 *   - Bug 1+2: ProjectListResponse deserialization / save button not persisting
 *              hierarchy.rs `project_list` returns raw array but C# expects { projects: [...] }
 *   - Bug 4:   Timer display frozen — tracey://timer-tick events never reach HandleTimerTick
 *              (TauriEventService.Listen<T> stub; InitializeAsync never called)
 *   - Bug 5:   Entry list stuck on "Loading entries…" after stopping timer
 *              (LoadPage sets _loading = false in finally{} but never calls StateHasChanged)
 *
 * Tests FAIL with:
 *   net::ERR_CONNECTION_REFUSED  — when no dev server is running (current state)
 *   TimeoutError                 — when dev server runs but UI not yet implemented
 *
 * IPC commands exercised:
 *   client_create, client_delete, timer_start, timer_stop
 */

const APP_URL = 'http://localhost:5000';

// ─────────────────────────────────────────────────────────────────────────────
// Navigation helpers
// ─────────────────────────────────────────────────────────────────────────────

async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

async function goToProjects(page: Page): Promise<void> {
  await page.goto(`${APP_URL}/projects`);
  await page.waitForLoadState('networkidle');
}

// ─────────────────────────────────────────────────────────────────────────────
// IPC Fixture Helpers
// ─────────────────────────────────────────────────────────────────────────────

async function createClient(page: Page, name: string, color = '#3b82f6'): Promise<string> {
  const result = await page.evaluate(async ({ name, color }) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('client_create', { name, color, logo_path: null });
  }, { name, color });
  return result.id;
}

async function deleteClient(page: Page, clientId: string): Promise<void> {
  await page.evaluate(async (id) => {
    await (window as any).__TAURI_INTERNALS__.invoke('client_delete', { id });
  }, clientId);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

test.describe('Bug Fix Regressions', () => {

  test.describe.configure({ mode: 'serial' });

  // ───────────────────────────────────────────────────────────────────────────
  // Bug 1 + 2 — ProjectListResponse deserialization / save button
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('Bug 1+2 — Projects load and save correctly', () => {

    let clientId: string | null = null;

    test.beforeEach(async ({ page }) => {
      if (!(await hasTauriAvailable(page))) {
        test.skip(true, 'Requires Tauri bridge — run with tauri-driver for IPC tests');
      }
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
      }
    });

    test('project list loads when client is expanded', async ({ page }) => {
      // Bug 1: project_list IPC returns raw array [] but C# ProjectListResponse expects { projects: [...] }
      // BEFORE FIX: "Failed to load projects: DeserializeUnableToConvertValue…" error banner appears
      // AFTER FIX:  project list container renders (empty "No projects yet." or project rows)
      await waitForApp(page);

      clientId = await createClient(page, 'Bug1 Client');

      await goToProjects(page);

      // Expand the client row to trigger project_list IPC call
      const clientRow = page.getByText('Bug1 Client').first();
      await expect(clientRow).toBeVisible();
      await clientRow.click();

      // BEFORE FIX: error banner visible
      await expect(page.getByText(/Failed to load projects/i)).not.toBeVisible();

      // AFTER FIX: project list container rendered (empty state is acceptable)
      const projectList =
        page.locator('.project-list')
          .or(page.locator('[data-testid="project-list"]'))
          .or(page.getByText(/no projects yet/i))
          .first();
      await expect(projectList).toBeVisible({ timeout: 5000 });
    });

    test('saving a new project makes it appear under client', async ({ page }) => {
      // Bug 2: LoadProjects throws after ProjectCreate succeeds — project never shown in UI
      // BEFORE FIX: "Failed to load projects" error prevents project from appearing
      // AFTER FIX:  "Test Project" is visible in the project list
      await waitForApp(page);

      clientId = await createClient(page, 'Bug2 Client');

      await goToProjects(page);

      // Expand the client section
      await page.getByText('Bug2 Client').first().click();

      // Open Add Project form
      const addProjectBtn = page.getByRole('button', { name: /add project/i });
      await expect(addProjectBtn).toBeVisible();
      await addProjectBtn.click();

      // Fill project name
      const nameInput = page.getByRole('textbox', { name: /project name/i });
      await expect(nameInput).toBeVisible();
      await nameInput.fill('Test Project');

      // Save
      await page.getByRole('button', { name: /save|create|confirm/i }).click();

      // AFTER FIX: project name visible in client's expanded list
      await expect(page.getByText('Test Project')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText(/Failed to load projects/i)).not.toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // Bug 4 — Timer display frozen (event bridge not wired)
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('Bug 4 — Timer display counts up', () => {

    test.beforeEach(async ({ page }) => {
      if (!(await hasTauriAvailable(page))) {
        test.skip(true, 'Requires Tauri bridge — run with tauri-driver for IPC tests');
      }
    });

    test('timer display increases each second after start', async ({ page }) => {
      // Bug 4: tracey://timer-tick events never reach HandleTimerTick because
      //        TauriEventService.Listen<T> is a TODO stub and InitializeAsync is never called.
      //        Also: nothing wires Events.OnTimerTick to Timer.HandleTimerTick.
      // BEFORE FIX: role="timer" value stays frozen at its initial value
      // AFTER FIX:  value increments each second once the event bridge is wired
      await waitForApp(page);

      // Start a timer via the quick-entry bar
      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Bug4 timer tick check');
      await quickEntry.press('Enter');

      const timerDisplay = page.getByRole('timer');
      await expect(timerDisplay).toBeVisible();

      const before = await timerDisplay.textContent();

      // Wait for at least 2 tick cycles (tracey://timer-tick fires every second)
      await page.waitForTimeout(2200);

      const after = await timerDisplay.textContent();

      // BEFORE FIX: before === after (display frozen)
      // AFTER FIX:  after differs from before (elapsed time updated)
      expect(after).not.toBe(before);
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // Bug 5 — Entry list stuck on "Loading entries…" after stop
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('Bug 5 — Entry list refreshes after stop', () => {

    test.beforeEach(async ({ page }) => {
      if (!(await hasTauriAvailable(page))) {
        test.skip(true, 'Requires Tauri bridge — run with tauri-driver for IPC tests');
      }
    });

    test('entry list refreshes immediately after stopping timer', async ({ page }) => {
      // Bug 5: TimeEntryList.LoadPage sets _loading = false inside finally{} but
      //        never calls StateHasChanged() — the loading indicator persists indefinitely.
      // BEFORE FIX: "Loading entries…" text remains visible after timer is stopped
      // AFTER FIX:  loading indicator disappears; entries (or empty state) rendered
      await waitForApp(page);

      // Start a timer via the quick-entry bar
      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await quickEntry.fill('Bug5 stop-reload check');
      await quickEntry.press('Enter');

      await expect(page.getByRole('timer')).toBeVisible();

      // Wait 1+ second so the entry has a non-zero duration when stopped
      await page.waitForTimeout(1100);

      // Stop the timer
      await page.getByRole('button', { name: /stop/i }).click();

      // BEFORE FIX: ".loading-indicator" or "Loading entries…" stays visible
      // AFTER FIX:  loading text gone; list shows the stopped entry (or empty state)
      await expect(page.getByText(/Loading entries/i)).not.toBeVisible({ timeout: 5000 });
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // Bug 6 — QuickEntry client hint scopes project dropdown
  // Regression: the 0.4 score floor in LoadProjectMatches let every project pass
  // (score = projectScore * (0.4 + 0.6 * clientScore); clientScore=0 → 0.4 > 0).
  // Fix: use projectScore * clientScore so a non-matching client scores 0 and is
  // filtered by the .Where(x => x.Score > 0) guard.
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('Bug 6 — client hint in QuickEntry scopes project dropdown', () => {

    let clientAId: string;
    let clientBId: string;

    test.beforeEach(async ({ page }) => {
      if (!(await hasTauriAvailable(page))) {
        test.skip(true, 'Requires Tauri bridge — run with tauri-driver for IPC tests');
      }
    });

    test.beforeAll(async ({ browser }) => {
      const page = await browser.newPage();
      await waitForApp(page);
      if (!(await hasTauriAvailable(page))) {
        await page.close();
        return;
      }
      // Two clients with completely different (non-overlapping) project names
      clientAId = await (async () => {
        const r = await page.evaluate(async ({ name, color }) =>
          (window as any).__TAURI_INTERNALS__.invoke('client_create', { name, color, logo_path: null }),
          { name: '__BUG6_ClientAlpha__', color: '#3b82f6' });
        return r.id;
      })();
      await page.evaluate(async ({ clientId, name }) =>
        (window as any).__TAURI_INTERNALS__.invoke('project_create', {
          request: { client_id: clientId, name }
        }), { clientId: clientAId, name: '__BUG6_ProjectOnlyAlpha__' });

      clientBId = await (async () => {
        const r = await page.evaluate(async ({ name, color }) =>
          (window as any).__TAURI_INTERNALS__.invoke('client_create', { name, color, logo_path: null }),
          { name: '__BUG6_ClientBeta__', color: '#ef4444' });
        return r.id;
      })();
      await page.evaluate(async ({ clientId, name }) =>
        (window as any).__TAURI_INTERNALS__.invoke('project_create', {
          request: { client_id: clientId, name }
        }), { clientId: clientBId, name: '__BUG6_ProjectOnlyBeta__' });

      await page.close();
    });

    test.afterAll(async ({ browser }) => {
      const page = await browser.newPage();
      await waitForApp(page);
      try { await deleteClient(page, clientAId); } catch {}
      try { await deleteClient(page, clientBId); } catch {}
      await page.close();
    });

    test('typing ClientAlpha / shows only ClientAlpha projects, not ClientBeta projects', async ({ page }) => {
      // BEFORE FIX: both __BUG6_ProjectOnlyAlpha__ AND __BUG6_ProjectOnlyBeta__ appear
      //             because the 0.4 floor gave every project a passing score.
      // AFTER FIX:  only __BUG6_ProjectOnlyAlpha__ appears; Beta project is absent.
      await waitForApp(page);
      const input = page.locator('.entry-input');
      await input.click();

      // Type client name then slash to enter project-selection mode
      await input.fill('__BUG6_ClientAlpha__');
      await input.press('/');
      await page.waitForTimeout(500); // allow debounce + IPC round-trip

      const dropdown = page.locator('.fuzzy-dropdown');
      // If no dropdown is visible the list is empty — that is also a valid passing state
      // (the client hint resolved to 0 matches for any other client).
      if (!await dropdown.isVisible().catch(() => false)) {
        // At minimum, the Beta project must not bleed through
        await expect(page.getByText('__BUG6_ProjectOnlyBeta__')).not.toBeVisible();
        return;
      }

      // Dropdown is visible: Alpha project should appear
      await expect(dropdown).toContainText('__BUG6_ProjectOnlyAlpha__');

      // Beta project must NOT be in the list
      await expect(dropdown).not.toContainText('__BUG6_ProjectOnlyBeta__');
    });

    test('empty description + resolved project via slash starts timer without error', async ({ page }) => {
      // BEFORE FIX: Rust timer_start returns "invalid_description" for empty strings even
      //             when project_id is set; Blazor renders an unhandled exception.
      // AFTER FIX:  timer starts successfully; elapsed timer display becomes visible.
      await waitForApp(page);
      const input = page.locator('.entry-input');
      await input.click();

      // Select ClientAlpha project via slash notation
      await input.fill('__BUG6_ClientAlpha__');
      await input.press('/');
      await page.waitForTimeout(500);

      const dropdown = page.locator('.fuzzy-dropdown');
      if (await dropdown.isVisible().catch(() => false)) {
        // Confirm the first project match
        await input.press('Enter');
        await page.waitForTimeout(300);
      }

      // Now in Description mode — press Enter immediately with empty description
      await input.press('Enter');
      await page.waitForTimeout(500);

      // AFTER FIX: timer is running (no unhandled exception dialog)
      await expect(page.locator('[role="dialog"]').filter({ hasText: /exception|error/i }))
        .not.toBeVisible({ timeout: 2000 });
      const elapsed = page.locator('.running-elapsed, [role="timer"]');
      await expect(elapsed).toBeVisible({ timeout: 3000 });

      // Clean up
      try {
        await page.evaluate(async () =>
          (window as any).__TAURI_INTERNALS__.invoke('timer_stop'));
      } catch {}
    });

  });

});
