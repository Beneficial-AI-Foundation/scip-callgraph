import { D3Graph, D3Node, D3Link, FilterOptions } from './types';

/**
 * Check if a node should be included based on mode filters
 */
function nodePassesModeFilter(node: D3Node, filters: FilterOptions): boolean {
  const mode = node.mode || 'exec';
  if (mode === 'exec' && !filters.showExecFunctions) return false;
  if (mode === 'proof' && !filters.showProofFunctions) return false;
  if (mode === 'spec' && !filters.showSpecFunctions) return false;
  return true;
}

/**
 * Convert a glob pattern to a regex
 * - * matches any characters (including empty)
 * - ? matches a single character
 * - Other regex special chars are escaped
 * - Pattern is anchored: p_* matches "p_foo" but not "step_1"
 * - Use *pattern* for substring matching
 * 
 * @public Exported for testing
 */
export function globToRegex(pattern: string): RegExp {
  // Escape regex special characters except * and ?
  let regexStr = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  
  // Convert glob wildcards to regex
  regexStr = regexStr.replace(/\*/g, '.*');
  regexStr = regexStr.replace(/\?/g, '.');
  
  // Anchor the pattern (start and end)
  regexStr = '^' + regexStr + '$';
  
  return new RegExp(regexStr, 'i');  // case-insensitive
}

/**
 * Check if a node matches any of the exclude NAME patterns
 * Returns true if node should be EXCLUDED (hidden)
 * Matches against node.display_name (function name)
 */
function nodeMatchesExcludeNamePattern(node: D3Node, excludePatterns: RegExp[]): boolean {
  if (excludePatterns.length === 0) return false;
  return excludePatterns.some(pattern => pattern.test(node.display_name));
}

/**
 * Check if a node matches any of the exclude PATH patterns
 * Returns true if node should be EXCLUDED (hidden)
 * Matches against node.id (full SCIP path)
 */
function nodeMatchesExcludePathPattern(node: D3Node, excludePatterns: RegExp[]): boolean {
  if (excludePatterns.length === 0) return false;
  return excludePatterns.some(pattern => pattern.test(node.id));
}

/**
 * Parse exclude patterns from comma-separated string and convert to regexes
 */
function parseExcludePatterns(patternsStr: string): RegExp[] {
  if (!patternsStr.trim()) return [];
  return patternsStr
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0)
    .map(p => globToRegex(p));
}

/**
 * Parsed include file pattern with metadata about whether it's a path pattern
 */
interface IncludeFilePattern {
  regex: RegExp;
  isPathPattern: boolean;  // true if pattern contains '/' (matches against relative_path)
  original: string;        // original pattern string for debugging
}

/**
 * Convert a path pattern to a regex that matches against relative_path
 * 
 * Unlike globToRegex (which anchors with ^...$), this handles partial paths:
 * - "ifma/edwards.rs" - matches paths ENDING with "/ifma/edwards.rs" or equal to "ifma/edwards.rs"
 * - "** /ifma/edwards.rs" - matches any path ending with "/ifma/edwards.rs"
 * - "curve25519-dalek/ **" - matches paths starting with "curve25519-dalek/"
 * 
 * @public Exported for testing
 */
