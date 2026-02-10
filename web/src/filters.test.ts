/**
 * Tests for the call graph filtering logic.
 * 
 * These tests focus on user-facing behavior, not implementation details.
 * A failing test should indicate a real bug that affects users.
 */

import { describe, it, expect } from 'vitest';
import { globToRegex, matchesQuery, applyFilters, pathPatternToRegex } from './filters';
import { D3Graph, D3Node, D3Link, FilterOptions } from './types';

// ============================================================================
// Test Helpers
// ============================================================================

/** Create a minimal D3Node for testing */
function createNode(props: {
  id: string;
  display_name: string;
  file_name?: string;
  parent_folder?: string;
  relative_path?: string;
  mode?: 'exec' | 'proof' | 'spec';
  is_libsignal?: boolean;
}): D3Node {
  return {
    id: props.id,
    display_name: props.display_name,
    symbol: props.id,
    full_path: `/path/to/${props.display_name}.rs`,
    relative_path: props.relative_path || `src/${props.display_name}.rs`,
    file_name: props.file_name || 'test.rs',
    parent_folder: props.parent_folder || 'src',
    is_libsignal: props.is_libsignal ?? true,
    dependencies: [],
    dependents: [],
    mode: props.mode || 'exec',
  };
}

/** Create a minimal D3Link for testing */
function createLink(source: string, target: string, type: string = 'inner'): D3Link {
  return { source, target, type };
}

/** Create default filter options */
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

