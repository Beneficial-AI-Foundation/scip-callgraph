/**
 * Composable query pipeline for scip-callgraph.
 *
 * This module implements a clean separation between:
 *   - Query AST (what traversal to perform)
 *   - Graph operators (pure functions that transform graphs)
 *   - Compiler (FilterOptions -> GraphQuery)
 *   - Executor (7-step pipeline producing a D3Graph)
 *
 * See web/QUERY_PIPELINE.md for the full architecture documentation.
 */

import {
  D3Graph, D3Node, D3Link, FilterOptions, ProjectLanguage,
  getKindSetsForLanguage,
} from './types';
import {
  globToRegex, asSubstringGlob, matchesQuery,
  SelectedNodeOptions,
} from './filters';

// ============================================================================
// 1. Types
// ============================================================================

export type NodeMatcher =
  | { kind: 'pattern'; query: string }
  | { kind: 'crate'; pattern: string }
  | { kind: 'nodeIds'; ids: Set<string> };

export type GraphQuery =
  | { type: 'callees'; from: NodeMatcher; maxDepth: number | null }
  | { type: 'callers'; to: NodeMatcher; maxDepth: number | null }
  | { type: 'neighborhood'; center: NodeMatcher; maxDepth: number | null }
  | { type: 'paths'; from: NodeMatcher; to: NodeMatcher }
  | { type: 'crateBoundary'; sourceCrate: string; targetCrate: string }
  | { type: 'depthFromSelected'; selectedNodes: Set<string>; maxDepth: number }
  | { type: 'noTraversal' };

export interface TraversalResult {
  nodeIds: Set<string>;
  calleeDepths?: Map<string, number>;
  callerDepths?: Map<string, number>;
  boundaryLinkPairs?: Set<string>;
}

interface IncludeFilePattern {
  regex: RegExp;
  isPathPattern: boolean;
  original: string;
}

export interface TraversalPredicates {
  kindFilter: (node: D3Node) => boolean;
  excludeNamePatterns: RegExp[];
  excludePathPatterns: RegExp[];
  includeFilePatterns: IncludeFilePattern[];
  hiddenNodes: Set<string>;
  excludeBuildArtifacts: boolean;
}

export interface DisplayPredicates {
  showLibsignal: boolean;
  showNonLibsignal: boolean;
}

export interface FocusConfig {
  focusNodeIds: Set<string>;
}

export interface LinkTypeFilter {
  showInnerCalls: boolean;
  showPreconditionCalls: boolean;
  showPostconditionCalls: boolean;
}

/** Full compiled result returned by compileQuery. */
export interface CompiledQuery {
  query: GraphQuery;
  traversalPredicates: TraversalPredicates;
  displayPredicates: DisplayPredicates;
  focusConfig: FocusConfig;
  linkTypeFilter: LinkTypeFilter;
  resultFilePatterns: IncludeFilePattern[];
}

// ============================================================================
// 2. Internal helpers (pattern parsing -- extracted from filters.ts)
// ============================================================================

function pathPatternToRegex(pattern: string): RegExp {
  let regexStr = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  regexStr = regexStr.replace(/\*\*/g, '<<<GLOBSTAR>>>');
  regexStr = regexStr.replace(/\*/g, '[^/]*');
  regexStr = regexStr.replace(/\?/g, '[^/]');
  regexStr = regexStr.replace(/<<<GLOBSTAR>>>/g, '.*');

  if (pattern.startsWith('**/')) {
    regexStr = regexStr.replace(/^\.\*\//, '');
    regexStr = '(^|.*/)' + regexStr + '$';
  } else if (!pattern.startsWith('**')) {
    regexStr = '(^|/)' + regexStr + '$';
  } else {
    regexStr = regexStr + '$';
  }
  return new RegExp(regexStr, 'i');
}

function parseExcludePatterns(patternsStr: string): RegExp[] {
  if (!patternsStr.trim()) return [];
  return patternsStr
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0)
    .map(p => globToRegex(p));
}

function parseIncludeFilePatterns(patternsStr: string): IncludeFilePattern[] {
  if (!patternsStr.trim()) return [];
  return patternsStr
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0)
    .map(p => ({
      regex: p.includes('/') ? pathPatternToRegex(p) : globToRegex(p),
      isPathPattern: p.includes('/'),
      original: p,
    }));
}