export function pathPatternToRegex(pattern: string): RegExp {
  // Escape regex special characters except * and ?
  let regexStr = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  
  // Handle ** BEFORE * to avoid double-replacement
  // Use placeholder to protect ** from being affected by * replacement
  regexStr = regexStr.replace(/\*\*/g, '<<<GLOBSTAR>>>');
  // Handle * (matches anything except /)
  regexStr = regexStr.replace(/\*/g, '[^/]*');
  // Handle ?
  regexStr = regexStr.replace(/\?/g, '[^/]');
  // Now replace the placeholder with the actual globstar pattern
  regexStr = regexStr.replace(/<<<GLOBSTAR>>>/g, '.*');
  
  // Handle **/ at the start specially - it means "any path prefix including empty"
  // e.g., "**/edwards.rs" should match both "foo/edwards.rs" and "edwards.rs"
  if (pattern.startsWith('**/')) {
    // The pattern already has .* at the start from the replacement above
    // We need to make it match "start of string OR any prefix ending with /"
    // Current: .*/rest... -> need: (^|.*/)rest...
    // Remove the leading .*/ and add the proper prefix
    regexStr = regexStr.replace(/^\.\*\//, '');
    regexStr = '(^|.*/)' + regexStr + '$';
  } else if (!pattern.startsWith('**')) {
    // Pattern doesn't start with ** - allow matching at end of path
    // e.g., "ifma/edwards.rs" matches "foo/bar/ifma/edwards.rs"
    regexStr = '(^|/)' + regexStr + '$';
  } else {
    // Pattern starts with ** but not **/ (e.g., "**foo") - anchor at end only
    regexStr = regexStr + '$';
  }
  
  return new RegExp(regexStr, 'i');  // case-insensitive
}

/**
 * Parse include file patterns from comma-separated string
 * Returns empty array if no patterns (meaning include all files)
 * 
 * Patterns containing '/' are matched against relative_path (for disambiguation)
 * Patterns without '/' are matched against file_name only
 * 
 * Examples:
 * - "edwards.rs" - matches file_name "edwards.rs" (all edwards.rs files)
 * - "src/edwards.rs" - matches paths ending with "src/edwards.rs"
 * - "ifma/edwards.rs" - matches paths ending with "ifma/edwards.rs"
 * - "curve25519-dalek/ ** /edwards.rs" - matches edwards.rs anywhere under curve25519-dalek
 * - "** /backend/ **" - matches any path containing /backend/
 */
function parseIncludeFilePatterns(patternsStr: string): IncludeFilePattern[] {
  if (!patternsStr.trim()) return [];
  return patternsStr
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0)
    .map(p => ({
      // Use different regex functions based on whether it's a path pattern
      regex: p.includes('/') ? pathPatternToRegex(p) : globToRegex(p),
      isPathPattern: p.includes('/'),
      original: p,
    }));
}

/**
 * Check if a node's file matches any of the include patterns
 * Returns true if node should be INCLUDED
 * If no patterns specified (empty array), all nodes are included
 * 
 * Path patterns (containing '/') match against relative_path
 * Filename patterns match against file_name
 */
function nodeMatchesIncludeFilePattern(node: D3Node, includePatterns: IncludeFilePattern[]): boolean {
  if (includePatterns.length === 0) return true;  // No filter = include all
  
  const fileName = node.file_name || '';
  const relativePath = node.relative_path || '';
  
  return includePatterns.some(pattern => {
    if (pattern.isPathPattern) {
      // Path pattern: match against relative_path
      return pattern.regex.test(relativePath);
    } else {
      // Filename pattern: match against file_name only
      return pattern.regex.test(fileName);
    }
  });
}

/**
 * Check if a node is from a build artifact directory (target/, build/, etc.)
 * Returns true if node should be EXCLUDED
 */
function nodeIsFromBuildArtifact(node: D3Node): boolean {
  const path = node.relative_path || '';
  // Common build artifact directories
  return path.startsWith('target/') || 
         path.startsWith('build/') ||
         path.includes('/target/') ||
         path.includes('/build/');
}

/**
 * Apply filters to the full graph and return a filtered graph
 */
/**
 * Options for exact node selection (used by VS Code integration)
 */
export interface SelectedNodeOptions {
  /** Exact SCIP ID of the selected node (overrides sourceQuery matching) */
  selectedNodeId?: string | null;
}

