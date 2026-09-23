/**
 * Manual test scenarios for the probegraph viewer
 * Run with: npx playwright test e2e/manual-test-scenarios.spec.ts
 * Against user's dev server (port 3000): BASE_URL=http://localhost:3000 npx playwright test e2e/manual-test-scenarios.spec.ts
 */
import { test, expect } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const BASE_URL = process.env.BASE_URL || 'http://localhost:3001';
// Pinned to a tracked fixture (1,333 nodes, single crate, contains
// "decompress") instead of the untracked graph_tmp.json.
const GRAPH_URL = `${BASE_URL}/probegraph/?json=./curve_from_rust_atomizer.json`;
const FULL_NODE_COUNT = 1333;

// Test D needs a Verus-language graph (the Requires/Ensures checkboxes only
// render for verus/mixed): a minimal atom dict with typed dependency edges.
const VERUS_FIXTURE_PUBLIC = path.resolve(__dirname, '../public/manual-verus-fixture.json');

function makeVerusAtomDict() {
  const atom = (name: string, kind: string, deps: Array<[string, string]>) => ({
    'display-name': name,
    dependencies: deps.map(([d]) => `probe:mini/${d}`),
    'dependencies-with-locations': deps.map(([d, location]) => ({
      'code-name': `probe:mini/${d}`, location, line: 1,
    })),
    'code-text': { 'lines-start': 1, 'lines-end': 5 },
    'code-path': 'src/lib.rs',
    'code-module': 'mini',
    kind,
  });
  return {
    'probe:mini/spec_ok': atom('spec_ok', 'spec', []),
    'probe:mini/helper': atom('helper', 'exec', []),
    'probe:mini/main_op': atom('main_op', 'exec', [['helper', 'inner']]),
    // The postcondition edge targets an exec node: spec-kind nodes are hidden
    // by default, which would hide the edge with them.
    'probe:mini/lemma_op': atom('lemma_op', 'proof', [
      ['spec_ok', 'precondition'],
      ['helper', 'postcondition'],
    ]),
  };
}

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
    expect(nodeCount).toBeGreaterThan(0);
    expect(nodeCount).toBeLessThan(FULL_NODE_COUNT); // Reduced from full graph

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

  test('Test D: Link type filtering - toggling Show postcondition changes edge count', async ({
    page,
  }) => {
    fs.writeFileSync(VERUS_FIXTURE_PUBLIC, JSON.stringify(makeVerusAtomDict()));
    try {
      await page.goto(`${BASE_URL}/probegraph/?json=./manual-verus-fixture.json`);
      await page.waitForLoadState('networkidle');
      await page.waitForSelector('#stats', { timeout: 15000 });
      await page.waitForTimeout(1500);

      const edgeCount = async () => {
        const statsText = await page.locator('#stats').textContent();
        const m = statsText?.match(/Total Edges:\s*(\d+)/);
        return m ? parseInt(m[1], 10) : -1;
      };

      // Postcondition (Ensures) edges are hidden by default: 1 inner edge.
      // The lemma's precondition/postcondition edges appear when checked.
      const baseline = await edgeCount();
      expect(baseline).toBe(1);

      const postCheckbox = page.locator('#show-postcondition-calls');
      await postCheckbox.check();
      await page.waitForTimeout(800);
      expect(await edgeCount()).toBe(baseline + 1);

      await postCheckbox.uncheck();
      await page.waitForTimeout(800);
      expect(await edgeCount()).toBe(baseline);

      await page.screenshot({
        path: 'test-screenshots/D-postcondition-unchecked.png',
        fullPage: true,
      });
    } finally {
      fs.rmSync(VERUS_FIXTURE_PUBLIC, { force: true });
    }
  });
});