function nodeIsFromBuildArtifact(node: D3Node): boolean {
  const p = node.relative_path || '';
  return p.startsWith('target/') || p.startsWith('build/')
    || p.includes('/target/') || p.includes('/build/');
}

function nodeMatchesExcludeNamePattern(node: D3Node, patterns: RegExp[]): boolean {
  if (patterns.length === 0) return false;
  return patterns.some(re => re.test(node.display_name));
}

function nodeMatchesExcludePathPattern(node: D3Node, patterns: RegExp[]): boolean {
  if (patterns.length === 0) return false;
  return patterns.some(re => re.test(node.id));
}

function nodeMatchesIncludeFilePattern(node: D3Node, patterns: IncludeFilePattern[]): boolean {
  if (patterns.length === 0) return true;
  const fileName = node.file_name || '';
  const relativePath = node.relative_path || '';
  return patterns.some(p =>
    p.isPathPattern ? p.regex.test(relativePath) : p.regex.test(fileName),
  );
}

function isCrateQuery(query: string): boolean {
  return query.startsWith('crate:');
}

function getLinkId(link: D3Link): { sourceId: string; targetId: string } {
  const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
  const targetId = typeof link.target === 'string' ? link.target : link.target.id;
  return { sourceId, targetId };
}

// ============================================================================
// 3. Graph Operators
// ============================================================================

/** Build a traversable subgraph by keeping only nodes that pass all traversal predicates. */
export function selectNodes(
  graph: D3Graph,
  predicates: TraversalPredicates,
): D3Graph {
  const allowed = new Set<string>();
  for (const node of graph.nodes) {
    if (!predicates.kindFilter(node)) continue;
    if (nodeMatchesExcludeNamePattern(node, predicates.excludeNamePatterns)) continue;
    if (nodeMatchesExcludePathPattern(node, predicates.excludePathPatterns)) continue;
    if (!nodeMatchesIncludeFilePattern(node, predicates.includeFilePatterns)) continue;
    if (predicates.excludeBuildArtifacts && nodeIsFromBuildArtifact(node)) continue;
    if (predicates.hiddenNodes.has(node.id)) continue;
    allowed.add(node.id);
  }

  return {
    nodes: graph.nodes.filter(n => allowed.has(n.id)),
    links: graph.links.filter(l => {
      const { sourceId, targetId } = getLinkId(l);
      return allowed.has(sourceId) && allowed.has(targetId);
    }),
    metadata: graph.metadata,
  };
}

/** BFS forward (caller -> callees) from startIds, respecting maxDepth. */
export function traverseForward(
  graph: D3Graph,
  startIds: Set<string>,
  maxDepth: number | null,
): TraversalResult {
  const adj = new Map<string, Set<string>>();
  for (const link of graph.links) {
    const { sourceId, targetId } = getLinkId(link);
    if (!adj.has(sourceId)) adj.set(sourceId, new Set());
    adj.get(sourceId)!.add(targetId);
  }

  const calleeDepths = new Map<string, number>();
  const queue: Array<{ id: string; depth: number }> = [];

  for (const id of startIds) {
    queue.push({ id, depth: 0 });
  }

  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    if (calleeDepths.has(id)) continue;
    calleeDepths.set(id, depth);
    if (maxDepth !== null && depth >= maxDepth) continue;
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    for (const nId of neighbors) {
      if (!calleeDepths.has(nId)) {
        queue.push({ id: nId, depth: depth + 1 });
      }
    }
  }

  return {
    nodeIds: new Set(calleeDepths.keys()),
    calleeDepths,
  };
}

/** BFS backward (callees -> callers) from startIds, respecting maxDepth. */
export function traverseBackward(
  graph: D3Graph,
  startIds: Set<string>,
  maxDepth: number | null,
): TraversalResult {
  const adj = new Map<string, Set<string>>();
  for (const link of graph.links) {
    const { sourceId, targetId } = getLinkId(link);
    if (!adj.has(targetId)) adj.set(targetId, new Set());
    adj.get(targetId)!.add(sourceId);
  }

  const callerDepths = new Map<string, number>();
  const queue: Array<{ id: string; depth: number }> = [];

  for (const id of startIds) {
    queue.push({ id, depth: 0 });
  }

  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    if (callerDepths.has(id)) continue;
    callerDepths.set(id, depth);
    if (maxDepth !== null && depth >= maxDepth) continue;
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    for (const nId of neighbors) {
      if (!callerDepths.has(nId)) {
        queue.push({ id: nId, depth: depth + 1 });
      }
    }
  }

  return {
    nodeIds: new Set(callerDepths.keys()),
    callerDepths,
  };
}