export function applyFilters(
  fullGraph: D3Graph,
  filters: FilterOptions,
  nodeOptions?: SelectedNodeOptions
): D3Graph {
  let filteredNodes = [...fullGraph.nodes];
  let filteredLinks = [...fullGraph.links];

  const sourceQuery = filters.sourceQuery.trim().toLowerCase();
  const sinkQuery = filters.sinkQuery.trim().toLowerCase();
  
  // Parse exclude patterns (name and path separately)
  const excludeNamePatterns = parseExcludePatterns(filters.excludeNamePatterns);
  const excludePathPatterns = parseExcludePatterns(filters.excludePathPatterns);

  // Parse include file patterns
  const includeFilePatterns = parseIncludeFilePatterns(filters.includeFiles);

  // Pre-filter: Create a set of node IDs that pass all filters
  // This is used to exclude links to/from filtered-out nodes during traversal
  const modeAllowedNodeIds = new Set<string>(
    fullGraph.nodes
      .filter(node => nodePassesModeFilter(node, filters))
      .filter(node => !nodeMatchesExcludeNamePattern(node, excludeNamePatterns))
      .filter(node => !nodeMatchesExcludePathPattern(node, excludePathPatterns))
      .filter(node => nodeMatchesIncludeFilePattern(node, includeFilePatterns))
      .filter(node => !nodeIsFromBuildArtifact(node))  // Exclude build artifacts (target/, build/)
      .filter(node => !filters.hiddenNodes.has(node.id))  // Exclude hidden nodes from traversal
      .map(node => node.id)
  );
  
  // Create a "traversable" graph that respects mode filters
  // This prevents traversal through spec/proof/exec nodes that are filtered out
  const traversableLinks = fullGraph.links.filter(link => {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    return modeAllowedNodeIds.has(sourceId) && modeAllowedNodeIds.has(targetId);
  });
  
  const traversableGraph: D3Graph = {
    nodes: fullGraph.nodes.filter(n => modeAllowedNodeIds.has(n.id)),
    links: traversableLinks,
    metadata: fullGraph.metadata,
  };

  // Find nodes matching source and sink queries
  const sourceMatchIds = new Set<string>();
  const sinkMatchIds = new Set<string>();
  
  // Check if we have an exact node selection (from VS Code)
  const selectedNodeId = nodeOptions?.selectedNodeId;
  
  if (sourceQuery !== '') {
    if (selectedNodeId) {
      // Use exact node ID for source matching
      // This ensures we only match the specific function the user clicked on
      const exactNode = fullGraph.nodes.find(n => n.id === selectedNodeId);
      if (exactNode) {
        sourceMatchIds.add(exactNode.id);
      } else {
        // Fallback to query matching if ID not found
        fullGraph.nodes.forEach(node => {
          if (matchesQuery(node, sourceQuery)) {
            sourceMatchIds.add(node.id);
          }
        });
      }
    } else {
      // Normal query matching
      fullGraph.nodes.forEach(node => {
        if (matchesQuery(node, sourceQuery)) {
          sourceMatchIds.add(node.id);
        }
      });
    }
  }
  
  if (sinkQuery !== '') {
    if (selectedNodeId) {
      // Use exact node ID for sink matching
      const exactNode = fullGraph.nodes.find(n => n.id === selectedNodeId);
      if (exactNode) {
        sinkMatchIds.add(exactNode.id);
      } else {
        // Fallback to query matching
        fullGraph.nodes.forEach(node => {
          if (matchesQuery(node, sinkQuery)) {
            sinkMatchIds.add(node.id);
          }
        });
      }
    } else {
      // Normal query matching
      fullGraph.nodes.forEach(node => {
        if (matchesQuery(node, sinkQuery)) {
          sinkMatchIds.add(node.id);
        }
      });
    }
  }

  // Build the set of nodes to include based on source/sink configuration
  const includedNodeIds = new Set<string>();

  const hasSource = sourceQuery !== '' && sourceMatchIds.size > 0;
  const hasSink = sinkQuery !== '' && sinkMatchIds.size > 0;
  
  // Check if source and sink are the same (for neighborhood mode)
  const isSameQuery = sourceQuery !== '' && sourceQuery === sinkQuery;

  // Track node depths for proper link filtering
  // For callees: depth increases from source (source_depth + 1 = target_depth)
  // For callers: depth increases towards source (target_depth + 1 = source_depth)
  const calleeDepths = new Map<string, number>();  // nodeId -> depth from source
  const callerDepths = new Map<string, number>();  // nodeId -> depth from sink
  let usePathBasedLinkFilter = false;  // Whether to filter links based on depth

  if (hasSource && hasSink) {
    if (isSameQuery) {
      // Same node in both → show full neighborhood (both directions)
      usePathBasedLinkFilter = true;
      
      sourceMatchIds.forEach(id => {
        if (modeAllowedNodeIds.has(id)) {
          includedNodeIds.add(id);
          calleeDepths.set(id, 0);
          callerDepths.set(id, 0);
        }
      });
      
      // Get both callers and callees (using traversableGraph to respect mode filters)
      sourceMatchIds.forEach(nodeId => {
        if (!modeAllowedNodeIds.has(nodeId)) return;
        const callersResult = getCallersRecursive(traversableGraph, nodeId, filters.maxDepth);
        const calleesResult = getCalleesRecursive(traversableGraph, nodeId, filters.maxDepth);
        callersResult.nodeIds.forEach(id => includedNodeIds.add(id));
        calleesResult.nodeIds.forEach(id => includedNodeIds.add(id));
        // Merge depths (keep minimum depth if node appears multiple times)
        callersResult.nodeDepths.forEach((depth, id) => {
          if (!callerDepths.has(id) || depth < callerDepths.get(id)!) {
            callerDepths.set(id, depth);
          }
        });
        calleesResult.nodeDepths.forEach((depth, id) => {
          if (!calleeDepths.has(id) || depth < calleeDepths.get(id)!) {
            calleeDepths.set(id, depth);
          }
        });
      });
    } else {
      // Different source and sink → find paths between them
      // NOTE: We ignore maxDepth here - we want ALL nodes on any path from source to sink
      // Filter source/sink to only include mode-allowed nodes
      const allowedSources = new Set([...sourceMatchIds].filter(id => modeAllowedNodeIds.has(id)));
      const allowedSinks = new Set([...sinkMatchIds].filter(id => modeAllowedNodeIds.has(id)));
      const pathNodes = findPathNodes(traversableGraph, allowedSources, allowedSinks, null);
      pathNodes.forEach(id => includedNodeIds.add(id));
      // Don't use path-based link filter for source→sink paths (show all path edges)
    }
  } else if (hasSource) {
    // Source only → show callees (what source calls)
    usePathBasedLinkFilter = true;
    
    sourceMatchIds.forEach(id => {
      if (modeAllowedNodeIds.has(id)) {
        includedNodeIds.add(id);
        calleeDepths.set(id, 0);
      }
    });
    
    sourceMatchIds.forEach(nodeId => {
      if (!modeAllowedNodeIds.has(nodeId)) return;
      const calleesResult = getCalleesRecursive(traversableGraph, nodeId, filters.maxDepth);
      calleesResult.nodeIds.forEach(id => includedNodeIds.add(id));
      // Merge depths (keep minimum depth if node appears multiple times)
      calleesResult.nodeDepths.forEach((depth, id) => {
        if (!calleeDepths.has(id) || depth < calleeDepths.get(id)!) {
          calleeDepths.set(id, depth);
        }
      });
    });
  } else if (hasSink) {
    // Sink only → show callers (who calls sink)
    usePathBasedLinkFilter = true;
    
    sinkMatchIds.forEach(id => {
      if (modeAllowedNodeIds.has(id)) {
        includedNodeIds.add(id);
        callerDepths.set(id, 0);
      }
    });
    
    sinkMatchIds.forEach(nodeId => {
      if (!modeAllowedNodeIds.has(nodeId)) return;
      const callersResult = getCallersRecursive(traversableGraph, nodeId, filters.maxDepth);
      callersResult.nodeIds.forEach(id => includedNodeIds.add(id));
      // Merge depths (keep minimum depth if node appears multiple times)
      callersResult.nodeDepths.forEach((depth, id) => {
        if (!callerDepths.has(id) || depth < callerDepths.get(id)!) {
          callerDepths.set(id, depth);
        }
      });
    });
  }
  
  // Filter nodes based on source/sink constraints or pre-filters
  const queryEntered = sourceQuery !== '' || sinkQuery !== '';
  const queryMatched = hasSource || hasSink;
  
  if (queryEntered && !queryMatched) {
    // Query was entered but matched nothing - return empty graph
    return {
      nodes: [],
      links: [],
      metadata: {
        ...fullGraph.metadata,
        total_nodes: 0,
        total_edges: 0,
      },
    };
  } else if (queryMatched) {
    // Apply source/sink traversal results
    filteredNodes = filteredNodes.filter(node => includedNodeIds.has(node.id));
  } else {
    // No source/sink query - apply pre-filters (mode, exclude patterns, include files)
    filteredNodes = filteredNodes.filter(node => modeAllowedNodeIds.has(node.id));
  }

  // Filter by libsignal flag
  if (!filters.showLibsignal || !filters.showNonLibsignal) {
    filteredNodes = filteredNodes.filter(node => {
      if (node.is_libsignal && !filters.showLibsignal) return false;
      if (!node.is_libsignal && !filters.showNonLibsignal) return false;
      return true;
    });
  }

  // Filter by function mode (exec/proof/spec)
  if (!filters.showExecFunctions || !filters.showProofFunctions || !filters.showSpecFunctions) {
    filteredNodes = filteredNodes.filter(node => {
      const mode = node.mode || 'exec';  // Default to exec for legacy data
      if (mode === 'exec' && !filters.showExecFunctions) return false;
      if (mode === 'proof' && !filters.showProofFunctions) return false;
      if (mode === 'spec' && !filters.showSpecFunctions) return false;
      return true;
    });
  }

  // Check if Include Files filter is active (non-empty)
  const hasIncludeFiles = filters.includeFiles.trim() !== '';

  // Apply depth filtering from selected nodes (click-based selection)
  // Only apply when there's NO source/sink query AND NO include files filter
  // - clicking shouldn't override query results or include files filter results
  if (filters.maxDepth !== null && filters.selectedNodes.size > 0 && !hasSource && !hasSink && !hasIncludeFiles) {
    // Use traversable graph to respect mode filters during depth traversal
    // When clicking (no query), explore BOTH directions to show full neighborhood
    const nodesAtDepth = computeDepthFromSelected(
      traversableGraph.nodes,
      traversableGraph.links,
      filters.selectedNodes,
      filters.maxDepth,
      true,  // include callees direction
      true   // include callers direction
    );
    
    filteredNodes = traversableGraph.nodes.filter(node => nodesAtDepth.has(node.id));
    
    // Also apply libsignal filter to depth-expanded nodes
    if (!filters.showLibsignal || !filters.showNonLibsignal) {
      filteredNodes = filteredNodes.filter(node => {
        if (node.is_libsignal && !filters.showLibsignal) return false;
        if (!node.is_libsignal && !filters.showNonLibsignal) return false;
        return true;
      });
    }
    
    // Also apply mode filter to depth-expanded nodes
    if (!filters.showExecFunctions || !filters.showProofFunctions || !filters.showSpecFunctions) {
      filteredNodes = filteredNodes.filter(node => {
        const mode = node.mode || 'exec';
        if (mode === 'exec' && !filters.showExecFunctions) return false;
        if (mode === 'proof' && !filters.showProofFunctions) return false;
        if (mode === 'spec' && !filters.showSpecFunctions) return false;
        return true;
      });
    }
  }

  // Filter out hidden nodes
  if (filters.hiddenNodes.size > 0) {
    filteredNodes = filteredNodes.filter(node => !filters.hiddenNodes.has(node.id));
  }

  // Create a set of valid node IDs for efficient lookup
  const validNodeIds = new Set(filteredNodes.map(node => node.id));

  // Filter links to only include those between valid nodes
  filteredLinks = fullGraph.links.filter(link => {
    const sourceId = typeof link.source === 'string' ? link.source : (link.source as any).id;
    const targetId = typeof link.target === 'string' ? link.target : (link.target as any).id;
    return validNodeIds.has(sourceId) && validNodeIds.has(targetId);
  });

  // Apply path-based link filtering when depth limit is active
  // Only show edges that are "on the path" (source_depth + 1 = target_depth for callees,
  // or target_depth + 1 = source_depth for callers)
  if (usePathBasedLinkFilter && filters.maxDepth !== null) {
    filteredLinks = filteredLinks.filter(link => {
      const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
      const targetId = typeof link.target === 'string' ? link.target : link.target.id;
      
      // Check if this link is on a callee path (source → target direction)
      // Valid if source is at depth N and target is at depth N+1
      if (calleeDepths.has(sourceId) && calleeDepths.has(targetId)) {
        const sourceDepth = calleeDepths.get(sourceId)!;
        const targetDepth = calleeDepths.get(targetId)!;
        if (sourceDepth + 1 === targetDepth) {
          return true;
        }
      }
      
      // Check if this link is on a caller path (target → source direction in the graph)
      // For callers, we traverse backwards: if we're showing callers of X,
      // a link A → X means A is a caller of X, so A is at depth 1 from X
      // Valid if target is at depth N and source is at depth N+1
      if (callerDepths.has(sourceId) && callerDepths.has(targetId)) {
        const sourceDepth = callerDepths.get(sourceId)!;
        const targetDepth = callerDepths.get(targetId)!;
        if (targetDepth + 1 === sourceDepth) {
          return true;
        }
      }
      
      return false;
    });
  }

  // Filter links by call type (inner, precondition, postcondition)
  filteredLinks = filteredLinks.filter(link => {
    const linkType = link.type || 'inner';  // Default to 'inner' for legacy data
    
    // Handle legacy 'calls' type as 'inner'
    if (linkType === 'calls' || linkType === 'inner') {
      return filters.showInnerCalls;
    }
    if (linkType === 'precondition') {
      return filters.showPreconditionCalls;
    }
    if (linkType === 'postcondition') {
      return filters.showPostconditionCalls;
    }
    
    // Unknown type - show by default
    return true;
  });
  
  // Remove isolated nodes (nodes with no edges in the filtered graph)
  // This includes query matches - if a sink has no callers, don't show it
  const connectedNodeIds = new Set<string>();
  
  for (const link of filteredLinks) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    connectedNodeIds.add(sourceId);
    connectedNodeIds.add(targetId);
  }
  
  // Only keep nodes that have at least one edge
  filteredNodes = filteredNodes.filter(node => connectedNodeIds.has(node.id));

  // Create fresh copies of nodes and links to prevent D3 from mutating the originals
  // D3's force simulation modifies link.source/target from string IDs to node references
  return {
    nodes: filteredNodes.map(n => ({ ...n })),
    links: filteredLinks.map(l => ({ source: l.source, target: l.target, type: l.type })),
    metadata: {
      ...fullGraph.metadata,
      total_nodes: filteredNodes.length,
      total_edges: filteredLinks.length,
    },
  };
}

