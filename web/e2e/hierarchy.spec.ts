/**
 * E2E test for the Hierarchy drill-down view.
 *
 * Uses the tracked multi-crate fixture graph_backup_dalek.json. Clicks on the
 * SVG member boxes are dispatched programmatically: Playwright's coordinate
 * clicks are flaky on elements the force/dagre layout can move between
 * resolving the locator and clicking.
 */

import { test, expect } from '@playwright/test';

const APP_URL = '/probegraph/?json=./graph_backup_dalek.json&view=hierarchy';

test.describe('Hierarchy view', () => {
  test('drills from crate overview to function level and back', async ({ page }) => {
    await page.goto(APP_URL);

    // Collapsed overview: one box per crate, no containers, aggregated edges
    await expect(page.locator('.hm-group').first()).toBeVisible({ timeout: 30000 });
    const crateCount = await page.locator('.hm-group').count();
    expect(crateCount).toBeGreaterThan(10);
    await expect(page.locator('.hm-container')).toHaveCount(0);
    expect(await page.locator('.hm-edge').count()).toBeGreaterThan(0);

    // Expand the first crate: it becomes a container, the URL records it
    await page.locator('.hm-group').first().dispatchEvent('click');
    await expect(page.locator('.hm-container')).toHaveCount(1);
    expect(page.url()).toContain('expanded=');

    // Drill until function boxes appear (bounded: the fixture's first crate
    // reaches file level within a few levels)
    for (let i = 0; i < 5; i++) {
      if (await page.locator('.hm-fn-group').count() > 0) break;
      await page.locator('.hm-group').first().dispatchEvent('click');
      await page.waitForTimeout(300);
    }
    await expect(page.locator('.hm-fn-group').first()).toBeVisible();

    // Clicking a function opens the details panel
    await page.locator('.hm-fn-group').first().dispatchEvent('click');
    await expect(page.locator('#node-info')).not.toContainText('Click or hover', { timeout: 10000 });

    // Reloading the shareable URL restores the expansion state
    const containersBefore = await page.locator('.hm-container').count();
    await page.goto(page.url());
    await expect(page.locator('.hm-container')).toHaveCount(containersBefore, { timeout: 30000 });
    expect(await page.locator('.hm-fn-group').count()).toBeGreaterThan(0);

    // Esc collapses everything and clears the URL state
    await page.keyboard.press('Escape');
    await expect(page.locator('.hm-container')).toHaveCount(0);
    await expect(page.locator('.hm-group')).toHaveCount(crateCount);
    expect(page.url()).not.toContain('expanded=');
  });

  test('survives switching to Call Graph and back', async ({ page }) => {
    await page.goto(APP_URL);
    await expect(page.locator('.hm-group').first()).toBeVisible({ timeout: 30000 });
    const crateCount = await page.locator('.hm-group').count();

    await page.locator('#view-callgraph').click();
    await expect(page.locator('.hm-group')).toHaveCount(0);

    await page.locator('#view-hierarchy').click();
    await expect(page.locator('.hm-group')).toHaveCount(crateCount, { timeout: 30000 });
    expect(page.url()).toContain('view=hierarchy');
  });
});