/** BFS in both directions from center nodes (used by click-based depth expansion). */
export function traverseBidirectional(
  graph: D3Graph,
  centerIds: Set<string>,
  maxDepth: number,
): TraversalResult {
  const adj = new Map<string, Set<string>>();
  for (const link of graph.links) {
    const { sourceId, targetId } = getLinkId(link);
    if (!adj.has(sourceId)) adj.set(sourceId, new Set());
    if (!adj.has(targetId)) adj.set(targetId, new Set());
    adj.get(sourceId)!.add(targetId);
    adj.get(targetId)!.add(sourceId);
  }

  const visited = new Set<string>();
  const queue: Array<{ id: string; depth: number }> = [];

  for (const id of centerIds) {
    queue.push({ id, depth: 0 });
    visited.add(id);
  }

  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    if (depth >= maxDepth) continue;
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    for (const nId of neighbors) {
      if (!visited.has(nId)) {
        visited.add(nId);
        queue.push({ id: nId, depth: depth + 1 });
      }
    }
  }

  return { nodeIds: visited };
}

/** DFS path finding from sourceIds to sinkIds, returns all nodes on any valid path. */
export function findPaths(
  graph: D3Graph,
  sourceIds: Set<string>,
  sinkIds: Set<string>,
): TraversalResult {
  const forwardAdj = new Map<string, Set<string>>();
  for (const link of graph.links) {
    const { sourceId, targetId } = getLinkId(link);
    if (!forwardAdj.has(sourceId)) forwardAdj.set(sourceId, new Set());
    forwardAdj.get(sourceId)!.add(targetId);
  }

  const nodesOnPaths = new Set<string>();

  for (const startId of sourceIds) {
    const currentPath: string[] = [];
    const visited = new Set<string>();

    function dfs(nodeId: string): boolean {
      if (sinkIds.has(nodeId)) {
        nodesOnPaths.add(nodeId);
        for (const p of currentPath) nodesOnPaths.add(p);
        return true;
      }
      if (visited.has(nodeId)) return false;
      visited.add(nodeId);
      currentPath.push(nodeId);

      let found = false;
      const neighbors = forwardAdj.get(nodeId);
      if (neighbors) {
        for (const nId of neighbors) {
          if (dfs(nId)) found = true;
        }
      }

      currentPath.pop();
      visited.delete(nodeId);
      if (found) nodesOnPaths.add(nodeId);
      return found;
    }

    dfs(startId);
  }

  return { nodeIds: nodesOnPaths };
}

/** Find cross-crate boundary edges from sourceCrate to targetCrate. */
export function crateBoundary(
  graph: D3Graph,
  sourceCrate: string,
  targetCrate: string,
): TraversalResult {
  const nodeMap = new Map<string, D3Node>();
  for (const n of graph.nodes) nodeMap.set(n.id, n);

  const srcRegex = globToRegex(asSubstringGlob(sourceCrate));
  const tgtRegex = globToRegex(asSubstringGlob(targetCrate));
  const nodeIds = new Set<string>();
  const boundaryLinkPairs = new Set<string>();

  for (const link of graph.links) {
    const { sourceId, targetId } = getLinkId(link);
    const sNode = nodeMap.get(sourceId);
    const tNode = nodeMap.get(targetId);
    if (!sNode || !tNode) continue;

    if (srcRegex.test(sNode.crate_name) && tgtRegex.test(tNode.crate_name)) {
      nodeIds.add(sourceId);
      nodeIds.add(targetId);
      boundaryLinkPairs.add(`${sourceId}\0${targetId}`);
    }
  }

  return { nodeIds, boundaryLinkPairs };
}

/** Keep only links whose type passes the filter. */
export function filterLinksByType(
  links: D3Link[],
  filter: LinkTypeFilter,
): D3Link[] {
  return links.filter(link => {
    const t = link.type || 'inner';
    if (t === 'calls' || t === 'inner') return filter.showInnerCalls;
    if (t === 'precondition') return filter.showPreconditionCalls;
    if (t === 'postcondition') return filter.showPostconditionCalls;
    return true;
  });
}