/**
 * Check if a node matches a search query
 * 
 * Supports three modes:
 * - Substring match (default): "foo" matches anything containing "foo"
 * - Exact match: '"foo"' (with quotes) matches only nodes where:
 *   - display_name equals exactly, OR
 *   - the function name part of symbol equals exactly
 * - Path-qualified match: "path:function" matches functions in files/paths containing "path"
 *   Examples:
 *   - "edwards.rs:decompress" → decompress in edwards.rs
 *   - "edwards:decompress" → decompress in any path containing "edwards"
 *   - "u64:mul" → mul in any path containing "u64"
 */
/**
 * @public Exported for testing
 */
export function matchesQuery(node: D3Node, query: string): boolean {
  // Check for Rust-style path-qualified syntax: "path::function"
  // Example: edwards::decompress, ristretto::*compress*
  const doubleColonIndex = query.indexOf('::');
  if (doubleColonIndex > 0 && doubleColonIndex < query.length - 2) {
    const pathPart = query.slice(0, doubleColonIndex);
    const funcPart = query.slice(doubleColonIndex + 2);
    
    // Strip .rs extension from file_name for matching
    // So "edwards" matches "edwards.rs"
    const fileNameWithoutExt = node.file_name.replace(/\.rs$/, '');
    
    // Check if path matches (file_name without .rs, or parent_folder)
    const pathRegex = globToRegex(pathPart);
    const pathMatches = 
      pathRegex.test(fileNameWithoutExt) ||
      pathRegex.test(node.parent_folder);
    
    if (!pathMatches) {
      return false;
    }
    
    // Check if function name matches using glob pattern
    const funcRegex = globToRegex(funcPart);
    return funcRegex.test(node.display_name);
  }
  
  // Use glob pattern matching:
  // - "decompress" (no wildcards) → exact match
  // - "*decompress*" → contains
  // - "decompress*" → starts with
  // - "*decompress" → ends with
  const regex = globToRegex(query);
  return regex.test(node.display_name);
}

