import { test, expect, Page } from '@playwright/test';

// Utility: set inactivity timeout (requires Tauri IPC, only works in live app)
async function setInactivityTimeout(page: Page, seconds: number): Promise<void> {
  await page.evaluate(async (s) => {
    // @ts-ignore
    await window.__TAURI_INTERNALS__.invoke('preferences_update', { 
      update: { inactivity_timeout_seconds: s } 
    });
  }, seconds);
}

test.describe('US2 — Idle Detection and On-Return Prompt', () => {

  test.describe.configure({ mode: 'serial' });

  test('idle modal does NOT appear when no timer is running', async ({ page }) => {
    await page.goto('/');
    
    // Ensure no timer running — stop any active
    try {
      await page.evaluate(async () => {
        // @ts-ignore
        await window.__TAURI_INTERNALS__.invoke('timer_stop');
      });
    } catch { /* no active timer — fine */ }
    
    // Set very short timeout
    await setInactivityTimeout(page, 5);
    
    // Wait longer than the timeout
    await page.waitForTimeout(7000);
    
    // Modal should NOT be visible when no timer was running
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).not.toBeVisible();
  });

  test('idle modal appears after inactivity threshold when timer is running', async ({ page }) => {
    await page.goto('/');
    
    await setInactivityTimeout(page, 5);
    
    // Start a timer
    const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
    await quickEntry.fill('Coding session');
    await quickEntry.press('Enter');
    
    // Confirm timer started
    await expect(page.getByRole('timer')).toBeVisible({ timeout: 2000 });
    
    // Wait for idle threshold + poll interval (idle detected within ~3s after threshold)
    await page.waitForTimeout(10000);
    
    // Modal should appear
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 5000 });
    
    // All 4 options must be present
    await expect(page.getByRole('button', { name: /break/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /meeting/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /specify/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /keep/i })).toBeVisible();
  });

  test('"Keep" option closes modal without creating new entry', async ({ page }) => {
    await page.goto('/');
    await setInactivityTimeout(page, 5);
    
    // Start timer
    const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
    await quickEntry.fill('Still working');
    await quickEntry.press('Enter');
    
    // Wait for idle modal
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 15000 });
    
    // Click Keep
    await page.getByRole('button', { name: /keep/i }).click();
    
    // Modal dismisses
    await expect(modal).not.toBeVisible({ timeout: 2000 });
    
    // Timer is still running (no new entry created for idle period)
    await expect(page.getByRole('timer')).toBeVisible();
  });

  test('"Break" option stops timer at idle start and creates break entry', async ({ page }) => {
    await page.goto('/');
    await setInactivityTimeout(page, 5);
    
    const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
    await quickEntry.fill('Deep work session');
    await quickEntry.press('Enter');
    
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 15000 });
    
    await page.getByRole('button', { name: /break/i }).click();
    
    // Modal closes
    await expect(modal).not.toBeVisible({ timeout: 2000 });
    
    // Navigate to Timeline to see the entries
    await page.getByRole('link', { name: /timeline/i }).click();
    await expect(page.getByText('Deep work session')).toBeVisible({ timeout: 3000 });
    await expect(page.getByText(/break/i)).toBeVisible({ timeout: 3000 });
  });

  test('"Meeting" option creates a meeting entry for the idle period', async ({ page }) => {
    await page.goto('/');
    await setInactivityTimeout(page, 5);
    
    const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
    await quickEntry.fill('Pre-meeting work');
    await quickEntry.press('Enter');
    
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 15000 });
    
    await page.getByRole('button', { name: /meeting/i }).click();
    
    await expect(modal).not.toBeVisible({ timeout: 2000 });
    
    // A meeting entry should appear in the list
    await page.getByRole('link', { name: /timeline/i }).click();
    await expect(page.getByText(/meeting/i)).toBeVisible({ timeout: 3000 });
  });

  test('"Specify" option shows description input and creates entry', async ({ page }) => {
    await page.goto('/');
    await setInactivityTimeout(page, 5);
    
    const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
    await quickEntry.fill('Before specify test');
    await quickEntry.press('Enter');
    
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).toBeVisible({ timeout: 15000 });
    
    await page.getByRole('button', { name: /specify/i }).click();
    
    // A description input should appear within the modal
    const specifyInput = modal.getByRole('textbox', { name: /description|what were you doing/i });
    await expect(specifyInput).toBeVisible({ timeout: 2000 });
    
    await specifyInput.fill('Reviewing architecture docs');
    await page.getByRole('button', { name: /save|confirm/i }).click();
    
    await expect(modal).not.toBeVisible({ timeout: 2000 });
    
    // Entry appears in list
    await page.getByRole('link', { name: /timeline/i }).click();
    await expect(page.getByText('Reviewing architecture docs')).toBeVisible({ timeout: 3000 });
  });

  test('idle threshold uses value from preferences (set via IPC)', async ({ page }) => {
    await page.goto('/');
    
    // Set long timeout — modal should NOT appear quickly
    await setInactivityTimeout(page, 300); // 5 minutes
    
    const quickEntry = page.getByRole('textbox', { name: /what are you working on/i });
    await quickEntry.fill('Short task');
    await quickEntry.press('Enter');
    
    await page.waitForTimeout(3000);
    
    // With 300s threshold, modal must not have appeared in 3 seconds
    const modal = page.getByRole('dialog', { name: /idle|away|back/i });
    await expect(modal).not.toBeVisible();
    
    // Restore reasonable threshold
    await setInactivityTimeout(page, 300);
    
    // Stop the timer
    await page.keyboard.press('Control+Space');
  });

});
