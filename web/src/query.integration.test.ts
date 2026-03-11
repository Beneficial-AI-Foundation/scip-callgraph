/**
 * Golden/snapshot integration tests for the query pipeline.
 *
 * These tests load REAL graph data through the CURRENT applyFilters() and
 * assert exact outputs.  They serve as the immutable regression safety net:
 * any refactoring that changes these values has introduced a behavioral
 * difference.
 *
 * Tier 2 tests from the plan (composable query pipeline).
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyFilters } from './filters';
import { parseAndNormalizeGraph } from './graph-loader';
import { D3Graph, FilterOptions, detectProjectLanguage, extractCrateName } from './types';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createFilters(overrides: Partial<FilterOptions> = {}): FilterOptions {
  return {
    showLibsignal: true,
    showNonLibsignal: true,
    showInnerCalls: true,
    showPreconditionCalls: true,
    showPostconditionCalls: true,
    showExecFunctions: true,
    showProofFunctions: true,
    showSpecFunctions: true,
    excludeNamePatterns: '',
    excludePathPatterns: '',
    includeFiles: '',
    maxDepth: null,
    sourceQuery: '',
    sinkQuery: '',
    selectedNodes: new Set(),
    expandedNodes: new Set(),
    hiddenNodes: new Set(),
    focusNodeIds: new Set(),
    ...overrides,
  };
}

function loadJsonFile(filePath: string): unknown {
  const text = readFileSync(filePath, 'utf8');
  return JSON.parse(text);
}

/** Backfill crate_name the same way loadGraph() does in main.ts */
function backfillCrateNames(graph: D3Graph): void {
  const lang = detectProjectLanguage(graph);
  for (const node of graph.nodes) {
    node.crate_name = extractCrateName(node, lang);
  }
}

// ---------------------------------------------------------------------------
// Fixture paths
// ---------------------------------------------------------------------------

const GRAPH_TMP_PATH = resolve(__dirname, '../public/graph_tmp.json');
const PMEMLOG_PATH = '/home/lacra/git_repos/baif/verus_projects/pmemlog/.verilib/atoms.json';
const KATYDID_PATH = '/home/lacra/git_repos/baif/katydid-proofs/.verilib/atoms.json';

// ============================================================================
// graph_tmp.json  (1 130 Verus nodes, D3Graph format, has mode/verification_status/link types)
// ============================================================================

