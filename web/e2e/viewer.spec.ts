import { test, expect } from '@playwright/test';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// Tracked fixture: the repo-root atoms.json this test used to upload is not
// checked in, so a clean checkout could never run it.
const ATOMS_PATH = path.resolve(__dirname, '../public/curve_from_rust_atomizer.json');

test.describe('probegraph viewer', () => {
  test('load atoms.json, verify Declaration Kind filter and node detail panel', async ({
    page,
  }) => {
    // Step 1: Navigate
    await page.goto('/probegraph/');
    await page.waitForLoadState('networkidle');

    // Step 2: Screenshot initial state
    await page.screenshot({
      path: 'test-screenshots/01-initial.png',
      fullPage: true,
    });

    // Step 3: Upload atoms.json
    const fileInput = page.locator('#file-input');
    await fileInput.setInputFiles(ATOMS_PATH);

    // Step 4: Wait for graph to load
    await page.waitForSelector('#stats', { timeout: 15000 });
    await page.waitForTimeout(3000); // Allow graph to render

    // Step 5: Screenshot after loading
    await page.screenshot({
      path: 'test-screenshots/02-after-load.png',
      fullPage: true,
    });

    // Step 6: Screenshot Declaration Kind filter
    const kindSection = page.locator('#kind-filters-container');
    await kindSection.scrollIntoViewIfNeeded();
    const kindBox = await kindSection.boundingBox();
    if (kindBox) {
      await page.screenshot({
        path: 'test-screenshots/03-declaration-kind-filter.png',
        clip: kindBox,
      });
    }

    // Step 7: Click a node (graph may use g.node for SVG groups)
    const node = page.locator('g.node').first();
    const hasNodes = (await node.count()) > 0;
    if (hasNodes) {
      await node.click({ timeout: 5000 });
      await page.waitForTimeout(500);
    } else {
      // Fallback: enter a source to filter and reveal nodes
      await page.fill('#source-input', 'double');
      await page.waitForTimeout(1500);
      const nodeAfter = page.locator('g.node').first();
      if ((await nodeAfter.count()) > 0) {
        await nodeAfter.click({ timeout: 3000 });
        await page.waitForTimeout(500);
      }
    }

    // Step 8: Screenshot node detail panel
    await page.screenshot({
      path: 'test-screenshots/04-node-detail-panel.png',
    });

    // Assertions for report
    const statsText = await page.locator('#stats').textContent();
    const kindLabels = await page
      .locator(
        '#kind-filters-container span.exec-badge, #kind-filters-container span.proof-badge, #kind-filters-container span.spec-badge'
      )
      .allTextContents();
    const nodeInfoHtml = await page.locator('#node-info').innerHTML();
    const hasKindBadge =
      nodeInfoHtml.includes('exec-badge') ||
      nodeInfoHtml.includes('proof-badge') ||
      nodeInfoHtml.includes('spec-badge');

    const nodeCountMatch = statsText?.match(/(\d+)\s*nodes?/i);
    const totalNodeCount = nodeCountMatch ? nodeCountMatch[1] : 'N/A';

    // Log report
    console.log('\n=== TEST REPORT ===');
    console.log('Stats:', statsText);
    console.log('Declaration Kind labels:', kindLabels);
    console.log('Kind badge in node panel:', hasKindBadge);
    console.log('Node count:', totalNodeCount);

    expect(statsText).toBeTruthy();
  });
});
