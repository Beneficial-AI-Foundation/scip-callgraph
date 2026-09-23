/**
 * Query label bar scenarios - verifies the query label above the graph
 * Run: npx playwright test e2e/query-label-scenarios.spec.ts
 */
import { test, expect } from '@playwright/test';

const BASE_URL = process.env.BASE_URL || 'http://localhost:3001';
// Pinned to a tracked fixture (1,333 nodes, contains "decompress" and "mul")
// so the assertions don't depend on whatever public/graph.json currently is.
const APP_URL = `${BASE_URL}/probegraph/?json=./curve_from_rust_atomizer.json`;

async function getQueryLabelText(page: any): Promise<string> {
  const el = page.locator('#query-label');
  const isVisible = await el.isVisible();
  if (!isVisible) return '(query label not visible)';
  const html = await el.innerHTML();
  // Strip HTML tags for readable text
  return html.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
}

test.describe('Query Label Bar Scenarios', () => {
  test('Step 1: Page load - query label after auto-load', async ({ page }) => {
    await page.goto(APP_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2500); // Allow graph to load and render

    const queryLabel = await getQueryLabelText(page);
    console.log('\n[Step 1] Query label:', queryLabel);

    await page.screenshot({
      path: 'test-screenshots/query-label-01-initial.png',
      fullPage: true,
    });

    // Query label should be visible (graph loaded)
    const labelEl = page.locator('#query-label');
    await expect(labelEl).toBeVisible();
  });

  test('Step 2: Source query "decompress" - callees from decompress', async ({
    page,
  }) => {
    await page.goto(APP_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2500);

    await page.fill('#source-input', 'decompress');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1200);

    const queryLabel = await getQueryLabelText(page);
    console.log('\n[Step 2] Query label:', queryLabel);

    await page.screenshot({
      path: 'test-screenshots/query-label-02-source-decompress.png',
      fullPage: true,
    });

    expect(queryLabel.toLowerCase()).toContain('callees');
    expect(queryLabel.toLowerCase()).toContain('decompress');
  });

  test('Step 3: Add sink "mul" - paths decompress → mul', async ({ page }) => {
    await page.goto(APP_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2500);

    await page.fill('#source-input', 'decompress');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(800);

    await page.fill('#sink-input', 'mul');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1200);

    const queryLabel = await getQueryLabelText(page);
    console.log('\n[Step 3] Query label:', queryLabel);

    await page.screenshot({
      path: 'test-screenshots/query-label-03-source-sink.png',
      fullPage: true,
    });

    expect(queryLabel.toLowerCase()).toContain('paths');
    expect(queryLabel.toLowerCase()).toContain('decompress');
    expect(queryLabel.toLowerCase()).toContain('mul');
  });

  test('Step 4: Clear both - query: all (no traversal)', async ({ page }) => {
    await page.goto(APP_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2500);

    // Set source and sink first
    await page.fill('#source-input', 'decompress');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);
    await page.fill('#sink-input', 'mul');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    // Clear source
    await page.fill('#source-input', '');
    await page.waitForTimeout(300);

    // Clear sink and press Enter
    await page.fill('#sink-input', '');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1200);

    const queryLabel = await getQueryLabelText(page);
    console.log('\n[Step 4] Query label:', queryLabel);

    await page.screenshot({
      path: 'test-screenshots/query-label-04-cleared.png',
      fullPage: true,
    });

    expect(queryLabel.toLowerCase()).toContain('all');
    expect(queryLabel.toLowerCase()).toContain('no traversal');
  });
});
