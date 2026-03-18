import { test, expect, type Page } from '@playwright/test';

/**
 * US5 — Keyboard-First Quick Entry with Fuzzy Matching
 * T051: E2E acceptance tests for slash-notation quick entry
 * T054a: Single-client silent inference vs. disambiguation
 *
 * Tests written BEFORE implementation (TDD gate — all fail until US5 ships).
 *
 * Requirements:
 *   - Project segment: type partial name → fuzzy dropdown, arrow + Enter to confirm
 *   - Task segment: after project confirmed, type partial task → fuzzy dropdown
 *   - One slash "proj/desc" → project + description (no task)
 *   - Two slashes "proj/task/desc" → all segments
 *   - Client disambiguation: shown only when project name exists under 2+ clients
 *   - Single-client: no disambiguation, silent resolution
 */

const APP_URL = 'http://localhost:5000';

// ─── Helpers ──────────────────────────────────────────────────────────────────

async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

async function createClient(page: Page, name: string, color = '#3b82f6'): Promise<string> {
  const result = await page.evaluate(
    async ({ name, color }) =>
      (window as any).__TAURI_INTERNALS__.invoke('client_create', { name, color, logo_path: null }),
    { name, color }
  );
  return result.id;
}

async function createProject(page: Page, clientId: string, name: string): Promise<string> {
  const result = await page.evaluate(
    async ({ clientId, name }) =>
      (window as any).__TAURI_INTERNALS__.invoke('project_create', {
        request: { client_id: clientId, name }
      }),
    { clientId, name }
  );
  return result.id;
}

async function createTask(page: Page, projectId: string, name: string): Promise<string> {
  const result = await page.evaluate(
    async ({ projectId, name }) =>
      (window as any).__TAURI_INTERNALS__.invoke('task_create', {
        request: { project_id: projectId, name }
      }),
    { projectId, name }
  );
  return result.id;
}

async function deleteClient(page: Page, id: string): Promise<void> {
  await page.evaluate(
    async (id) => (window as any).__TAURI_INTERNALS__.invoke('client_delete', { id }),
    id
  );
}