/** Keep only links that follow the BFS depth tree (no shortcut edges). */
export function depthFilterLinks(
  links: D3Link[],
  calleeDepths: Map<string, number> | undefined,
  callerDepths: Map<string, number> | undefined,
): D3Link[] {
  return links.filter(link => {
    const { sourceId, targetId } = getLinkId(link);

    if (calleeDepths && calleeDepths.has(sourceId) && calleeDepths.has(targetId)) {
      if (calleeDepths.get(sourceId)! + 1 === calleeDepths.get(targetId)!) return true;
    }
    if (callerDepths && callerDepths.has(sourceId) && callerDepths.has(targetId)) {
      if (callerDepths.get(targetId)! + 1 === callerDepths.get(sourceId)!) return true;
    }
    return false;
  });
}

/** Remove nodes that have no edges, unless they're in the keepSet. */
export function removeIsolated(
  nodes: D3Node[],
  links: D3Link[],
  keepSet?: Set<string>,
): D3Node[] {
  const connected = new Set<string>();
  for (const link of links) {
    const { sourceId, targetId } = getLinkId(link);
    connected.add(sourceId);
    connected.add(targetId);
  }
  return nodes.filter(n =>
    connected.has(n.id) || (keepSet && keepSet.size > 0 && keepSet.has(n.id)),
  );
}

// ============================================================================
// 4. Resolver
// ============================================================================

/**
 * Resolve a NodeMatcher to concrete node IDs.
 * Patterns are matched against fullGraph; results are filtered to traversableIds.
 * The exactOverride (VS Code selectedNodeId) bypasses pattern matching.
 */
export function resolveNodeMatcher(
  matcher: NodeMatcher,
  fullGraph: D3Graph,
  traversableIds: Set<string>,
  exactOverride?: string | null,
): Set<string> {
  const matched = new Set<string>();

  if (exactOverride) {
    const node = fullGraph.nodes.find(n => n.id === exactOverride);
    if (node) {
      matched.add(node.id);
    } else {
      resolveWithoutOverride(matcher, fullGraph, matched);
    }
  } else {
    resolveWithoutOverride(matcher, fullGraph, matched);
  }

  // Filter to traversable set
  const result = new Set<string>();
  for (const id of matched) {
    if (traversableIds.has(id)) result.add(id);
  }
  return result;
}

function resolveWithoutOverride(
  matcher: NodeMatcher,
  fullGraph: D3Graph,
  out: Set<string>,
): void {
  switch (matcher.kind) {
    case 'pattern':
      for (const node of fullGraph.nodes) {
        if (matchesQuery(node, matcher.query)) out.add(node.id);
      }
      break;
    case 'crate':
      for (const node of fullGraph.nodes) {
        if (matchesQuery(node, `crate:${matcher.pattern}`)) out.add(node.id);
      }
      break;
    case 'nodeIds':
      for (const id of matcher.ids) out.add(id);
      break;
  }
}

// ============================================================================
// 5. Compiler
// ============================================================================

function buildMatcher(queryStr: string): NodeMatcher {
  const q = queryStr.trim().toLowerCase();
  if (isCrateQuery(q)) {
    return { kind: 'crate', pattern: q.slice(6) };
  }
  return { kind: 'pattern', query: q };
}

/**
 * Compile FilterOptions (+ optional nodeOptions) into a CompiledQuery.
 * Pure function -- no graph access.
 */
