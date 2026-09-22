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

    // Clearing the selection returns to the seeded view, and the query label
    // names the seeded view rather than showing the stale selection query
    await page.locator('#clear-selection').click();
    await expect(stats).toContainText('entry points, depth 1', { timeout: 30000 });
    await expect(page.locator('#query-label')).toContainText('entry points');
  });
});

// ============================================================================
// Synthetic fixture: always runs (CI-safe, no sibling checkout needed).
// 2,100 nodes (> LARGE_GRAPH_NODE_THRESHOLD): 100 roots, each with 5 verified
// children (depth-1 view = 600 nodes / 500 links), plus a 1,500-node filler
// chain below the children to push the total over the threshold.
// ============================================================================

const SYNTHETIC_PUBLIC = path.resolve(__dirname, '../public/seeded-view-synthetic-fixture.json');

function makeSyntheticGraph() {
  const node = (id: string, verification_status?: string) => ({
    id,
    display_name: id,
    symbol: id,
    full_path: `/synthetic/${id}.rs`,
    relative_path: `src/${id}.rs`,
    file_name: `${id}.rs`,
    parent_folder: 'src',
    crate_name: 'synthetic',
    is_libsignal: false,
    dependencies: [],
    dependents: [],
    kind: 'def',
    ...(verification_status ? { verification_status } : {}),
  });
  const nodes = [];
  const links = [];
  for (let i = 0; i < 100; i++) {
    nodes.push(node(`r${i}`));
    for (let j = 0; j < 5; j++) {
      const child = `c${i * 5 + j}`;
      nodes.push(node(child, 'verified'));
      links.push({ source: `r${i}`, target: child, type: 'inner' });
    }
  }
  let prev = 'c0';
  for (let k = 0; k < 1500; k++) {
    const filler = `f${k}`;
    nodes.push(node(filler));
    links.push({ source: prev, target: filler, type: 'inner' });
    prev = filler;
  }
  return {
    nodes,
    links,
    metadata: {
      total_nodes: nodes.length,
      total_edges: links.length,
      project_root: '/synthetic',
      generated_at: '2026-01-01',
    },
  };
}

test.describe('Seeded initial view (synthetic large graph)', () => {
  test.beforeAll(() => {
    fs.writeFileSync(SYNTHETIC_PUBLIC, JSON.stringify(makeSyntheticGraph()));
  });

  test.afterAll(() => {
    fs.rmSync(SYNTHETIC_PUBLIC, { force: true });
  });

  test('renders the seeded view and applies display-only toggles on top', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-synthetic-fixture.json');
    const stats = page.locator('#stats');
    await expect(stats).toContainText('Showing 600 of 2,100 nodes (entry points, depth 1)', { timeout: 30000 });
    await expect(page.locator('#graph-container svg circle')).toHaveCount(600, { timeout: 30000 });

    // Display-only toggle: hiding verified nodes drops the 500 children but
    // stays in seeded mode (no reseeding, banner still present)
    await page.locator('#show-verified-nodes').setChecked(false);
    await expect(stats).toContainText('Showing 100 of 2,100 nodes (entry points, depth 1)', { timeout: 30000 });
    await page.locator('#show-verified-nodes').setChecked(true);
    await expect(stats).toContainText('Showing 600 of 2,100 nodes (entry points, depth 1)', { timeout: 30000 });
  });

  test('hide node (shift+click) applies on top of the seeded view', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-synthetic-fixture.json');
    const stats = page.locator('#stats');
    await expect(stats).toContainText('Showing 600 of 2,100 nodes', { timeout: 30000 });

    await page.locator('#graph-container svg circle').first().dispatchEvent('click', { shiftKey: true });
    await expect(stats).toContainText('Showing 599 of 2,100 nodes (entry points, depth 1)', { timeout: 30000 });
  });
});

// ============================================================================
// Explicit entry points (Phase 2): same synthetic graph, but with
// is_entry_point flags as the loader would derive them from is-public-api /
// the Aeneas join / @[blueprint]. 10 roots plus one non-root child are
// flagged, so the entry-points tier (61 nodes at depth 1) must win over the
// sources tier (600 nodes).
// ============================================================================

const ENTRYPOINT_PUBLIC = path.resolve(__dirname, '../public/seeded-view-entrypoint-fixture.json');

