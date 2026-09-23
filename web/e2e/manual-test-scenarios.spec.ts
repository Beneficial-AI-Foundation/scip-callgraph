/**
 * Manual test scenarios for the probegraph viewer
 * Run with: npx playwright test e2e/manual-test-scenarios.spec.ts
 * Against user's dev server (port 3000): BASE_URL=http://localhost:3000 npx playwright test e2e/manual-test-scenarios.spec.ts
 */
import { test, expect } from '@playwright/test';

const BASE_URL = process.env.BASE_URL || 'http://localhost:3001';
const GRAPH_URL = `${BASE_URL}/probegraph/?json=graph_tmp.json`;

test.describe('Manual Test Scenarios', () => {
  test('Test A: Load graph_tmp.json and verify basic functionality', async ({
    page,
  }) => {
    // Navigate with graph_tmp.json auto-loaded via URL param
    await page.goto(GRAPH_URL);
    await page.waitForLoadState('networkidle');

    // Wait for graph to load - stats should show node/edge counts
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2000); // Allow graph to render

    const statsText = await page.locator('#stats').textContent();
    expect(statsText).toBeTruthy();

    // Extract node/link counts from stats (format: "Total Nodes: X (of Y)" etc.)
    const nodeMatch = statsText?.match(/Total Nodes:\s*(\d+)/);
    const edgeMatch = statsText?.match(/Total Edges:\s*(\d+)/);
    const nodeCount = nodeMatch ? parseInt(nodeMatch[1], 10) : 0;
    const edgeCount = edgeMatch ? parseInt(edgeMatch[1], 10) : 0;

    console.log(`\n[Test A] Node count: ${nodeCount}, Edge count: ${edgeCount}`);
    expect(nodeCount).toBeGreaterThan(0);
    expect(edgeCount).toBeGreaterThan(0);

    await page.screenshot({
      path: 'test-screenshots/A-loaded-graph.png',
      fullPage: true,
    });
  });

  test('Test B: Forward traversal with source query', async ({ page }) => {
    await page.goto(GRAPH_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2000);

    // Type "decompress" in Source field
    await page.fill('#source-input', 'decompress');

    // Set Max depth to 2 (range slider - set value and trigger input event)
    await page.locator('#depth-limit').evaluate((el) => {
      (el as HTMLInputElement).value = '2';
      el.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await page.waitForTimeout(500);

    // Wait for graph to update (debounce is 300ms)
    await page.waitForTimeout(1500);

    const statsText = await page.locator('#stats').textContent();
    const nodeMatch = statsText?.match(/Total Nodes:\s*(\d+)/);
    const nodeCount = nodeMatch ? parseInt(nodeMatch[1], 10) : 0;

    console.log(`\n[Test B] Filtered node count: ${nodeCount}`);
    expect(nodeCount).toBeLessThan(1130); // Should be reduced from full graph

    await page.screenshot({
      path: 'test-screenshots/B-source-decompress-depth2.png',
      fullPage: true,
    });
  });

  test('Test C: Backward traversal with sink query', async ({ page }) => {
    await page.goto(GRAPH_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2000);

    // Clear source
    await page.fill('#source-input', '');

    // Type "decompress" in Sink field
    await page.fill('#sink-input', 'decompress');

    await page.waitForTimeout(1500);

    const statsText = await page.locator('#stats').textContent();
    const nodeMatch = statsText?.match(/Total Nodes:\s*(\d+)/);
    const nodeCount = nodeMatch ? parseInt(nodeMatch[1], 10) : 0;

    console.log(`\n[Test C] Sink filter node count: ${nodeCount}`);

    await page.screenshot({
      path: 'test-screenshots/C-sink-decompress.png',
      fullPage: true,
    });
  });

  test('Test D: Link type filtering - uncheck Show postcondition', async ({
    page,
  }) => {
    await page.goto(GRAPH_URL);
    await page.waitForLoadState('networkidle');
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(2000);

    // Clear source and sink
    await page.fill('#source-input', '');
    await page.fill('#sink-input', '');

    // Uncheck "Show postcondition" checkbox (Ensures edges)
    // It's unchecked by default - check first to add edges, then uncheck to show filter effect
    const postCheckbox = page.locator('#show-postcondition-calls');
    if (!(await postCheckbox.isChecked())) {
      await postCheckbox.click();
      await page.waitForTimeout(800);
    }
    await postCheckbox.click(); // Uncheck
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'test-screenshots/D-postcondition-unchecked.png',
      fullPage: true,
    });
  });
});