/**
 * Find all nodes that lie on any path from source nodes to sink nodes.
 * 
 * Uses DFS with backtracking: explores paths from each source, and when a sink
 * is reached, marks all nodes on the current path as being on a valid path.
 */
function findPathNodes(
  graph: D3Graph,
  sourceIds: Set<string>,
  sinkIds: Set<string>,
  _maxDepth: number | null  // ignored for source→sink paths
): Set<string> {
  // Build forward adjacency (caller → callees)
  const forwardAdj = new Map<string, Set<string>>();
  
  for (const link of graph.links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    
    if (!forwardAdj.has(sourceId)) forwardAdj.set(sourceId, new Set());
    forwardAdj.get(sourceId)!.add(targetId);
  }
  
  const nodesOnPaths = new Set<string>();
  
  // DFS with backtracking from each source
  for (const sourceId of sourceIds) {
    const currentPath: string[] = [];
    const visited = new Set<string>();
    
    function dfs(nodeId: string): boolean {
      // If we've reached a sink, this path is valid
      if (sinkIds.has(nodeId)) {
        // Add current node and all nodes on the path to result
        nodesOnPaths.add(nodeId);
        for (const pathNode of currentPath) {
          nodesOnPaths.add(pathNode);
        }
        return true;
      }
      
      // Avoid cycles
      if (visited.has(nodeId)) {
        return false;
      }
      
      visited.add(nodeId);
      currentPath.push(nodeId);
      
      let foundPath = false;
      const neighbors = forwardAdj.get(nodeId);
      
      if (neighbors) {
        for (const neighbor of neighbors) {
          if (dfs(neighbor)) {
            foundPath = true;
            // Don't break - continue to find all paths
          }
        }
      }
      
      // Backtrack
      currentPath.pop();
      visited.delete(nodeId);
      
      // If we found a path through this node, add it
      if (foundPath) {
        nodesOnPaths.add(nodeId);
      }
      
      return foundPath;
    }
    
    dfs(sourceId);
  }
  
  return nodesOnPaths;
}

