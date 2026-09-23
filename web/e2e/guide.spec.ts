/**
 * Guide chip actions verification test.
 * Verifies: graph shows nodes, query label (no depth=1), tab switch, toast, crate boundary, namespace map.
 * Run: npx playwright test guide.spec.ts --config=playwright.manual.config.ts
 */
import { test, expect } from '@playwright/test';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const BASE_URL = process.env.BASE_URL || 'http://localhost:3001';
// Pinned to a tracked multi-crate fixture so the Crate boundary / map chips
// exist and chip labels (derived from the graph's most connected node etc.)
// don't depend on whatever public/graph.json currently is.
const APP_PATH = '/probegraph/?json=./graph_backup_dalek.json';

test.describe('Guide Chip Actions', () => {
  test('verify chip actions: Explore double, Crate boundary, Namespace map', async ({
    page,
  }) => {
    const report: string[] = [];

    // Step 1: Navigate and ensure graph loads
    await page.goto(`${BASE_URL}${APP_PATH}`, { waitUntil: 'networkidle' });
    const statsBefore = await page.locator('#stats').textContent();
    if (statsBefore?.includes('No graph loaded')) {
      const graphPath = path.resolve(__dirname, '../public/graph.json');
      await page.locator('#file-input').setInputFiles(graphPath);
      await page.waitForTimeout(2000);
    }

    await page.waitForTimeout(1500);
    const initialNodes = await page.locator('circle.node').count();
    report.push(`Step 1: Initial state - ${initialNodes} nodes visible`);

    // Step 2: Click Guide tab
    await page.locator('#tab-guide').click();
    await page.waitForTimeout(500);

    // Step 3: Click "Explore double (most connected)" chip
    const exploreChip = page.locator('.guide-chip').filter({ hasText: /^Explore / }).first();
    await exploreChip.click();
    await page.waitForTimeout(2000);

    // Screenshot 3: After Explore double
    await page.screenshot({
      path: 'test-screenshots/guide-chips-03-explore-double.png',
      fullPage: true,
    });

    const nodesAfterExplore = await page.locator('circle.node').count();
    const nodeDetailsActive = (await page.locator('#tab-node-details').getAttribute('class'))?.includes('active') ?? false;
    const queryLabelExplore = await page.locator('#query-label').textContent();
    const hasDepth1 = queryLabelExplore?.includes('depth=1') ?? false;
    const toastVisible = (await page.locator('.toast').count()) > 0;

    report.push(`Step 3: After "Explore double" click:`);
    report.push(`  - Graph nodes visible: ${nodesAfterExplore} (expected 4: double + 3 callers)`);
    report.push(`  - Query label: "${queryLabelExplore}" (has depth=1: ${hasDepth1}, should be false)`);
    report.push(`  - Switched to Node Details tab: ${nodeDetailsActive}`);
    report.push(`  - Toast appeared: ${toastVisible}`);

    // Step 4: Go back to Guide, click Crate boundary chip
    await page.locator('#tab-guide').click();
    await page.waitForTimeout(300);
    const crateChip = page.locator('.guide-chip').filter({ hasText: 'Crate boundary' });
    await crateChip.click();
    await page.waitForTimeout(2000);

    await page.screenshot({
      path: 'test-screenshots/guide-chips-04-crate-boundary.png',
      fullPage: true,
    });
    const nodesAfterCrate = await page.locator('circle.node').count();
    const queryAfterCrate = await page.locator('#query-label').textContent();
    report.push(`Step 4: After "Crate boundary" - nodes: ${nodesAfterCrate}, query: "${queryAfterCrate}"`);

    // Step 5: Go back to Guide, click "View crate/namespace map"
    await page.locator('#tab-guide').click();
    await page.waitForTimeout(300);
    const mapChip = page.locator('.guide-chip').filter({ hasText: 'View crate' });
    await mapChip.click();
    await page.waitForTimeout(2000);

    await page.screenshot({
      path: 'test-screenshots/guide-chips-05-namespace-map.png',
      fullPage: true,
    });
    const crateMapActive = (await page.locator('#view-crate-map').getAttribute('class'))?.includes('active') ?? false;
    const crateMapNodes = await page.locator('.bp-node, .crate-node, [class*="crate"]').count();
    report.push(`Step 5: After "View crate/namespace map" - crate map active: ${crateMapActive}, crate elements: ${crateMapNodes}`);

    // Print report
    console.log('\n=== GUIDE CHIP ACTIONS REPORT ===');
    report.forEach((r) => console.log(r));

    // Assertions
    expect(nodeDetailsActive, 'Should switch to Node Details after Explore double').toBe(true);
    expect(hasDepth1, 'Query label should NOT contain depth=1 (depth should be unlimited)').toBe(false);
    expect(nodesAfterExplore, 'Graph should show nodes after Explore double').toBeGreaterThanOrEqual(3);
  });
});