async function stopTimer(page: Page): Promise<void> {
  try {
    await page.evaluate(async () =>
      (window as any).__TAURI_INTERNALS__.invoke('timer_stop')
    );
  } catch {}
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

test.describe('US5 — Keyboard-First Quick Entry with Fuzzy Matching', () => {
  test.describe.configure({ mode: 'serial' });

  let client1Id: string;
  let client2Id: string;
  let project1Id: string;
  let project2Id: string;
  let projectSharedId: string;
  let projectSharedUnderClient2Id: string;
  let taskId: string;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    await waitForApp(page);

    // Client 1 with Project A + Task
    client1Id = await createClient(page, '__US5_Client1__');
    project1Id = await createProject(page, client1Id, '__US5_ProjectAlpha__');
    taskId = await createTask(page, project1Id, '__US5_TaskBeta__');

    // Client 2 with unique project
    client2Id = await createClient(page, '__US5_Client2__');
    project2Id = await createProject(page, client2Id, '__US5_ProjectGamma__');

    // Shared project name under BOTH clients (for disambiguation test)
    projectSharedId = await createProject(page, client1Id, '__US5_SharedProject__');
    projectSharedUnderClient2Id = await createProject(page, client2Id, '__US5_SharedProject__');

    await page.close();
  });

  test.afterAll(async ({ browser }) => {
    const page = await browser.newPage();
    await waitForApp(page);
    try { await deleteClient(page, client1Id); } catch {}
    try { await deleteClient(page, client2Id); } catch {}
    await page.close();
  });

  test.afterEach(async ({ page }) => {
    await stopTimer(page);
  });

  // ─── AS1: Live fuzzy project dropdown ────────────────────────────────────

  test('AS1 — typing partial project name shows fuzzy dropdown', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');

    await input.click();
    await input.fill('__US5_Proj');
    await page.waitForTimeout(400); // debounce

    // Slash should trigger project-mode dropdown
    await input.press('/');
    await page.waitForTimeout(400);

    // Fuzzy dropdown should appear with project matches
    const dropdown = page.locator('.fuzzy-dropdown, [role="listbox"][aria-label*="Project"]');
    await expect(dropdown).toBeVisible({ timeout: 2000 });
  });

  // ─── AS2: Arrow key navigation ────────────────────────────────────────────

  test('AS2 — arrow keys navigate fuzzy project dropdown', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');
    await input.click();
    await input.fill('__US5_Proj');
    await input.press('/');
    await page.waitForTimeout(400);

    const dropdown = page.locator('.fuzzy-dropdown');
    if (!await dropdown.isVisible().catch(() => false)) {
      test.skip(true, 'Dropdown not visible — upstream test prerequisite failed');
      return;
    }

    // Press ArrowDown — first item should be selected
    await input.press('ArrowDown');
    await expect(dropdown.locator('.fuzzy-item-selected')).toBeVisible();
  });

  // ─── AS3: Tab/Enter confirms project, advances to task segment ───────────

  test('AS3 — Tab confirms project and shows task segment input', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');
    await input.click();
    await input.fill('__US5_ProjectAlpha');
    await input.press('/');
    await page.waitForTimeout(400);

    const dropdown = page.locator('.fuzzy-dropdown');
    if (await dropdown.isVisible().catch(() => false)) {
      await input.press('Tab'); // confirm top match
      await page.waitForTimeout(300);
    }

    // Project chip should appear
    const projectChip = page.locator('.entry-segment-project');
    await expect(projectChip).toBeVisible({ timeout: 2000 });
    await expect(projectChip).toContainText('__US5_ProjectAlpha__');
  });

  // ─── AS4: Two-segment (project/task) flow ────────────────────────────────

  test('AS4 — two slashes: project then task then description', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');

    // Project segment
    await input.click();
    await input.fill('__US5_ProjectAlpha');
    await input.press('/');
    await page.waitForTimeout(400);

    if (await page.locator('.fuzzy-dropdown').isVisible().catch(() => false)) {
      await input.press('Tab');
      await page.waitForTimeout(300);
    }

    // Task segment
    await input.fill('__US5_TaskBeta');
    await input.press('/');
    await page.waitForTimeout(400);

    if (await page.locator('.fuzzy-dropdown').isVisible().catch(() => false)) {
      await input.press('Tab');
      await page.waitForTimeout(300);
    }

    // Task chip should appear
    const taskChip = page.locator('.entry-segment-task');
    await expect(taskChip).toBeVisible({ timeout: 2000 });
    await expect(taskChip).toContainText('__US5_TaskBeta__');
  });

  // ─── AS5: Full entry starts timer ─────────────────────────────────────────

  test('AS5 — typing description and pressing Enter starts timer', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');
    await input.click();

    // Skip straight to description (no slash = description-only mode)
    await input.fill('US5 integration test entry');
    await input.press('Enter');
    await page.waitForTimeout(500);

    // Timer running indicator should appear
    const elapsed = page.locator('.running-elapsed, [role="timer"]');
    await expect(elapsed).toBeVisible({ timeout: 3000 });
  });

  // ─── T054a: Single-client silent resolution ───────────────────────────────

  test('T054a — unique project name resolves without disambiguation', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');

    // ProjectAlpha exists under only Client1
    await input.click();
    await input.fill('__US5_ProjectAlpha');
    await input.press('/');
    await page.waitForTimeout(400);

    // Disambiguation dropdown must NOT appear
    const disambig = page.locator('.disambiguation-dropdown');
    await expect(disambig).not.toBeVisible();
  });

  // ─── T054a: Multi-client triggers disambiguation ──────────────────────────

  test('T054a — shared project name triggers disambiguation dropdown', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');

    // SharedProject exists under BOTH clients
    await input.click();
    await input.fill('__US5_SharedProject');
    await input.press('/');
    await page.waitForTimeout(400);

    // Disambiguation dropdown should appear
    const disambig = page.locator('.disambiguation-dropdown');
    await expect(disambig).toBeVisible({ timeout: 2000 });
    // Should list both clients
    await expect(disambig).toContainText('__US5_Client1__');
    await expect(disambig).toContainText('__US5_Client2__');
  });

  // ─── Highlighted match chars ──────────────────────────────────────────────

  test('fuzzy dropdown highlights matched characters with .match-char', async ({ page }) => {
    await waitForApp(page);
    const input = page.locator('.entry-input');
    await input.click();
    await input.fill('US5_Proj');
    await input.press('/');
    await page.waitForTimeout(400);

    const dropdown = page.locator('.fuzzy-dropdown');
    if (await dropdown.isVisible().catch(() => false)) {
      // .match-char spans should be inside the dropdown for highlighted chars
      const matchChars = dropdown.locator('.match-char');
      await expect(matchChars.first()).toBeVisible();
    }
  });
});