/** Create a simple test graph: A → B → C */
function createLinearGraph(): D3Graph {
  return {
    nodes: [
      createNode({ id: 'a', display_name: 'func_a', file_name: 'module_a.rs' }),
      createNode({ id: 'b', display_name: 'func_b', file_name: 'module_b.rs' }),
      createNode({ id: 'c', display_name: 'func_c', file_name: 'module_c.rs' }),
    ],
    links: [
      createLink('a', 'b'),
      createLink('b', 'c'),
    ],
    metadata: { total_nodes: 3, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
  };
}

/**
 * Create a diamond graph for path testing:
 *     A
 *    / \
 *   B   C
 *    \ /
 *     D
 */
function createDiamondGraph(): D3Graph {
  return {
    nodes: [
      createNode({ id: 'a', display_name: 'start' }),
      createNode({ id: 'b', display_name: 'left_path' }),
      createNode({ id: 'c', display_name: 'right_path' }),
      createNode({ id: 'd', display_name: 'end' }),
    ],
    links: [
      createLink('a', 'b'),
      createLink('a', 'c'),
      createLink('b', 'd'),
      createLink('c', 'd'),
    ],
    metadata: { total_nodes: 4, total_edges: 4, project_root: '/test', generated_at: '2024-01-01' },
  };
}

// ============================================================================
// Category 1: Glob Pattern Matching
// ============================================================================

describe('Glob Pattern Matching', () => {
  describe('globToRegex', () => {
    it('wildcard * matches any characters including empty', () => {
      const regex = globToRegex('*foo*');
      expect(regex.test('foo')).toBe(true);
      expect(regex.test('xfooy')).toBe(true);
      expect(regex.test('before_foo_after')).toBe(true);
    });

    it('pattern without wildcards requires exact match', () => {
      const regex = globToRegex('foo');
      expect(regex.test('foo')).toBe(true);
      expect(regex.test('xfooy')).toBe(false);
      expect(regex.test('foobar')).toBe(false);
    });

    it('regex special characters are escaped properly', () => {
      // foo.bar should match "foo.bar", not "fooXbar"
      const regex = globToRegex('foo.bar');
      expect(regex.test('foo.bar')).toBe(true);
      expect(regex.test('fooXbar')).toBe(false);
    });

    it('matching is case-insensitive', () => {
      const regex = globToRegex('FooBar');
      expect(regex.test('foobar')).toBe(true);
      expect(regex.test('FOOBAR')).toBe(true);
      expect(regex.test('FooBar')).toBe(true);
    });

    it('? matches exactly one character', () => {
      const regex = globToRegex('fo?');
      expect(regex.test('foo')).toBe(true);
      expect(regex.test('fox')).toBe(true);
      expect(regex.test('fo')).toBe(false);
      expect(regex.test('fooo')).toBe(false);
    });

    it('prefix pattern works', () => {
      const regex = globToRegex('lemma_*');
      expect(regex.test('lemma_foo')).toBe(true);
      expect(regex.test('lemma_')).toBe(true);
      expect(regex.test('not_lemma_foo')).toBe(false);
    });

    it('suffix pattern works', () => {
      const regex = globToRegex('*_comm');
      expect(regex.test('lemma_comm')).toBe(true);
      expect(regex.test('add_comm')).toBe(true);
      expect(regex.test('comm_extra')).toBe(false);
    });
  });
});

// ============================================================================
// Category 2: Path-Qualified Queries
// ============================================================================

describe('Path-Qualified Queries', () => {
  const nodeInEdwards = createNode({
    id: 'edwards::decompress',
    display_name: 'decompress',
    file_name: 'edwards.rs',
    parent_folder: 'curve',
  });

  const nodeInRistretto = createNode({
    id: 'ristretto::compress',
    display_name: 'compress',
    file_name: 'ristretto.rs',
    parent_folder: 'curve',
  });

  it('path::function syntax matches function in specific file', () => {
    expect(matchesQuery(nodeInEdwards, 'edwards::decompress')).toBe(true);
    expect(matchesQuery(nodeInEdwards, 'ristretto::decompress')).toBe(false);
  });

  it('path part matches file name without .rs extension', () => {
    // "edwards" should match "edwards.rs"
    expect(matchesQuery(nodeInEdwards, 'edwards::decompress')).toBe(true);
  });

  it('path part can use wildcards', () => {
    expect(matchesQuery(nodeInEdwards, 'ed*::decompress')).toBe(true);
    expect(matchesQuery(nodeInEdwards, '*wards::decompress')).toBe(true);
  });

  it('function part can use wildcards', () => {
    expect(matchesQuery(nodeInEdwards, 'edwards::*')).toBe(true);
    expect(matchesQuery(nodeInEdwards, 'edwards::*compress')).toBe(true);
  });

  it('simple query without :: uses display_name matching', () => {
    expect(matchesQuery(nodeInEdwards, 'decompress')).toBe(true);
    expect(matchesQuery(nodeInEdwards, '*compress')).toBe(true);
    expect(matchesQuery(nodeInRistretto, 'compress')).toBe(true);
  });
});

// ============================================================================
// Category 3: Graph Traversal (Source/Sink/Paths)
// ============================================================================

describe('Graph Traversal', () => {
  describe('Source query (shows callees)', () => {
    it('returns source node and its immediate callees at depth 1', () => {
      const graph = createLinearGraph();
      const filters = createFilters({ sourceQuery: 'func_a', maxDepth: 1 });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name).sort();
      
      expect(nodeNames).toContain('func_a');
      expect(nodeNames).toContain('func_b');
      // func_c is at depth 2, should NOT be included
      expect(nodeNames).not.toContain('func_c');
    });

    it('returns deeper callees when depth allows', () => {
      const graph = createLinearGraph();
      const filters = createFilters({ sourceQuery: 'func_a', maxDepth: 2 });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name).sort();
      
      expect(nodeNames).toContain('func_a');
      expect(nodeNames).toContain('func_b');
      expect(nodeNames).toContain('func_c');
    });
  });

  describe('Sink query (shows callers)', () => {
    it('returns sink node and its immediate callers at depth 1', () => {
      const graph = createLinearGraph();
      const filters = createFilters({ sinkQuery: 'func_c', maxDepth: 1 });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name).sort();
      
      expect(nodeNames).toContain('func_c');
      expect(nodeNames).toContain('func_b');
      // func_a is at depth 2, should NOT be included
      expect(nodeNames).not.toContain('func_a');
    });
  });

  describe('Source → Sink path finding', () => {
    it('finds all nodes on path between source and sink', () => {
      const graph = createDiamondGraph();
      const filters = createFilters({ sourceQuery: 'start', sinkQuery: 'end' });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name).sort();
      
      // All nodes should be on some path from start to end
      expect(nodeNames).toContain('start');
      expect(nodeNames).toContain('end');
      // Both paths should be included
      expect(nodeNames).toContain('left_path');
      expect(nodeNames).toContain('right_path');
    });

    it('returns empty when no path exists', () => {
      const graph = createLinearGraph();
      // Try to find path from C to A (wrong direction)
      const filters = createFilters({ sourceQuery: 'func_c', sinkQuery: 'func_a' });
      
      const result = applyFilters(graph, filters);
      
      // No path from C to A, so should be empty or just source/sink
      expect(result.nodes.length).toBeLessThanOrEqual(2);
    });
  });

  describe('Same source and sink (neighborhood mode)', () => {
    it('shows both callers and callees of the node', () => {
      const graph = createLinearGraph();
      // B is in the middle, should show A (caller) and C (callee)
      const filters = createFilters({ sourceQuery: 'func_b', sinkQuery: 'func_b', maxDepth: 1 });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name).sort();
      
      expect(nodeNames).toContain('func_a'); // caller
      expect(nodeNames).toContain('func_b'); // self
      expect(nodeNames).toContain('func_c'); // callee
    });
  });
});