export function compileQuery(
  filters: FilterOptions,
  projectLanguage: ProjectLanguage = 'unknown',
): CompiledQuery {
  const { proofKinds, specKinds } = getKindSetsForLanguage(projectLanguage);

  const parsedFilePatterns = parseIncludeFilePatterns(filters.includeFiles);

  // -- Query compilation (dispatch table) --
  const sourceQuery = filters.sourceQuery.trim().toLowerCase();
  const sinkQuery = filters.sinkQuery.trim().toLowerCase();
  const hasSource = sourceQuery !== '';
  const hasSink = sinkQuery !== '';
  const isSame = hasSource && hasSink && sourceQuery === sinkQuery;
  const bothCrate = hasSource && hasSink && isCrateQuery(sourceQuery) && isCrateQuery(sinkQuery);
  const hasIncludeFiles = filters.includeFiles.trim() !== '';
  const hasDirectionalQuery = hasSource || hasSink;

  let query: GraphQuery;

  if (hasSource && hasSink) {
    if (isSame) {
      query = { type: 'neighborhood', center: buildMatcher(sourceQuery), maxDepth: filters.maxDepth };
    } else if (bothCrate) {
      query = {
        type: 'crateBoundary',
        sourceCrate: sourceQuery.slice(6),
        targetCrate: sinkQuery.slice(6),
      };
    } else {
      query = { type: 'paths', from: buildMatcher(sourceQuery), to: buildMatcher(sinkQuery) };
    }
  } else if (hasSource) {
    query = { type: 'callees', from: buildMatcher(sourceQuery), maxDepth: filters.maxDepth };
  } else if (hasSink) {
    query = { type: 'callers', to: buildMatcher(sinkQuery), maxDepth: filters.maxDepth };
  } else if (
    filters.selectedNodes.size > 0
    && filters.maxDepth !== null
    && !hasIncludeFiles
  ) {
    query = { type: 'depthFromSelected', selectedNodes: filters.selectedNodes, maxDepth: filters.maxDepth };
  } else {
    query = { type: 'noTraversal' };
  }

  // When there's a directional query (source/sink), the file filter should be
  // a post-traversal result filter so the BFS can traverse through the full
  // graph and results are narrowed afterwards. For noTraversal / depthFromSelected
  // the file filter stays as a traversal predicate (restricts which nodes appear).
  const useFileAsResultFilter = hasDirectionalQuery && hasIncludeFiles;

  // -- Traversal predicates --
  const traversalPredicates: TraversalPredicates = {
    kindFilter: (node: D3Node) => {
      const kind = node.kind || 'exec';
      if (proofKinds.has(kind) && !filters.showProofFunctions) return false;
      if (specKinds.has(kind) && !filters.showSpecFunctions) return false;
      if (!proofKinds.has(kind) && !specKinds.has(kind) && !filters.showExecFunctions) return false;
      return true;
    },
    excludeNamePatterns: parseExcludePatterns(filters.excludeNamePatterns),
    excludePathPatterns: parseExcludePatterns(filters.excludePathPatterns),
    includeFilePatterns: useFileAsResultFilter ? [] : parsedFilePatterns,
    hiddenNodes: filters.hiddenNodes,
    excludeBuildArtifacts: true,
  };

  const displayPredicates: DisplayPredicates = {
    showLibsignal: filters.showLibsignal,
    showNonLibsignal: filters.showNonLibsignal,
  };

  const focusConfig: FocusConfig = {
    focusNodeIds: filters.focusNodeIds,
  };

  const linkTypeFilter: LinkTypeFilter = {
    showInnerCalls: filters.showInnerCalls,
    showPreconditionCalls: filters.showPreconditionCalls,
    showPostconditionCalls: filters.showPostconditionCalls,
  };

  return {
    query, traversalPredicates, displayPredicates, focusConfig, linkTypeFilter,
    resultFilePatterns: useFileAsResultFilter ? parsedFilePatterns : [],
  };
}

// ============================================================================
// 6. Executor
// ============================================================================

/**
 * Execute a compiled query against a full graph, producing a filtered D3Graph.
 *
 * The 7-step pipeline:
 *   1. selectNodes  (traversal predicates -> traversable subgraph)
 *   2. resolve matchers  (full graph, filter to traversable)
 *   3. dispatch traversal
 *   4. assemble result nodes
 *   5. apply display predicates
 *   6. filter links
 *   7. removeIsolated + build nodeDepths
 */