/**
 * Result of depth traversal: node IDs mapped to their depth from start
 */
interface DepthTraversalResult {
  nodeDepths: Map<string, number>;  // nodeId -> depth from start
  nodeIds: Set<string>;              // convenience set of all node IDs
}

/**
 * Get callees recursively up to maxDepth, tracking depth of each node
 */
function getCalleesRecursive(
  graph: D3Graph,
  startNodeId: string,
  maxDepth: number | null
): DepthTraversalResult {
  const nodeDepths = new Map<string, number>();
  const queue: Array<{ id: string; depth: number }> = [{ id: startNodeId, depth: 0 }];
  
  // Build adjacency for forward traversal
  const adj = new Map<string, Set<string>>();
  for (const link of graph.links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    if (!adj.has(sourceId)) adj.set(sourceId, new Set());
    adj.get(sourceId)!.add(targetId);
  }
  
  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    if (nodeDepths.has(id)) continue;
    nodeDepths.set(id, depth);
    
    if (maxDepth !== null && depth >= maxDepth) continue;
    
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    
    for (const neighborId of neighbors) {
      if (!nodeDepths.has(neighborId)) {
        queue.push({ id: neighborId, depth: depth + 1 });
      }
    }
  }
  
  return {
    nodeDepths,
    nodeIds: new Set(nodeDepths.keys())
  };
}

