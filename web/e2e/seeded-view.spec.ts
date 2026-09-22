/**
 * E2E test for the entry-point-seeded initial view on a large graph.
 *
 * Uses the sm-import-test merged fixture (1,547 nodes / 22,479 links through
 * the loader, 6 MB file): it trips the link threshold, so without seeding the
 * page rendered nothing. Skipped when the fixture repo is not checked out
 * alongside this one.
 *
 * Known budget behavior of this graph (133 in-degree-0 seeds):
 * depth 1 = 648 nodes / 4,514 links, depth 2 = 877 / 7,010,
 * depth 3 = 1,043 / 9,607, depth 4 = 13,688 links -> refused (limit 10,000).
 */

import { test, expect } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE_SRC = path.resolve(
  __dirname,
  '../../../sm-import-test/.verilib/probes/merged_smimporttest_securemessaging.json',
);
const FIXTURE_PUBLIC = path.resolve(__dirname, '../public/seeded-view-e2e-fixture.json');

test.describe('Seeded initial view (large graph)', () => {
  test.skip(!fs.existsSync(FIXTURE_SRC), 'sm-import-test fixture not checked out');

  test.beforeAll(() => {
    fs.copyFileSync(FIXTURE_SRC, FIXTURE_PUBLIC);
  });

  test.afterAll(() => {
    fs.rmSync(FIXTURE_PUBLIC, { force: true });
  });

  test('renders a bounded seeded view instead of a blank page', async ({ page }) => {
    const start = Date.now();
    await page.goto('/scip-callgraph/?json=./seeded-view-e2e-fixture.json');

    // The 6 MB file is under the 10 MiB auto-load cap, so it loads without
    // the "Load & Search" prompt, and the seeded banner appears.
    const stats = page.locator('#stats');
    await expect(stats).toContainText('entry points, depth 1', { timeout: 30000 });
    await expect(stats).toContainText('Showing 648 of 1,547 nodes');
    console.log(`seeded view banner visible after ${Date.now() - start} ms`);

    // The graph actually rendered: one circle per node in the seeded view
    await expect(page.locator('#graph-container svg circle')).toHaveCount(648, { timeout: 30000 });
  });

  test('depth slider commits admissible depths and refuses over-budget ones', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-e2e-fixture.json');
    const stats = page.locator('#stats');
    await expect(stats).toContainText('entry points, depth 1', { timeout: 30000 });

    const slider = page.locator('#depth-limit');

    // Depth 2 fits the budget: transactional commit updates banner and label.
    // The expansion has 877 nodes; the default display filters (spec/axiom
    // kinds hidden) drop 2 of them on top of the seeded view.
    await slider.fill('2');
    await expect(stats).toContainText('entry points, depth 2', { timeout: 30000 });
    await expect(stats).toContainText('Showing 875 of 1,547 nodes');
    await expect(page.locator('#depth-value')).toHaveText('2');

    // Depth 4 exceeds the link budget: refused, view and label unchanged
    await slider.fill('4');
    await expect(stats).toContainText('would need 13,688 links (limit 10,000)', { timeout: 30000 });
    await expect(stats).toContainText('entry points, depth 2');
    await expect(page.locator('#depth-value')).toHaveText('2');
    await expect(slider).toHaveValue('2');
  });

  test('node selection exits seeded mode without blanking; reset returns to it', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-e2e-fixture.json');
    const stats = page.locator('#stats');
    await expect(stats).toContainText('entry points, depth 1', { timeout: 30000 });

    // Click a rendered node (programmatically: the force layout can place it
    // outside the viewport): selection counts as a filter, so the view
    // becomes the selected node's depth-limited neighborhood, not empty.
    await page.locator('#graph-container svg circle').first().dispatchEvent('click');
    await expect(page.locator('#query-label')).toContainText('selected', { timeout: 30000 });
    await expect(page.locator('#graph-container svg circle').first()).toBeVisible();

    // Clearing the selection returns to the seeded view
    await page.locator('#clear-selection').click();
    await expect(stats).toContainText('entry points, depth 1', { timeout: 30000 });
  });
});