test.describe('Seeded initial view (explicit entry points)', () => {
  test.beforeAll(() => {
    const g = makeSyntheticGraph();
    const flagged = new Set(['r0', 'r1', 'r2', 'r3', 'r4', 'r5', 'r6', 'r7', 'r8', 'r9', 'c495']);
    for (const n of g.nodes) {
      if (flagged.has(n.id)) (n as Record<string, unknown>).is_entry_point = true;
    }
    fs.writeFileSync(ENTRYPOINT_PUBLIC, JSON.stringify(g));
  });

  test.afterAll(() => {
    fs.rmSync(ENTRYPOINT_PUBLIC, { force: true });
  });

  test('prefers the entry-points tier over the sources tier', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-entrypoint-fixture.json');
    const stats = page.locator('#stats');
    // 10 roots -> 50 children, plus the flagged non-root child c495
    await expect(stats).toContainText('Showing 61 of 2,100 nodes (entry points, depth 1)', { timeout: 30000 });
    await expect(stats).toContainText('Seeded from 11 explicit entry points (public API / blueprint)');
  });
});

// ============================================================================
// ?entrypoints= URL param (Phase 2): a probe-leanblueprint payload whose
// blueprint-label-carrying atoms seed the view. 8 of its 12 labeled atoms
// exist in the graph; the banner reports the matched/unmatched split. The
// failure test points ?entrypoints= at a JSON with no blueprint-label atoms
// and expects the fallback banner note on top of the sources-tier view.
// ============================================================================

const BLUEPRINT_GRAPH_PUBLIC = path.resolve(__dirname, '../public/seeded-view-blueprint-graph.json');
const BLUEPRINT_PAYLOAD_PUBLIC = path.resolve(__dirname, '../public/seeded-view-blueprint-payload.json');

test.describe('Seeded initial view (?entrypoints= blueprint payload)', () => {
  test.beforeAll(() => {
    fs.writeFileSync(BLUEPRINT_GRAPH_PUBLIC, JSON.stringify(makeSyntheticGraph()));
    const payload: Record<string, unknown> = {};
    for (const id of ['r0', 'r1', 'r2', 'r3', 'r4', 'r5', 'r6', 'r7']) {
      payload[id] = { 'blueprint-label': `label-${id}`, 'blueprint-kind': 'theorem' };
    }
    for (const id of ['ghost0', 'ghost1', 'ghost2', 'ghost3']) {
      payload[id] = { 'blueprint-label': `label-${id}` };
    }
    // Unlabeled atoms must not become seeds
    payload['r98'] = { 'display-name': 'r98' };
    // Synthetic blueprint-layer nodes (language: "blueprint") carry labels but
    // must count for neither the seeds nor the banner's denominator — even
    // when their id collides with a graph node (r99 is a real root here)
    payload['r99'] = { 'blueprint-label': 'label-r99', language: 'blueprint' };
    fs.writeFileSync(BLUEPRINT_PAYLOAD_PUBLIC, JSON.stringify(payload));
  });

  test.afterAll(() => {
    fs.rmSync(BLUEPRINT_GRAPH_PUBLIC, { force: true });
    fs.rmSync(BLUEPRINT_PAYLOAD_PUBLIC, { force: true });
  });

  test('seeds from matched blueprint declarations and reports the match count', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-blueprint-graph.json&entrypoints=./seeded-view-blueprint-payload.json');
    const stats = page.locator('#stats');
    // 8 matched roots -> 40 children (the async fetch re-renders the initial
    // sources-tier view, so wait for the final banner)
    await expect(stats).toContainText('Showing 48 of 2,100 nodes (entry points, depth 1)', { timeout: 30000 });
    await expect(stats).toContainText('Seeded from 8 of 12 blueprint declarations (?entrypoints=) matched in this graph');
    // The param survives in the shareable URL
    expect(page.url()).toContain('entrypoints=');
  });

  test('falls through the normal seed chain with a banner note when the payload has no blueprint labels', async ({ page }) => {
    // The graph file itself is valid JSON but carries no blueprint-label atoms
    await page.goto('/scip-callgraph/?json=./seeded-view-blueprint-graph.json&entrypoints=./seeded-view-blueprint-graph.json');
    const stats = page.locator('#stats');
    await expect(stats).toContainText('?entrypoints= no atoms carry blueprint-label', { timeout: 30000 });
    // Sources-tier view still renders
    await expect(stats).toContainText('Showing 600 of 2,100 nodes (entry points, depth 1)');
    await expect(stats).toContainText('root functions (no callers)');
  });
});

