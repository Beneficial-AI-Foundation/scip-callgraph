/**
 * Config for manual test scenarios - uses existing dev server (no webServer).
 * Run: BASE_URL=http://localhost:3000 npx playwright test --config=playwright.manual.config.ts e2e/manual-test-scenarios.spec.ts
 */
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  testMatch: [
    '**/manual-test-scenarios.spec.ts',
    '**/query-label-scenarios.spec.ts',
    '**/ai-guide.spec.ts',
  ],
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:3000',
    trace: 'on-first-retry',
    screenshot: 'off', // We capture via page.screenshot() in tests
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  // No webServer - assume dev server is already running
});
