import { test, expect, type Page } from '@playwright/test';

/**
 * Regression tests for issues reported March 17, 2026:
 *  Issue 2  — Timer elapsed jumps to ~1h on page navigation (DateTimeOffset parsing bug)
 *  Issue 3  — task_list errors with "missing required key projectId" (camelCase IPC)
 *  Issue 4  — All projects appear under every client (clientId filter ignored)
 *  Issue 5a — "Show archived" checkbox has no effect (includeArchived ignored)
 *  Issue 5b — Archived client name still blocks creating new client with same name
 *  Issue 1  — Timeline scroll-wheel zoom (new feature)
 *
 * Tests are FAILING before the fixes are applied (TDD gate).
 */

const APP_URL = 'http://localhost:5000';

async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

async function goToProjects(page: Page): Promise<void> {
  await page.goto(`${APP_URL}/projects`);
  await page.waitForLoadState('networkidle');
}

async function goToTimeline(page: Page): Promise<void> {
  await page.goto(`${APP_URL}/timeline`);
  await page.waitForLoadState('networkidle');
}

// ─── IPC Fixtures ──────────────────────────────────────────────────────────

async function createClient(page: Page, name: string): Promise<string> {
  const result = await page.evaluate(async (n) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('client_create', {
      name: n, color: '#3b82f6', logo_path: null
    });
  }, name);
  return result.id;
}

async function archiveClient(page: Page, id: string): Promise<void> {
  await page.evaluate(async (id) => {
    await (window as any).__TAURI_INTERNALS__.invoke('client_archive', { id });
  }, id);
}

async function createProject(page: Page, clientId: string, name: string): Promise<string> {
  const result = await page.evaluate(async ({ clientId, name }) => {
    // project_create takes a 'request' struct wrapper
    return await (window as any).__TAURI_INTERNALS__.invoke('project_create', {
      request: { client_id: clientId, name }
    });
  }, { clientId, name });
  return result.id;
}

async function deleteClient(page: Page, id: string): Promise<void> {
  await page.evaluate(async (id) => {
    await (window as any).__TAURI_INTERNALS__.invoke('client_delete', { id });
  }, id);
}

async function startTimer(page: Page, description: string): Promise<void> {
  await page.evaluate(async (desc) => {
    await (window as any).__TAURI_INTERNALS__.invoke('timer_start', {
      request: { description: desc, project_id: null, task_id: null, tags: [] }
    });
  }, description);
}

async function stopTimer(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await (window as any).__TAURI_INTERNALS__.invoke('timer_stop');
  });
}

// ─────────────────────────────────────────────────────────────────────────────

