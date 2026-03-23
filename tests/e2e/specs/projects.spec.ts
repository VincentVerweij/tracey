import { test, expect, type Page } from '@playwright/test';
import { hasTauriAvailable } from './tauri-helpers';

/**
 * US3 — Manage Clients, Projects, and Tasks
 * T037: All acceptance scenarios from spec.md US3
 *
 * Tests written BEFORE implementation (TDD gate for Phase 5).
 * Must be confirmed FAILING before Root/Reese touch the Projects page + IPC.
 *
 * Tests FAIL with:
 *   net::ERR_CONNECTION_REFUSED  — when no dev server is running (current state)
 *   TimeoutError                 — when dev server runs but UI not yet implemented
 *
 * IPC commands exercised (from contracts/ipc-commands.md US3):
 *   client_create, client_list, client_archive, client_unarchive, client_delete
 *   project_create, project_list, project_archive, project_unarchive, project_delete
 *   task_create, task_list, task_delete
 */

const APP_URL = 'http://localhost:5000';

/**
 * Navigate to the app and wait for Blazor WASM to hydrate.
 */
async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
}

/**
 * Navigate directly to the Projects page.
 */
async function goToProjects(page: Page): Promise<void> {
  await page.goto(`${APP_URL}/projects`);
  await page.waitForLoadState('networkidle');
}

// ─────────────────────────────────────────────────────────────────────────────
// IPC Fixture Helpers — call Tauri IPC directly for fast test setup
// (avoids slow UI interaction for prerequisite state)
// ─────────────────────────────────────────────────────────────────────────────

async function createClient(page: Page, name: string, color = '#3B82F6'): Promise<string> {
  const result = await page.evaluate(async ({ name, color }) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('client_create', { name, color, logo_path: null });
  }, { name, color });
  return result.id;
}

async function createProject(page: Page, clientId: string, name: string): Promise<string> {
  const result = await page.evaluate(async ({ clientId, name }) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('project_create', { client_id: clientId, name });
  }, { clientId, name });
  return result.id;
}

async function createTask(page: Page, projectId: string, name: string): Promise<string> {
  const result = await page.evaluate(async ({ projectId, name }) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('task_create', { project_id: projectId, name });
  }, { projectId, name });
  return result.id;
}

async function deleteClient(page: Page, clientId: string): Promise<void> {
  await page.evaluate(async (id) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('client_delete', { id });
  }, clientId);
}

async function deleteProject(page: Page, projectId: string): Promise<void> {
  await page.evaluate(async (id) => {
    return await (window as any).__TAURI_INTERNALS__.invoke('project_delete', { id });
  }, projectId);
}

// ─────────────────────────────────────────────────────────────────────────────