export function executeQuery(
  compiled: CompiledQuery,
  fullGraph: D3Graph,
  nodeOptions?: SelectedNodeOptions,
): D3Graph {
  const { query, traversalPredicates, displayPredicates, focusConfig, linkTypeFilter, resultFilePatterns } = compiled;
  const exactOverride = nodeOptions?.selectedNodeId;

  // -- Step 1: build traversable subgraph --
  const traversableGraph = selectNodes(fullGraph, traversalPredicates);
  const traversableIds = new Set(traversableGraph.nodes.map(n => n.id));

  // -- Step 2: resolve matchers --
  let sourceMatchIds: Set<string> | undefined;
  let sinkMatchIds: Set<string> | undefined;

  if (query.type === 'callees') {
    sourceMatchIds = resolveNodeMatcher(query.from, fullGraph, traversableIds, exactOverride);
  } else if (query.type === 'callers') {
    sinkMatchIds = resolveNodeMatcher(query.to, fullGraph, traversableIds, exactOverride);
  } else if (query.type === 'neighborhood') {
    sourceMatchIds = resolveNodeMatcher(query.center, fullGraph, traversableIds, exactOverride);
    sinkMatchIds = sourceMatchIds; // same set
  } else if (query.type === 'paths') {
    sourceMatchIds = resolveNodeMatcher(query.from, fullGraph, traversableIds, exactOverride);
    sinkMatchIds = resolveNodeMatcher(query.to, fullGraph, traversableIds, exactOverride);
  }

  // If a query was entered but matched nothing, return empty
  const queryEntered = query.type !== 'noTraversal' && query.type !== 'depthFromSelected';
  if (queryEntered) {
    const hasMatches =
      (sourceMatchIds && sourceMatchIds.size > 0) ||
      (sinkMatchIds && sinkMatchIds.size > 0) ||
      query.type === 'crateBoundary';
    if (!hasMatches) {
      return {
        nodes: [],
        links: [],
        metadata: { ...fullGraph.metadata, total_nodes: 0, total_edges: 0 },
      };
    }
  }

  // -- Step 3: dispatch traversal --
  let traversalResult: TraversalResult;
  let useDepthFilter = false;

  switch (query.type) {
    case 'callees': {
      useDepthFilter = true;
      const merged = mergeTraversals(
        traversableGraph, sourceMatchIds!, query.maxDepth, 'forward',
      );
      traversalResult = merged;
      break;
    }
    case 'callers': {
      useDepthFilter = true;
      const merged = mergeTraversals(
        traversableGraph, sinkMatchIds!, query.maxDepth, 'backward',
      );
      traversalResult = merged;
      break;
    }
    case 'neighborhood': {
      useDepthFilter = true;
      // Run both forward and backward from the center, then merge
      const fwdMerged = mergeTraversals(
        traversableGraph, sourceMatchIds!, query.maxDepth, 'forward',
      );
      const bwdMerged = mergeTraversals(
        traversableGraph, sourceMatchIds!, query.maxDepth, 'backward',
      );
      const allIds = new Set<string>();
      for (const id of fwdMerged.nodeIds) allIds.add(id);
      for (const id of bwdMerged.nodeIds) allIds.add(id);

      traversalResult = {
        nodeIds: allIds,
        calleeDepths: fwdMerged.calleeDepths,
        callerDepths: bwdMerged.callerDepths,
      };
      break;
    }
    case 'paths': {
      traversalResult = findPaths(traversableGraph, sourceMatchIds!, sinkMatchIds!);
      break;
    }
    case 'crateBoundary': {
      traversalResult = crateBoundary(traversableGraph, query.sourceCrate, query.targetCrate);
      break;
    }
    case 'depthFromSelected': {
      traversalResult = traverseBidirectional(
        traversableGraph, query.selectedNodes, query.maxDepth,
      );
      break;
    }
    case 'noTraversal': {
      let nodeIds: Set<string>;
      if (focusConfig.focusNodeIds.size > 0) {
        nodeIds = new Set(
          traversableGraph.nodes
            .filter(n => focusConfig.focusNodeIds.has(n.id))
            .map(n => n.id),
        );
      } else {
        nodeIds = traversableIds;
      }
      traversalResult = { nodeIds };
      break;
    }
  }

  // -- Step 4: assemble result nodes --
  let resultNodes = fullGraph.nodes.filter(n => traversalResult.nodeIds.has(n.id));

  // -- Step 5: apply display predicates --
  if (!displayPredicates.showLibsignal || !displayPredicates.showNonLibsignal) {
    resultNodes = resultNodes.filter(n => {
      if (n.is_libsignal && !displayPredicates.showLibsignal) return false;
      if (!n.is_libsignal && !displayPredicates.showNonLibsignal) return false;
      return true;
    });
  }

  // Re-apply kind filter on result nodes (for depthFromSelected, the traversal
  // used the traversableGraph which already has kind-filtered nodes, but we
  // still need kind filtering for the no-traversal case that also post-filters)
  if (query.type !== 'noTraversal') {
    resultNodes = resultNodes.filter(n => traversalPredicates.kindFilter(n));
  }

  // Re-apply hidden node filter (already in traversal predicates, but for
  // assembling from fullGraph we need to recheck)
  if (traversalPredicates.hiddenNodes.size > 0) {
    resultNodes = resultNodes.filter(n => !traversalPredicates.hiddenNodes.has(n.id));
  }

  // -- Step 5b: apply result file filter --
  // When source/sink queries are combined with includeFiles, the file filter
  // is applied here (post-traversal) instead of in selectNodes. This allows
  // the BFS to traverse through the full graph while narrowing the displayed
  // results to the requested files. Seed nodes (source/sink matches) are kept
  // regardless so the user sees the connection context.
  if (resultFilePatterns.length > 0) {
    const seedIds = new Set<string>();
    if (sourceMatchIds) for (const id of sourceMatchIds) seedIds.add(id);
    if (sinkMatchIds) for (const id of sinkMatchIds) seedIds.add(id);
    resultNodes = resultNodes.filter(n =>
      seedIds.has(n.id) || nodeMatchesIncludeFilePattern(n, resultFilePatterns),
    );
  }

  // -- Step 6: filter links --
  const validNodeIds = new Set(resultNodes.map(n => n.id));
  let resultLinks = fullGraph.links.filter(l => {
    const { sourceId, targetId } = getLinkId(l);
    if (!validNodeIds.has(sourceId) || !validNodeIds.has(targetId)) return false;
    if (traversalResult.boundaryLinkPairs) {
      return traversalResult.boundaryLinkPairs.has(`${sourceId}\0${targetId}`);
    }
    return true;
  });

  if (useDepthFilter && query.type !== 'depthFromSelected') {
    const maxDepth = (query as any).maxDepth;
    if (maxDepth !== null) {
      resultLinks = depthFilterLinks(
        resultLinks,
        traversalResult.calleeDepths,
        traversalResult.callerDepths,
      );
    }
  }

  resultLinks = filterLinksByType(resultLinks, linkTypeFilter);

  // -- Step 7: cleanup --
  const keepSet = focusConfig.focusNodeIds.size > 0 ? focusConfig.focusNodeIds : undefined;
  resultNodes = removeIsolated(resultNodes, resultLinks, keepSet);

  // Build nodeDepths
  let nodeDepths: Map<string, number> | undefined;
  if (query.type === 'callers' && traversalResult.callerDepths) {
    nodeDepths = new Map(traversalResult.callerDepths);
  } else if (query.type === 'callees' && traversalResult.calleeDepths) {
    nodeDepths = new Map(traversalResult.calleeDepths);
  } else if (query.type === 'neighborhood') {
    nodeDepths = new Map<string, number>();
    if (traversalResult.callerDepths) {
      for (const [id, d] of traversalResult.callerDepths) nodeDepths.set(id, d);
    }
    if (traversalResult.calleeDepths) {
      for (const [id, d] of traversalResult.calleeDepths) {
        if (!nodeDepths.has(id) || d < nodeDepths.get(id)!) nodeDepths.set(id, d);
      }
    }
  }

  return {
    nodes: resultNodes.map(n => ({ ...n })),
    links: resultLinks.map(l => ({ source: l.source, target: l.target, type: l.type })),
    metadata: {
      ...fullGraph.metadata,
      total_nodes: resultNodes.length,
      total_edges: resultLinks.length,
    },
    nodeDepths,
  };
}

/**
 * Run traversal from multiple start nodes and merge results, keeping minimum
 * depth for each node.
 */
function mergeTraversals(
  graph: D3Graph,
  startIds: Set<string>,
  maxDepth: number | null,
  direction: 'forward' | 'backward',
): TraversalResult {
  const allNodeIds = new Set<string>();
  const mergedDepths = new Map<string, number>();

  for (const startId of startIds) {
    const single =
      direction === 'forward'
        ? traverseForward(graph, new Set([startId]), maxDepth)
        : traverseBackward(graph, new Set([startId]), maxDepth);

    for (const id of single.nodeIds) allNodeIds.add(id);

    const depthMap = direction === 'forward' ? single.calleeDepths : single.callerDepths;
    if (depthMap) {
      for (const [id, depth] of depthMap) {
        if (!mergedDepths.has(id) || depth < mergedDepths.get(id)!) {
          mergedDepths.set(id, depth);
        }
      }
    }
  }

  return direction === 'forward'
    ? { nodeIds: allNodeIds, calleeDepths: mergedDepths }
    : { nodeIds: allNodeIds, callerDepths: mergedDepths };
}