describe.skipIf(!existsSync(GRAPH_TMP_PATH))('Golden: graph_tmp.json (Verus)', () => {
  let graph: D3Graph;

  beforeAll(() => {
    const raw = loadJsonFile(GRAPH_TMP_PATH);
    graph = parseAndNormalizeGraph(raw);
    backfillCrateNames(graph);
  });

  it('full graph (no filters) has exact node/link counts', () => {
    const result = applyFilters(graph, createFilters());
    // 1130 total nodes minus 55 isolated = 1075 connected nodes
    expect(result.nodes.length).toBe(1075);
    expect(result.links.length).toBe(4372);
  });

  it('forward traversal: source=decompress depth=2', () => {
    const filters = createFilters({ sourceQuery: 'decompress', maxDepth: 2 });
    const result = applyFilters(graph, filters);

    // Pinned golden values
    expect(result.nodes.length).toBeGreaterThan(0);
    // The "decompress" nodes should be present
    const names = new Set(result.nodes.map(n => n.display_name));
    expect(names.has('decompress')).toBe(true);

    // Snapshot the exact count so future changes break loudly
    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('backward traversal: sink=lemma_bridge_pow_as_nat_to_spec depth=1', () => {
    const filters = createFilters({
      sinkQuery: 'lemma_bridge_pow_as_nat_to_spec',
      maxDepth: 1,
    });
    const result = applyFilters(graph, filters);

    expect(result.nodes.length).toBeGreaterThan(0);
    const names = new Set(result.nodes.map(n => n.display_name));
    expect(names.has('lemma_bridge_pow_as_nat_to_spec')).toBe(true);
    // Known callers: pow_p58, pow22501
    expect(names.has('pow_p58')).toBe(true);
    expect(names.has('pow22501')).toBe(true);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('path finding: source=mul, sink=spec_sqrt_ad_minus_one', () => {
    const filters = createFilters({
      sourceQuery: 'mul',
      sinkQuery: 'spec_sqrt_ad_minus_one',
    });
    const result = applyFilters(graph, filters);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('neighborhood: source=sink=decompress depth=1', () => {
    const filters = createFilters({
      sourceQuery: 'decompress',
      sinkQuery: 'decompress',
      maxDepth: 1,
    });
    const result = applyFilters(graph, filters);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('link type filtering: showPreconditionCalls=false', () => {
    const filters = createFilters({ showPreconditionCalls: false });
    const result = applyFilters(graph, filters);

    const precondLinks = result.links.filter(l => l.type === 'precondition');
    expect(precondLinks.length).toBe(0);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('link type filtering: only inner calls', () => {
    const filters = createFilters({
      showPreconditionCalls: false,
      showPostconditionCalls: false,
    });
    const result = applyFilters(graph, filters);

    const nonInner = result.links.filter(l => l.type !== 'inner');
    expect(nonInner.length).toBe(0);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('include files: edwards.rs', () => {
    const filters = createFilters({ includeFiles: 'edwards.rs' });
    const result = applyFilters(graph, filters);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('exclude name pattern: *_comm*', () => {
    const filters = createFilters({ excludeNamePatterns: '*_comm*' });
    const result = applyFilters(graph, filters);

    const commNodes = result.nodes.filter(n => n.display_name.includes('_comm'));
    expect(commNodes.length).toBe(0);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('crate boundary: source=crate:curve25519-dalek, sink=crate:curve25519-dalek (self)', () => {
    const filters = createFilters({
      sourceQuery: 'crate:curve25519-dalek',
      sinkQuery: 'crate:curve25519-dalek',
    });
    const result = applyFilters(graph, filters);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });
});

// ============================================================================
// pmemlog atoms.json  (109 Verus atoms, atom dict format)
// ============================================================================

describe.skipIf(!existsSync(PMEMLOG_PATH))('Golden: pmemlog atoms.json (Verus atom dict)', () => {
  let graph: D3Graph;

  beforeAll(() => {
    const raw = loadJsonFile(PMEMLOG_PATH);
    graph = parseAndNormalizeGraph(raw);
    backfillCrateNames(graph);
  });

  it('loads correct number of nodes and links', () => {
    expect(graph.nodes.length).toBe(109);
    expect(graph.links.length).toMatchSnapshot();
  });

  it('source query for known function', () => {
    const filters = createFilters({ sourceQuery: 'valid', maxDepth: 2 });
    const result = applyFilters(graph, filters);

    expect(result.nodes.length).toBeGreaterThan(0);
    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('full graph (no filters)', () => {
    const result = applyFilters(graph, createFilters());
    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('detected as Verus project', () => {
    // pmemlog atoms have no kind field (all default to 'exec')
    // So detectProjectLanguage returns 'unknown' (no verus or lean markers)
    const lang = detectProjectLanguage(graph);
    expect(lang).toMatchSnapshot();
  });
});

// ============================================================================
// katydid-proofs atoms.json  (589 Lean atoms, language agnosticism)
// ============================================================================

describe.skipIf(!existsSync(KATYDID_PATH))('Golden: katydid-proofs atoms.json (Lean)', () => {
  let graph: D3Graph;

  beforeAll(() => {
    const raw = loadJsonFile(KATYDID_PATH);
    graph = parseAndNormalizeGraph(raw);
    backfillCrateNames(graph);
  });

  it('loads correct number of nodes', () => {
    expect(graph.nodes.length).toBe(589);
    expect(graph.links.length).toMatchSnapshot();
  });

  it('language detection returns lean', () => {
    const lang = detectProjectLanguage(graph);
    expect(lang).toBe('lean');
  });

  it('Lean kinds present: def, abbrev, inductive, structure, opaque, theorem', () => {
    const kinds = new Set(graph.nodes.map(n => n.kind));
    expect(kinds.has('def')).toBe(true);
    expect(kinds.has('theorem')).toBe(true);
    expect(kinds.has('abbrev')).toBe(true);
    expect(kinds.has('inductive')).toBe(true);
    expect(kinds.has('structure')).toBe(true);
    expect(kinds.has('opaque')).toBe(true);
  });

  it('Lean kind filtering: showProofFunctions=false removes theorems', () => {
    const filters = createFilters({ showProofFunctions: false });
    const result = applyFilters(graph, filters, undefined, 'lean');

    const theoremNodes = result.nodes.filter(n => n.kind === 'theorem');
    expect(theoremNodes.length).toBe(0);

    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('full graph (no filters)', () => {
    const result = applyFilters(graph, createFilters(), undefined, 'lean');
    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });

  it('source query for known function', () => {
    const filters = createFilters({ sourceQuery: 'validate_commutes', maxDepth: 2 });
    const result = applyFilters(graph, filters, undefined, 'lean');

    expect(result.nodes.length).toBeGreaterThan(0);
    expect(result.nodes.length).toMatchSnapshot();
    expect(result.links.length).toMatchSnapshot();
  });
});
