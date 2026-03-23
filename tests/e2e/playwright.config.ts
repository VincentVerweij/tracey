import { defineConfig, devices } from '@playwright/test';

/**
 * Tracey E2E Test Configuration
 *
 * Tests run against the full Tauri application built with --features test
 * (enables GDI screenshot test stub — no real screen capture in CI).
 *
 * The app binary is built by Fusco's CI pipeline before Playwright runs.
 * For local runs, build with: cargo tauri build --features test
 */
export default defineConfig({
  testDir: './specs',
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  fullyParallel: false, // Desktop app tests must run serially — single app instance
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1, // Single worker — one app instance at a time
  reporter: [
    ['html', { open: 'never' }],
    ['list'],
  ],
  use: {
    // Tauri app is launched as a subprocess by the test fixture
    // baseURL is not used — tests interact with the app window directly
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
