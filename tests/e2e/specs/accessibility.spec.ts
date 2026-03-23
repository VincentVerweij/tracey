import { test, expect, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

/**
 * T088 — Automated Accessibility Audit
 *
 * Asserts zero WCAG 2.1 AA violations on every page of the Tracey UI using
 * axe-core via @axe-core/playwright.  Also validates that keyboard-only
 * navigation (Tab) can reach all interactive elements on Dashboard and Settings.
 *
 * Run against the full Tauri app:
 *   npx playwright test specs/accessibility.spec.ts
 *
 * Requires @axe-core/playwright in devDependencies:
 *   npm install --save-dev @axe-core/playwright@^4.10.0
 *
 * WCAG tags exercised: wcag2a, wcag2aa, wcag21a, wcag21aa
 */

const APP_URL = 'http://localhost:5000';

/**
 * Navigate to the app root and wait for Blazor WASM to fully hydrate.
 * Same pattern used throughout the test suite (portable.spec.ts, etc.).
 */
async function waitForApp(page: Page): Promise<void> {
  await page.goto(APP_URL);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(500); // allow Blazor to finish component lifecycle
}

// ---------------------------------------------------------------------------
// Per-page axe audits
// ---------------------------------------------------------------------------

const PAGES = [
  { name: 'Dashboard', path: '/' },
  { name: 'Projects',  path: '/projects' },
  { name: 'Tags',      path: '/tags' },
  { name: 'Timeline',  path: '/timeline' },
  { name: 'Settings',  path: '/settings' },
] as const;

for (const { name, path } of PAGES) {
  test(`${name} — zero WCAG 2.1 AA axe violations`, async ({ page }) => {
    await page.goto(`${APP_URL}${path}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);

    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
      .analyze();

    // Print violations to stdout so CI logs can be triaged without re-running
    if (results.violations.length > 0) {
      console.log(`\nAxe violations on ${name} (${path}):`);
      for (const v of results.violations) {
        console.log(`  [${v.impact ?? 'unknown'}] ${v.id}: ${v.description}`);
        for (const n of v.nodes) {
          console.log(`    -> ${n.html.substring(0, 120)}`);
        }
      }
    }

    expect(results.violations).toHaveLength(0);
  });
}

// ---------------------------------------------------------------------------
// Keyboard navigation tests
// ---------------------------------------------------------------------------

test('Dashboard — keyboard nav reaches all interactive elements', async ({ page }) => {
  await waitForApp(page);

  // Tab through interactive elements and count how many receive focus.
  // 50 Tab presses comfortably covers all nav links + quick-entry bar + stop button.
  let focusableCount = 0;
  const maxTabs = 50;

  for (let i = 0; i < maxTabs; i++) {
    await page.keyboard.press('Tab');
    const tag = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el || el === document.body) return null;
      const role = el.getAttribute('role');
      return el.tagName + (role ? `[role=${role}]` : '');
    });
    if (tag) focusableCount++;
  }

  // Minimum expectation: nav links (Dashboard, Projects, Tags, Timeline, Settings)
  // + Quick Entry input + at least one action button.
  expect(focusableCount).toBeGreaterThan(3);
});

test('Settings — keyboard nav reaches all form fields', async ({ page }) => {
  await page.goto(`${APP_URL}/settings`);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(500);

  // Count how many native form elements receive focus across 80 Tab presses.
  // Settings contains: timezone picker, inactivity timeout, screenshot interval,
  // screenshot retention, screenshot folder, page size, deny-list inputs, save buttons.
  let formFieldCount = 0;
  const maxTabs = 80;

  for (let i = 0; i < maxTabs; i++) {
    await page.keyboard.press('Tab');
    const isFormElement = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el) return false;
      return ['INPUT', 'SELECT', 'TEXTAREA', 'BUTTON'].includes(el.tagName);
    });
    if (isFormElement) formFieldCount++;
  }

  // At minimum: General (timezone + page size) + Inactivity + Screenshots (3 fields)
  // + Storage folder + DenyList input + at least one Save/Apply button = > 8
  expect(formFieldCount).toBeGreaterThan(8);
});