// ============================================================================
// Category 4: Edge Cases That Could Crash
// ============================================================================

describe('Edge Cases', () => {
  it('empty graph returns empty result without crashing', () => {
    const emptyGraph: D3Graph = {
      nodes: [],
      links: [],
      metadata: { total_nodes: 0, total_edges: 0, project_root: '/test', generated_at: '2024-01-01' },
    };
    const filters = createFilters({ sourceQuery: 'anything' });
    
    const result = applyFilters(emptyGraph, filters);
    
    expect(result.nodes).toHaveLength(0);
    expect(result.links).toHaveLength(0);
  });

  it('query matching nothing returns empty result without crashing', () => {
    const graph = createLinearGraph();
    const filters = createFilters({ sourceQuery: 'nonexistent_function' });
    
    const result = applyFilters(graph, filters);
    
    expect(result.nodes).toHaveLength(0);
  });

  it('graph with cycle terminates without infinite loop', () => {
    // A → B → C → A (cycle)
    const cyclicGraph: D3Graph = {
      nodes: [
        createNode({ id: 'a', display_name: 'func_a' }),
        createNode({ id: 'b', display_name: 'func_b' }),
        createNode({ id: 'c', display_name: 'func_c' }),
      ],
      links: [
        createLink('a', 'b'),
        createLink('b', 'c'),
        createLink('c', 'a'), // Creates cycle
      ],
      metadata: { total_nodes: 3, total_edges: 3, project_root: '/test', generated_at: '2024-01-01' },
    };
    const filters = createFilters({ sourceQuery: 'func_a', maxDepth: 10 });
    
    // This should complete without hanging
    const result = applyFilters(cyclicGraph, filters);
    
    // All nodes should be reachable
    expect(result.nodes.length).toBeGreaterThan(0);
  });

  it('self-referential node without other connections is treated as isolated', () => {
    // A node that only calls itself with no other connections is considered isolated
    // and removed from the filtered graph. This is expected behavior.
    const selfRefGraph: D3Graph = {
      nodes: [
        createNode({ id: 'recursive', display_name: 'recursive_fn' }),
      ],
      links: [
        createLink('recursive', 'recursive'), // Calls itself
      ],
      metadata: { total_nodes: 1, total_edges: 1, project_root: '/test', generated_at: '2024-01-01' },
    };
    const filters = createFilters({ sourceQuery: 'recursive_fn', maxDepth: 5 });
    
    const result = applyFilters(selfRefGraph, filters);
    
    // Self-referential nodes are filtered out as isolated (no connections to OTHER nodes)
    expect(result.nodes).toHaveLength(0);
  });

  it('self-referential node WITH other connections is preserved', () => {
    // When a recursive function has other callers/callees, it should be shown
    const graph: D3Graph = {
      nodes: [
        createNode({ id: 'caller', display_name: 'caller_fn' }),
        createNode({ id: 'recursive', display_name: 'recursive_fn' }),
      ],
      links: [
        createLink('caller', 'recursive'),
        createLink('recursive', 'recursive'), // Self-loop
      ],
      metadata: { total_nodes: 2, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
    };
    const filters = createFilters({ sinkQuery: 'recursive_fn', maxDepth: 1 });
    
    const result = applyFilters(graph, filters);
    const nodeNames = result.nodes.map(n => n.display_name);
    
    expect(nodeNames).toContain('recursive_fn');
    expect(nodeNames).toContain('caller_fn');
  });
});

// ============================================================================
// Category 5: Filter Behavior
// ============================================================================

describe('Filter Behavior', () => {
  describe('Mode filters', () => {
    it('hiding spec functions removes them from results', () => {
      const graph: D3Graph = {
        nodes: [
          createNode({ id: 'exec_fn', display_name: 'exec_fn', mode: 'exec' }),
          createNode({ id: 'spec_fn', display_name: 'spec_fn', mode: 'spec' }),
          createNode({ id: 'proof_fn', display_name: 'proof_fn', mode: 'proof' }),
        ],
        links: [
          createLink('exec_fn', 'spec_fn'),
          createLink('exec_fn', 'proof_fn'),
        ],
        metadata: { total_nodes: 3, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      const filters = createFilters({
        sourceQuery: 'exec_fn',
        maxDepth: 1,
        showSpecFunctions: false,
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      expect(nodeNames).toContain('exec_fn');
      expect(nodeNames).toContain('proof_fn');
      expect(nodeNames).not.toContain('spec_fn');
    });
  });

  describe('Hidden nodes', () => {
    it('hidden nodes are excluded from results', () => {
      const graph = createLinearGraph();
      const filters = createFilters({
        sourceQuery: 'func_a',
        maxDepth: 2,
        hiddenNodes: new Set(['b']), // Hide func_b
      });
      
      const result = applyFilters(graph, filters);
      const nodeIds = result.nodes.map(n => n.id);
      
      expect(nodeIds).not.toContain('b');
    });
  });

  describe('Exclude patterns', () => {
    it('exclude name pattern removes matching functions', () => {
      const graph: D3Graph = {
        nodes: [
          createNode({ id: 'main', display_name: 'main' }),
          createNode({ id: 'lemma_comm', display_name: 'lemma_comm' }),
          createNode({ id: 'helper', display_name: 'helper' }),
        ],
        links: [
          createLink('main', 'lemma_comm'),
          createLink('main', 'helper'),
        ],
        metadata: { total_nodes: 3, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      const filters = createFilters({
        sourceQuery: 'main',
        maxDepth: 1,
        excludeNamePatterns: '*_comm',
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      expect(nodeNames).toContain('main');
      expect(nodeNames).toContain('helper');
      expect(nodeNames).not.toContain('lemma_comm');
    });
  });

  describe('Include files filter', () => {
    it('only includes functions from specified files', () => {
      const graph: D3Graph = {
        nodes: [
          createNode({ id: 'a', display_name: 'func_a', file_name: 'edwards.rs' }),
          createNode({ id: 'b', display_name: 'func_b', file_name: 'ristretto.rs' }),
          createNode({ id: 'c', display_name: 'func_c', file_name: 'edwards.rs' }),
        ],
        links: [
          createLink('a', 'b'),
          createLink('a', 'c'),
        ],
        metadata: { total_nodes: 3, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      const filters = createFilters({
        sourceQuery: 'func_a',
        maxDepth: 1,
        includeFiles: 'edwards.rs',
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      expect(nodeNames).toContain('func_a');
      expect(nodeNames).toContain('func_c');
      expect(nodeNames).not.toContain('func_b'); // In ristretto.rs
    });

    it('shows edges between files when using includeFiles alone (no source/sink)', () => {
      // Simulates: hazmat.rs has ExpandedSecretKey::from_bytes which calls Scalar::from_bytes_mod_order in scalar.rs
      const graph: D3Graph = {
        nodes: [
          createNode({ id: 'hazmat_from_bytes', display_name: 'from_bytes', file_name: 'hazmat.rs' }),
          createNode({ id: 'scalar_from_bytes_mod_order', display_name: 'from_bytes_mod_order', file_name: 'scalar.rs' }),
          createNode({ id: 'other_func', display_name: 'other_func', file_name: 'other.rs' }),
          createNode({ id: 'another_func', display_name: 'another_func', file_name: 'another.rs' }),
        ],
        links: [
          createLink('hazmat_from_bytes', 'scalar_from_bytes_mod_order'),  // hazmat.rs -> scalar.rs
          createLink('other_func', 'hazmat_from_bytes'),  // other.rs -> hazmat.rs
          createLink('another_func', 'other_func'),  // another.rs -> other.rs
        ],
        metadata: { total_nodes: 4, total_edges: 3, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      // Filter to only show hazmat.rs and scalar.rs (no source/sink query)
      const filters = createFilters({
        includeFiles: 'hazmat.rs, scalar.rs',
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      // Should include both nodes from the specified files
      expect(nodeNames).toContain('from_bytes');
      expect(nodeNames).toContain('from_bytes_mod_order');
      // Should NOT include nodes from other files
      expect(nodeNames).not.toContain('other_func');
      expect(nodeNames).not.toContain('another_func');
      // Should have the edge between them
      expect(result.links.length).toBe(1);
      expect(result.links[0].source).toBe('hazmat_from_bytes');
      expect(result.links[0].target).toBe('scalar_from_bytes_mod_order');
    });

    it('supports multiple file patterns with glob wildcards', () => {
      const graph: D3Graph = {
        nodes: [
          createNode({ id: 'a', display_name: 'func_a', file_name: 'hazmat.rs' }),
          createNode({ id: 'b', display_name: 'func_b', file_name: 'scalar.rs' }),
          createNode({ id: 'c', display_name: 'func_c', file_name: 'edwards.rs' }),
          createNode({ id: 'd', display_name: 'func_d', file_name: 'ristretto.rs' }),
        ],
        links: [
          createLink('a', 'b'),
          createLink('c', 'd'),
        ],
        metadata: { total_nodes: 4, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      // Use glob pattern to match files ending with .rs that start with 'h' or 's'
      const filters = createFilters({
        includeFiles: 'h*.rs, s*.rs',
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      expect(nodeNames).toContain('func_a');  // hazmat.rs
      expect(nodeNames).toContain('func_b');  // scalar.rs
      expect(nodeNames).not.toContain('func_c');  // edwards.rs - no edge within included files
      expect(nodeNames).not.toContain('func_d');  // ristretto.rs - no edge within included files
    });

    it('clicking nodes does NOT apply depth filtering when includeFiles is active', () => {
      // This tests that the Include Files filter behaves like Source/Sink:
      // clicking a node should NOT narrow the graph to just that node's neighborhood
      const graph: D3Graph = {
        nodes: [
          createNode({ id: 'a', display_name: 'func_a', file_name: 'hazmat.rs' }),
          createNode({ id: 'b', display_name: 'func_b', file_name: 'hazmat.rs' }),
          createNode({ id: 'c', display_name: 'func_c', file_name: 'hazmat.rs' }),
          createNode({ id: 'd', display_name: 'func_d', file_name: 'other.rs' }),
        ],
        links: [
          createLink('a', 'b'),  // a -> b
          createLink('b', 'c'),  // b -> c (depth 2 from a)
          createLink('d', 'a'),  // d -> a (from other file)
        ],
        metadata: { total_nodes: 4, total_edges: 3, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      // Include Files is set, and we "click" on node 'a' (simulate by adding to selectedNodes)
      const filters = createFilters({
        includeFiles: 'hazmat.rs',
        selectedNodes: new Set(['a']),  // Simulating a click on node 'a'
        maxDepth: 1,  // Would normally limit to depth 1 from clicked node
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      // Even though maxDepth=1 and we clicked 'a', all hazmat.rs nodes should remain
      // because Include Files is active (clicking shouldn't narrow the view)
      expect(nodeNames).toContain('func_a');
      expect(nodeNames).toContain('func_b');
      expect(nodeNames).toContain('func_c');  // Would be excluded if depth filtering was applied (depth 2)
      expect(nodeNames).not.toContain('func_d');  // Excluded because it's in other.rs
      // Should have edges a->b and b->c (within hazmat.rs)
      expect(result.links.length).toBe(2);
    });
  });

  describe('Path pattern disambiguation', () => {
    it('pathPatternToRegex matches paths ending with pattern', () => {
      const regex = pathPatternToRegex('ifma/edwards.rs');
      
      // Should match paths ending with /ifma/edwards.rs
      expect(regex.test('curve25519-dalek/src/backend/vector/ifma/edwards.rs')).toBe(true);
      expect(regex.test('some/other/ifma/edwards.rs')).toBe(true);
      
      // Should match exact path
      expect(regex.test('ifma/edwards.rs')).toBe(true);
      
      // Should NOT match different paths
      expect(regex.test('curve25519-dalek/src/edwards.rs')).toBe(false);
      expect(regex.test('avx2/edwards.rs')).toBe(false);
    });

    it('pathPatternToRegex handles ** glob patterns', () => {
      const regex = pathPatternToRegex('**/backend/**/edwards.rs');
      
      // Should match any path with backend and something after it before edwards.rs
      expect(regex.test('curve25519-dalek/src/backend/vector/ifma/edwards.rs')).toBe(true);
      expect(regex.test('curve25519-dalek/src/backend/vector/avx2/edwards.rs')).toBe(true);
      expect(regex.test('some/backend/x/edwards.rs')).toBe(true);
      
      // Should NOT match paths without backend
      expect(regex.test('curve25519-dalek/src/edwards.rs')).toBe(false);
      
      // Test simpler ** pattern
      const regex2 = pathPatternToRegex('**/edwards.rs');
      expect(regex2.test('curve25519-dalek/src/edwards.rs')).toBe(true);
      expect(regex2.test('backend/vector/ifma/edwards.rs')).toBe(true);
      expect(regex2.test('edwards.rs')).toBe(true);
    });

    it('path patterns in includeFiles disambiguate duplicate filenames', () => {
      // Simulates the real scenario: multiple edwards.rs files in different paths
      // Each file has internal edges so nodes aren't isolated when filtered
      const graph: D3Graph = {
        nodes: [
          createNode({ 
            id: 'src_edwards_func1', 
            display_name: 'decompress', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/edwards.rs'
          }),
          createNode({ 
            id: 'src_edwards_func2', 
            display_name: 'compress', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/edwards.rs'
          }),
          createNode({ 
            id: 'ifma_edwards_func1', 
            display_name: 'mul_by_pow_2', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/backend/vector/ifma/edwards.rs'
          }),
          createNode({ 
            id: 'ifma_edwards_func2', 
            display_name: 'double', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/backend/vector/ifma/edwards.rs'
          }),
          createNode({ 
            id: 'avx2_edwards_func', 
            display_name: 'neg', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/backend/vector/avx2/edwards.rs'
          }),
        ],
        links: [
          // Internal edges within each file
          createLink('src_edwards_func1', 'src_edwards_func2'),
          createLink('ifma_edwards_func1', 'ifma_edwards_func2'),
          // Cross-file edges
          createLink('ifma_edwards_func1', 'src_edwards_func1'),
          createLink('avx2_edwards_func', 'src_edwards_func1'),
        ],
        metadata: { total_nodes: 5, total_edges: 4, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      // Filter to only the ifma edwards.rs using path pattern
      const filters = createFilters({
        includeFiles: 'ifma/edwards.rs',
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      // Should only include the ifma edwards.rs functions (they have an edge between them)
      expect(nodeNames).toContain('mul_by_pow_2');
      expect(nodeNames).toContain('double');
      // Should NOT include other edwards.rs files
      expect(nodeNames).not.toContain('decompress');
      expect(nodeNames).not.toContain('compress');
      expect(nodeNames).not.toContain('neg');
      // Should have 1 edge (the internal ifma edge)
      expect(result.links.length).toBe(1);
    });

    it('simple filename pattern still matches all files with that name', () => {
      const graph: D3Graph = {
        nodes: [
          createNode({ 
            id: 'src_edwards_func', 
            display_name: 'decompress', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/edwards.rs'
          }),
          createNode({ 
            id: 'ifma_edwards_func', 
            display_name: 'mul_by_pow_2', 
            file_name: 'edwards.rs',
            relative_path: 'curve25519-dalek/src/backend/vector/ifma/edwards.rs'
          }),
          createNode({ 
            id: 'scalar_func', 
            display_name: 'from_bytes', 
            file_name: 'scalar.rs',
            relative_path: 'curve25519-dalek/src/scalar.rs'
          }),
        ],
        links: [
          createLink('src_edwards_func', 'ifma_edwards_func'),
          createLink('src_edwards_func', 'scalar_func'),
        ],
        metadata: { total_nodes: 3, total_edges: 2, project_root: '/test', generated_at: '2024-01-01' },
      };
      
      // Filter with just filename (no path) - should match ALL edwards.rs files
      const filters = createFilters({
        includeFiles: 'edwards.rs',
      });
      
      const result = applyFilters(graph, filters);
      const nodeNames = result.nodes.map(n => n.display_name);
      
      // Should include BOTH edwards.rs functions
      expect(nodeNames).toContain('decompress');
      expect(nodeNames).toContain('mul_by_pow_2');
      // Should NOT include scalar.rs
      expect(nodeNames).not.toContain('from_bytes');
    });
  });
});