// ============================================================================
// Depth preservation across the async ?entrypoints= re-seed: the provisional
// sources-tier render can only achieve depth 1 here (300 roots -> 900 nodes at
// depth 1, 3,900 at depth 2 > the 2,000 budget) and commits that to the
// slider, but the blueprint tier arriving later must retry the URL's ?depth=3,
// not the fallback's achieved depth.
// ============================================================================

const DEPTH_GRAPH_PUBLIC = path.resolve(__dirname, '../public/seeded-view-depth-graph.json');
const DEPTH_PAYLOAD_PUBLIC = path.resolve(__dirname, '../public/seeded-view-depth-payload.json');

test.describe('Seeded view depth request survives the async blueprint re-seed', () => {
  test.beforeAll(() => {
    const node = (id: string) => ({
      id, display_name: id, symbol: id,
      full_path: `/synthetic/${id}.rs`, relative_path: `src/${id}.rs`,
      file_name: `${id}.rs`, parent_folder: 'src', crate_name: 'synthetic',
      is_libsignal: false, dependencies: [], dependents: [], kind: 'def',
    });
    const nodes = [];
    const links = [];
    for (let i = 0; i < 300; i++) {
      nodes.push(node(`r${i}`));
      for (let j = 0; j < 2; j++) {
        const child = `m${i * 2 + j}`;
        nodes.push(node(child));
        links.push({ source: `r${i}`, target: child, type: 'inner' });
        for (let k = 0; k < 5; k++) {
          const leaf = `l${(i * 2 + j) * 5 + k}`;
          nodes.push(node(leaf));
          links.push({ source: child, target: leaf, type: 'inner' });
        }
      }
    }
    const g = {
      nodes, links,
      metadata: { total_nodes: nodes.length, total_edges: links.length, project_root: '/synthetic', generated_at: '2026-01-01' },
    };
    fs.writeFileSync(DEPTH_GRAPH_PUBLIC, JSON.stringify(g));
    const payload: Record<string, unknown> = {};
    for (const id of ['r0', 'r1', 'r2']) payload[id] = { 'blueprint-label': `label-${id}` };
    fs.writeFileSync(DEPTH_PAYLOAD_PUBLIC, JSON.stringify(payload));
  });

  test.afterAll(() => {
    fs.rmSync(DEPTH_GRAPH_PUBLIC, { force: true });
    fs.rmSync(DEPTH_PAYLOAD_PUBLIC, { force: true });
  });

  test('re-seeds at the requested depth once the blueprint payload arrives', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-depth-graph.json&depth=3&entrypoints=./seeded-view-depth-payload.json');
    const stats = page.locator('#stats');
    // 3 blueprint roots -> 6 children -> 30 leaves, at the requested depth 3
    // (saturated at 2), not at the provisional sources tier's achieved depth 1
    await expect(stats).toContainText('Showing 39 of 3,900 nodes (entry points, depth 3)', { timeout: 30000 });
    await expect(page.locator('#depth-value')).toHaveText('3');
  });
});

// ============================================================================
// Fixture where every seed tier exceeds the budget: 2,100 isolated nodes,
// so all of them are in-degree-0 seeds (> 2,000 node budget). The viewer must
// fall back to the "use filters" message, and — regression guard — the depth
// slider must keep its normal behavior of pre-setting maxDepth for a later
// query instead of refusing.
// ============================================================================

const UNSEEDABLE_PUBLIC = path.resolve(__dirname, '../public/seeded-view-unseedable-fixture.json');

test.describe('Large graph where seeding fails', () => {
  test.beforeAll(() => {
    const g = makeSyntheticGraph();
    fs.writeFileSync(UNSEEDABLE_PUBLIC, JSON.stringify({ ...g, links: [] }));
  });

  test.afterAll(() => {
    fs.rmSync(UNSEEDABLE_PUBLIC, { force: true });
  });

  test('falls back to the filter prompt and keeps the depth slider usable', async ({ page }) => {
    await page.goto('/scip-callgraph/?json=./seeded-view-unseedable-fixture.json');
    const stats = page.locator('#stats');
    await expect(stats).toContainText('Large Graph', { timeout: 30000 });
    await expect(stats).not.toContainText('entry points');

    // Pre-setting depth for a later query must not be refused or reverted
    await page.locator('#depth-limit').fill('3');
    await expect(page.locator('#depth-value')).toHaveText('3');
    await expect(stats).not.toContainText('would need');
  });
});