/**
 * Get callers recursively up to maxDepth, tracking depth of each node
 */
function getCallersRecursive(
  graph: D3Graph,
  startNodeId: string,
  maxDepth: number | null
): DepthTraversalResult {
  const nodeDepths = new Map<string, number>();
  const queue: Array<{ id: string; depth: number }> = [{ id: startNodeId, depth: 0 }];
  
  // Build adjacency for backward traversal
  const adj = new Map<string, Set<string>>();
  for (const link of graph.links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    if (!adj.has(targetId)) adj.set(targetId, new Set());
    adj.get(targetId)!.add(sourceId);
  }
  
  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    if (nodeDepths.has(id)) continue;
    nodeDepths.set(id, depth);
    
    if (maxDepth !== null && depth >= maxDepth) continue;
    
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    
    for (const neighborId of neighbors) {
      if (!nodeDepths.has(neighborId)) {
        queue.push({ id: neighborId, depth: depth + 1 });
      }
    }
  }
  
  return {
    nodeDepths,
    nodeIds: new Set(nodeDepths.keys())
  };
}

/**
 * Compute nodes within a certain depth from selected nodes
 * (Used for click-based selection filtering)
 */
function computeDepthFromSelected(
  _nodes: D3Node[],
  links: D3Link[],
  selectedNodes: Set<string>,
  maxDepth: number,
  includeCallees: boolean,
  includeCallers: boolean
): Set<string> {
  const adjacencyList = new Map<string, Set<string>>();

  // Build adjacency list based on direction settings
  for (const link of links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;

    if (!adjacencyList.has(sourceId)) adjacencyList.set(sourceId, new Set());
    if (!adjacencyList.has(targetId)) adjacencyList.set(targetId, new Set());

    if (includeCallees) {
      adjacencyList.get(sourceId)!.add(targetId);
    }
    
    if (includeCallers) {
      adjacencyList.get(targetId)!.add(sourceId);
    }
  }

  // BFS from selected nodes
  const visited = new Set<string>();
  const queue: Array<{ id: string; depth: number }> = [];

  for (const nodeId of selectedNodes) {
    queue.push({ id: nodeId, depth: 0 });
    visited.add(nodeId);
  }

  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;

    if (depth >= maxDepth) continue;

    const neighbors = adjacencyList.get(id);
    if (!neighbors) continue;

    for (const neighborId of neighbors) {
      if (!visited.has(neighborId)) {
        visited.add(neighborId);
        queue.push({ id: neighborId, depth: depth + 1 });
      }
    }
  }

  return visited;
}

/**
 * Get immediate callers of a node
 */
export function getCallers(graph: D3Graph, nodeId: string): D3Node[] {
  const callers: D3Node[] = [];
  const nodeMap = new Map(graph.nodes.map(node => [node.id, node]));

  for (const link of graph.links) {
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    if (targetId === nodeId) {
      const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
      const caller = nodeMap.get(sourceId);
      if (caller) callers.push(caller);
    }
  }

  return callers;
}

/**
 * Get immediate callees of a node
 */
export function getCallees(graph: D3Graph, nodeId: string): D3Node[] {
  const callees: D3Node[] = [];
  const nodeMap = new Map(graph.nodes.map(node => [node.id, node]));

  for (const link of graph.links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    if (sourceId === nodeId) {
      const targetId = typeof link.target === 'string' ? link.target : link.target.id;
      const callee = nodeMap.get(targetId);
      if (callee) callees.push(callee);
    }
  }

  return callees;
}