test.describe('US3 — Manage Clients, Projects, and Tasks', () => {

  test.describe.configure({ mode: 'serial' });

  test.beforeEach(async ({ page }) => {
    if (!(await hasTauriAvailable(page))) {
      test.skip(true, 'Requires Tauri bridge — run with tauri-driver for IPC tests');
    }
  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC1 — Navigate to Projects page
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC1 — Projects Page Navigation', () => {

    test('navigate to projects page shows page heading', async ({ page }) => {
      // Spec: /projects route renders a page with a visible heading
      await goToProjects(page);

      const heading = page.getByRole('heading', { name: /projects/i });
      await expect(heading).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC2 — Create a Client
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC2 — Create Client', () => {

    let clientId: string | null = null;

    test.afterEach(async ({ page }) => {
      // Clean up IPC-created client after each test in this group
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
      }
    });

    test('create a client via UI form — appears in client list with color swatch', async ({ page }) => {
      // Spec US3: "Add Client" button opens form; name + color → client saved and shown
      await goToProjects(page);

      // Open Add Client form
      const addClientBtn = page.getByRole('button', { name: /add client/i });
      await expect(addClientBtn).toBeVisible();
      await addClientBtn.click();

      // Fill in name
      const nameInput = page.getByRole('textbox', { name: /client name/i });
      await expect(nameInput).toBeVisible();
      await nameInput.fill('Acme Corp');

      // Set color — color input or color picker
      const colorInput = page.locator('input[type="color"], input[aria-label*="color" i]').first();
      await colorInput.fill('#FF5733');

      // Save
      const saveBtn = page.getByRole('button', { name: /save|create|confirm/i });
      await saveBtn.click();

      // Verify client appears in list
      await expect(page.getByText('Acme Corp')).toBeVisible();

      // Verify color swatch element is present in the client row
      const clientRow = page.locator('[aria-label*="Acme Corp" i], [data-testid*="client" i]').filter({ hasText: 'Acme Corp' });
      const colorSwatch = clientRow.locator('[class*="swatch" i], [class*="color" i], [style*="background"]').first()
        .or(page.locator('[aria-label="Acme Corp color swatch"]'));
      await expect(colorSwatch).toBeVisible();
    });

    test('client name conflict shows error message', async ({ page }) => {
      // Spec US3: duplicate name → error displayed, no duplicate created
      await waitForApp(page);

      // Create first client via IPC for speed
      clientId = await createClient(page, 'Acme Corp');

      await goToProjects(page);

      // Attempt to create duplicate via UI
      await page.getByRole('button', { name: /add client/i }).click();

      const nameInput = page.getByRole('textbox', { name: /client name/i });
      await nameInput.fill('Acme Corp');

      await page.getByRole('button', { name: /save|create|confirm/i }).click();

      // Expect an error/validation message
      const errorMsg = page.getByRole('alert')
        .or(page.locator('[class*="error" i], [class*="invalid" i], [aria-live]'))
        .filter({ hasText: /already exists|conflict|duplicate|in use/i });
      await expect(errorMsg).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC3 — Create a Project under a Client
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC3 — Create Project', () => {

    let clientId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
      }
    });

    test('create a project under a client — appears in collapsible project list', async ({ page }) => {
      // Spec US3: project created under client; appears in collapsible section
      await goToProjects(page);

      // Expand Acme Corp section if collapsed
      const clientSection = page.locator('[aria-label*="Acme Corp" i]').first()
        .or(page.getByText('Acme Corp').locator('..'));

      // Find and click Add Project button within the client section
      const addProjectBtn = page.getByRole('button', { name: /add project/i })
        .or(page.locator('[aria-label*="add project" i]'));
      await addProjectBtn.first().click();

      // Fill in project name
      const projectNameInput = page.getByRole('textbox', { name: /project name/i });
      await expect(projectNameInput).toBeVisible();
      await projectNameInput.fill('Website Redesign');

      // Save
      await page.getByRole('button', { name: /save|create|confirm/i }).click();

      // Verify project appears
      await expect(page.getByText('Website Redesign')).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC4 — Create a Task under a Project
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC4 — Create Task', () => {

    let clientId: string | null = null;
    let projectId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
      projectId = await createProject(page, clientId!, 'Website Redesign');
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
        projectId = null;
      }
    });

    test('create a task under a project — appears under the project', async ({ page }) => {
      // Spec US3: task created under project; shown in task list under that project
      await goToProjects(page);

      // Locate the project section and its Add Task form/button
      const addTaskBtn = page.getByRole('button', { name: /add task/i }).first();
      await expect(addTaskBtn).toBeVisible();
      await addTaskBtn.click();

      // Fill task name
      const taskNameInput = page.getByRole('textbox', { name: /task name/i })
        .or(page.locator('input[placeholder*="task" i]')).first();
      await expect(taskNameInput).toBeVisible();
      await taskNameInput.fill('Design Mockups');

      // Save / confirm
      await page.keyboard.press('Enter');

      // Verify task appears
      await expect(page.getByText('Design Mockups')).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC5 — Archive a Project
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC5 — Archive Project', () => {

    let clientId: string | null = null;
    let projectId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
      projectId = await createProject(page, clientId!, 'Website Redesign');
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
        projectId = null;
      }
    });

    test('archive a project — disappears from active list', async ({ page }) => {
      // Spec US3: archive project → removed from default (non-archived) view
      await goToProjects(page);

      await expect(page.getByText('Website Redesign')).toBeVisible();

      // Click Archive on the project row
      const archiveBtn = page.getByRole('button', { name: /^archive$/i })
        .or(page.locator('[aria-label="Archive Website Redesign"]'));
      await archiveBtn.first().click();

      // Project should no longer appear in the active list
      await expect(page.getByText('Website Redesign')).not.toBeVisible();
    });

    test('archived project appears when show-archived view is toggled', async ({ page }) => {
      // Spec US3: toggle archived view → archived project becomes visible
      await waitForApp(page);

      // Archive via IPC for speed
      await page.evaluate(async (id) => {
        await (window as any).__TAURI_INTERNALS__.invoke('project_archive', { id });
      }, projectId);

      await goToProjects(page);

      // Should NOT appear by default
      await expect(page.getByText('Website Redesign')).not.toBeVisible();

      // Toggle "Show archived" / "Include archived"
      const showArchivedToggle = page.getByRole('checkbox', { name: /show archived|include archived/i })
        .or(page.getByRole('button', { name: /show archived|include archived/i }));
      await showArchivedToggle.click();

      // Now should appear
      await expect(page.getByText('Website Redesign')).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC6 — Unarchive a Project
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC6 — Unarchive Project', () => {

    let clientId: string | null = null;
    let projectId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
      projectId = await createProject(page, clientId!, 'Website Redesign');
      // Pre-archive the project via IPC
      await page.evaluate(async (id) => {
        await (window as any).__TAURI_INTERNALS__.invoke('project_archive', { id });
      }, projectId);
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
        projectId = null;
      }
    });

    test('unarchive a project — returns to active list', async ({ page }) => {
      // Spec US3: unarchive → project visible in default (active) list again
      await goToProjects(page);

      // Show archived to reveal the project
      const showArchivedToggle = page.getByRole('checkbox', { name: /show archived|include archived/i })
        .or(page.getByRole('button', { name: /show archived|include archived/i }));
      await showArchivedToggle.click();

      await expect(page.getByText('Website Redesign')).toBeVisible();

      // Click Unarchive
      const unarchiveBtn = page.getByRole('button', { name: /^unarchive$/i })
        .or(page.locator('[aria-label="Unarchive Website Redesign"]'));
      await unarchiveBtn.first().click();

      // Hide archived view again
      await showArchivedToggle.click();

      // Project should now be visible in the active list
      await expect(page.getByText('Website Redesign')).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC7 — Archived project absent from QuickEntryBar autocomplete
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC7 — Picker Integration (archived absent from autocomplete)', () => {

    let clientId: string | null = null;
    let projectId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
      projectId = await createProject(page, clientId!, 'Website Redesign');
      // Archive the project via IPC
      await page.evaluate(async (id) => {
        await (window as any).__TAURI_INTERNALS__.invoke('project_archive', { id });
      }, projectId);
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
        projectId = null;
      }
    });

    test('archived project does not appear in QuickEntryBar autocomplete suggestions', async ({ page }) => {
      // Spec US3 picker integration: archived projects NOT in autocomplete
      await waitForApp(page);

      // Navigate to Dashboard/Home where the QuickEntryBar lives
      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await expect(quickEntry).toBeVisible();

      await quickEntry.fill('Website');

      const dropdown = page.getByRole('listbox');
      // Either no dropdown at all, or no suggestion matches the archived project
      const hasDropdown = await dropdown.isVisible().catch(() => false);
      if (hasDropdown) {
        const options = dropdown.getByRole('option');
        const matchingOption = options.filter({ hasText: /website redesign/i });
        await expect(matchingOption).toHaveCount(0);
      }
      // If no dropdown appears at all — that also satisfies the requirement (no suggestions shown)
    });

    test('archived client does not appear in QuickEntryBar autocomplete suggestions', async ({ page }) => {
      // Spec US3 picker integration: archived clients NOT in autocomplete
      // First archive the client as well
      await page.evaluate(async (id) => {
        await (window as any).__TAURI_INTERNALS__.invoke('client_archive', { id });
      }, clientId);

      await waitForApp(page);

      const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
      await expect(quickEntry).toBeVisible();

      await quickEntry.fill('Acme');

      const dropdown = page.getByRole('listbox');
      const hasDropdown = await dropdown.isVisible().catch(() => false);
      if (hasDropdown) {
        const matchingOption = dropdown.getByRole('option').filter({ hasText: /acme corp/i });
        await expect(matchingOption).toHaveCount(0);
      }
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // AC8 — Delete Client with cascade confirmation
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('AC8 — Delete Client (cascade confirmation)', () => {

    let clientId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
      // Create a project and task so cascade counts are non-zero
      const projectId = await createProject(page, clientId!, 'Website Redesign');
      await createTask(page, projectId, 'Design Mockups');
    });

    test.afterEach(async ({ page }) => {
      // Attempt cleanup in case the test did not delete
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone — expected on delete test */ }
        clientId = null;
      }
    });

    test('delete client shows confirmation modal with cascade count, then removes client', async ({ page }) => {
      // Spec US3: delete client → confirmation modal shows deleted_projects, deleted_tasks, orphaned_entries
      await goToProjects(page);

      await expect(page.getByText('Acme Corp')).toBeVisible();

      // Click Delete on the client row
      const deleteBtn = page.getByRole('button', { name: /delete/i })
        .or(page.locator('[aria-label="Delete Acme Corp"]'));
      await deleteBtn.first().click();

      // Confirmation modal/dialog must appear
      const confirmModal = page.getByRole('dialog');
      await expect(confirmModal).toBeVisible();

      // Modal should show cascade information (projects, tasks, orphaned entries)
      await expect(confirmModal).toContainText(/project|task|entr/i);

      // Confirm deletion
      const confirmBtn = confirmModal.getByRole('button', { name: /delete|confirm|yes/i });
      await confirmBtn.click();

      // Client should be gone
      await expect(page.getByText('Acme Corp')).not.toBeVisible();
      clientId = null; // already deleted
    });

    test('cancelling delete confirmation leaves client in place', async ({ page }) => {
      // Spec US3: cancel in confirmation modal → client not deleted
      await goToProjects(page);

      await expect(page.getByText('Acme Corp')).toBeVisible();

      const deleteBtn = page.getByRole('button', { name: /delete/i })
        .or(page.locator('[aria-label="Delete Acme Corp"]'));
      await deleteBtn.first().click();

      // Confirmation modal appears
      const confirmModal = page.getByRole('dialog');
      await expect(confirmModal).toBeVisible();

      // Click Cancel
      const cancelBtn = confirmModal.getByRole('button', { name: /cancel|no|keep/i });
      await cancelBtn.click();

      // Modal dismissed
      await expect(confirmModal).not.toBeVisible();

      // Client still present
      await expect(page.getByText('Acme Corp')).toBeVisible();
    });

  });

  // ───────────────────────────────────────────────────────────────────────────
  // Additional: Archive / Unarchive client
  // ───────────────────────────────────────────────────────────────────────────

  test.describe('Archive / Unarchive Client', () => {

    let clientId: string | null = null;

    test.beforeEach(async ({ page }) => {
      await waitForApp(page);
      clientId = await createClient(page, 'Acme Corp');
    });

    test.afterEach(async ({ page }) => {
      if (clientId) {
        try { await deleteClient(page, clientId); } catch { /* already gone */ }
        clientId = null;
      }
    });

    test('archive a client — disappears from active client list', async ({ page }) => {
      await goToProjects(page);

      await expect(page.getByText('Acme Corp')).toBeVisible();

      const archiveBtn = page.getByRole('button', { name: /^archive$/i })
        .or(page.locator('[aria-label="Archive Acme Corp"]'));
      await archiveBtn.first().click();

      await expect(page.getByText('Acme Corp')).not.toBeVisible();
    });

    test('unarchive a client — returns to active client list', async ({ page }) => {
      // Archive first via IPC
      await page.evaluate(async (id) => {
        await (window as any).__TAURI_INTERNALS__.invoke('client_archive', { id });
      }, clientId);

      await goToProjects(page);
      await expect(page.getByText('Acme Corp')).not.toBeVisible();

      // Show archived
      const showArchivedToggle = page.getByRole('checkbox', { name: /show archived|include archived/i })
        .or(page.getByRole('button', { name: /show archived|include archived/i }));
      await showArchivedToggle.click();

      await expect(page.getByText('Acme Corp')).toBeVisible();

      // Unarchive
      const unarchiveBtn = page.getByRole('button', { name: /^unarchive$/i })
        .or(page.locator('[aria-label="Unarchive Acme Corp"]'));
      await unarchiveBtn.first().click();

      // Dismiss archived view, client should now appear in active list
      await showArchivedToggle.click();
      await expect(page.getByText('Acme Corp')).toBeVisible();
    });

  });

});