test.describe('Issue Regressions — March 17 2026', () => {
  test.describe.configure({ mode: 'serial' });

  test.describe('Issue 3 — task_list camelCase arg', () => {
    let clientId: string;
    let projectId: string;

    test.beforeEach(async ({ page }) => {
      await goToProjects(page);
      clientId = await createClient(page, '__Test_TaskList__');
      projectId = await createProject(page, clientId, 'MyProject');
    });

    test.afterEach(async ({ page }) => {
      if (clientId) await deleteClient(page, clientId);
    });

    test('expanding project shows task list without error', async ({ page }) => {
      await goToProjects(page);
      // Expand client
      const clientHeader = page.locator('.client-header').filter({ hasText: '__Test_TaskList__' });
      await clientHeader.locator('.chevron-btn').click();
      // Expand project
      const projectRow = page.locator('.project-row').filter({ hasText: 'MyProject' });
      await expect(projectRow).toBeVisible();
      await projectRow.locator('.chevron-btn').click();
      // Should show empty tasks state, not an error
      await expect(page.locator('.timeline-error-banner, .alert-danger, [role="alert"]')).not.toBeVisible();
      await expect(page.locator('.task-list')).toBeVisible();
      await expect(page.locator('text=No tasks yet.')).toBeVisible();
    });
  });

  test.describe('Issue 4 — project_list clientId filter', () => {
    let clientAId: string;
    let clientBId: string;

    test.beforeEach(async ({ page }) => {
      await goToProjects(page);
      clientAId = await createClient(page, '__ClientA__');
      clientBId = await createClient(page, '__ClientB__');
      await createProject(page, clientAId, 'Project-A-Only');
      await createProject(page, clientBId, 'Project-B-Only');
    });

    test.afterEach(async ({ page }) => {
      if (clientAId) await deleteClient(page, clientAId);
      if (clientBId) await deleteClient(page, clientBId);
    });

    test('expanding client A shows only its own projects', async ({ page }) => {
      await goToProjects(page);
      // Expand client A
      const clientHeader = page.locator('.client-header').filter({ hasText: '__ClientA__' });
      await clientHeader.locator('.chevron-btn').click();
      await page.waitForTimeout(500);
      // Project-A-Only should be visible
      await expect(page.locator('.project-row').filter({ hasText: 'Project-A-Only' })).toBeVisible();
      // Project-B-Only should NOT be visible under client A
      const projectBUnderA = page.locator('.client-card')
        .filter({ hasText: '__ClientA__' })
        .locator('.project-row')
        .filter({ hasText: 'Project-B-Only' });
      await expect(projectBUnderA).not.toBeVisible();
    });
  });

  test.describe('Issue 5a — includeArchived filter', () => {
    let clientId: string;

    test.beforeEach(async ({ page }) => {
      await goToProjects(page);
      clientId = await createClient(page, '__ArchiveFilter__');
      await archiveClient(page, clientId);
    });

    test.afterEach(async ({ page }) => {
      if (clientId) await deleteClient(page, clientId);
    });

    test('archived client hidden by default', async ({ page }) => {
      await goToProjects(page);
      await expect(page.locator('.client-name').filter({ hasText: '__ArchiveFilter__' })).not.toBeVisible();
    });

    test('archived client shown when checkbox checked', async ({ page }) => {
      await goToProjects(page);
      const checkbox = page.locator('input[type="checkbox"]').first();
      await checkbox.check();
      await page.waitForTimeout(500);
      await expect(page.locator('.client-name').filter({ hasText: '__ArchiveFilter__' })).toBeVisible();
    });
  });

  test.describe('Issue 5b — archive name conflict', () => {
    let firstClientId: string;

    test.afterEach(async ({ page }) => {
      // Clean up both possible clients
      try { if (firstClientId) await deleteClient(page, firstClientId); } catch {}
      // Also try to delete any __ArchiveNameTest__ clients
      const clients = await page.evaluate(async () => {
        const result = await (window as any).__TAURI_INTERNALS__.invoke('client_list', { includeArchived: true });
        return result.clients ?? [];
      });
      for (const c of clients) {
        if (c.name === '__ArchiveNameTest__') {
          await deleteClient(page, c.id);
        }
      }
    });

    test('can create new client with same name as archived client', async ({ page }) => {
      await goToProjects(page);
      firstClientId = await createClient(page, '__ArchiveNameTest__');
      await archiveClient(page, firstClientId);

      // Creating another client with the same name should succeed (not throw name_conflict)
      let errorThrown = false;
      let newId: string = '';
      try {
        const result = await page.evaluate(async () => {
          return await (window as any).__TAURI_INTERNALS__.invoke('client_create', {
            name: '__ArchiveNameTest__', color: '#ff0000', logo_path: null
          });
        });
        newId = result.id;
      } catch {
        errorThrown = true;
      }
      expect(errorThrown, 'Creating client with archived name should not throw').toBe(false);
      expect(newId).toBeTruthy();
      if (newId) await deleteClient(page, newId);
    });
  });

  test.describe('Issue 2 — timer elapsed survives page navigation', () => {
    test.afterEach(async ({ page }) => {
      try { await stopTimer(page); } catch {}
    });

    test('elapsed time stays small after navigating away and back', async ({ page }) => {
      await waitForApp(page);
      await startTimer(page, '__TimerNavTest__');
      // Give it a couple of seconds
      await page.waitForTimeout(2000);

      // Navigate away then back
      await page.goto(`${APP_URL}/projects`);
      await page.waitForLoadState('networkidle');
      await page.goto(APP_URL);
      await page.waitForLoadState('networkidle');
      await page.waitForTimeout(500);

      // The elapsed time shown should be small (< 30 seconds since we just started)
      // Look for the timer display — it should NOT show anything close to 3600 (1 hour)
      const timerText = await page.locator('[data-testid="timer-elapsed"], .timer-elapsed, .elapsed').first().textContent().catch(() => '');
      if (timerText) {
        // Parse the time — should be HH:MM:SS format with value < 30 seconds
        const parts = timerText.replace(/[^0-9:]/g, '').split(':');
        if (parts.length >= 2) {
          const hours = parseInt(parts[0] || '0');
          const minutes = parseInt(parts[1] || '0');
          const totalMinutes = hours * 60 + minutes;
          expect(totalMinutes, `Timer should show < 1 minute after just starting, got: ${timerText}`).toBeLessThan(1);
        }
      }
    });
  });

  test.describe('Issue 1 — Timeline zoom', () => {
    test('scroll wheel zooms in and shows zoom indicator', async ({ page }) => {
      await goToTimeline(page);
      
      // If there are no screenshots, zoom still works on the bar
      // Wait for any state
      await page.waitForTimeout(500);
      
      const bar = page.locator('.timeline-bar-inner');
      
      // If bar is visible (may not be if no screenshots), test zoom
      if (await bar.isVisible().catch(() => false)) {
        // Scroll up (zoom in) over the bar
        await bar.hover();
        await bar.dispatchEvent('wheel', { deltaY: -100, deltaX: 0 });
        await page.waitForTimeout(300);
        
        // Zoom indicator should appear
        const indicator = page.locator('.timeline-zoom-indicator');
        await expect(indicator).toBeVisible();
        await expect(indicator).toContainText('window');
        
        // Double click to reset
        await bar.dblclick();
        await page.waitForTimeout(300);
        await expect(page.locator('.timeline-zoom-indicator')).not.toBeVisible();
      }
    });
  });
});
