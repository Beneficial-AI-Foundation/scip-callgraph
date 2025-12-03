import { D3Graph, D3Node, D3Link, FilterOptions } from './types';

/**
 * Apply filters to the full graph and return a filtered graph
 */
export function applyFilters(
  fullGraph: D3Graph,
  filters: FilterOptions
): D3Graph {
  let filteredNodes = [...fullGraph.nodes];
  let filteredLinks = [...fullGraph.links];

  const sourceQuery = filters.sourceQuery.trim().toLowerCase();
  const sinkQuery = filters.sinkQuery.trim().toLowerCase();
  
  console.log('[DEBUG] Source query:', JSON.stringify(sourceQuery), 'Sink query:', JSON.stringify(sinkQuery));

  // Find nodes matching source and sink queries
  const sourceMatchIds = new Set<string>();
  const sinkMatchIds = new Set<string>();
  
  if (sourceQuery !== '') {
    fullGraph.nodes.forEach(node => {
      if (matchesQuery(node, sourceQuery)) {
        sourceMatchIds.add(node.id);
      }
    });
    // Log matched node names for debugging
    const sourceNames = fullGraph.nodes
      .filter(n => sourceMatchIds.has(n.id))
      .map(n => `${n.display_name} (${n.file_name})`);
    console.log('[DEBUG] Source matches:', sourceMatchIds.size, sourceNames);
  }
  
  if (sinkQuery !== '') {
    fullGraph.nodes.forEach(node => {
      if (matchesQuery(node, sinkQuery)) {
        sinkMatchIds.add(node.id);
      }
    });
    const sinkNames = fullGraph.nodes
      .filter(n => sinkMatchIds.has(n.id))
      .map(n => `${n.display_name} (${n.file_name})`);
    console.log('[DEBUG] Sink matches:', sinkMatchIds.size, sinkNames);
  }

  // Build the set of nodes to include based on source/sink configuration
  const includedNodeIds = new Set<string>();

  const hasSource = sourceQuery !== '' && sourceMatchIds.size > 0;
  const hasSink = sinkQuery !== '' && sinkMatchIds.size > 0;
  
  // Check if source and sink are the same (for neighborhood mode)
  const isSameQuery = sourceQuery !== '' && sourceQuery === sinkQuery;

  if (hasSource && hasSink) {
    if (isSameQuery) {
      // Same node in both → show full neighborhood (both directions)
      console.log('[DEBUG] Same query mode - showing full neighborhood');
      sourceMatchIds.forEach(id => includedNodeIds.add(id));
      
      // Get both callers and callees
      sourceMatchIds.forEach(nodeId => {
        const callers = getCallersRecursive(fullGraph, nodeId, filters.maxDepth);
        const callees = getCalleesRecursive(fullGraph, nodeId, filters.maxDepth);
        callers.forEach(id => includedNodeIds.add(id));
        callees.forEach(id => includedNodeIds.add(id));
      });
    } else {
      // Different source and sink → find paths between them
      // NOTE: We ignore maxDepth here - we want ALL nodes on any path from source to sink
      console.log('[DEBUG] Path mode - finding paths from source to sink (ignoring depth limit)');
      const pathNodes = findPathNodes(fullGraph, sourceMatchIds, sinkMatchIds, null);
      pathNodes.forEach(id => includedNodeIds.add(id));
    }
  } else if (hasSource) {
    // Source only → show callees (what source calls)
    console.log('[DEBUG] Source-only mode - showing callees');
    sourceMatchIds.forEach(id => includedNodeIds.add(id));
    
    sourceMatchIds.forEach(nodeId => {
      const callees = getCalleesRecursive(fullGraph, nodeId, filters.maxDepth);
      callees.forEach(id => includedNodeIds.add(id));
    });
  } else if (hasSink) {
    // Sink only → show callers (who calls sink)
    console.log('[DEBUG] Sink-only mode - showing callers');
    sinkMatchIds.forEach(id => includedNodeIds.add(id));
    
    sinkMatchIds.forEach(nodeId => {
      const callers = getCallersRecursive(fullGraph, nodeId, filters.maxDepth);
      callers.forEach(id => includedNodeIds.add(id));
    });
  }
  // If neither source nor sink, show all nodes (no filtering by query)

  // Filter nodes if we have any source/sink constraints
  if (hasSource || hasSink) {
    filteredNodes = filteredNodes.filter(node => includedNodeIds.has(node.id));
  }

  // Filter by libsignal flag
  if (!filters.showLibsignal || !filters.showNonLibsignal) {
    filteredNodes = filteredNodes.filter(node => {
      if (node.is_libsignal && !filters.showLibsignal) return false;
      if (!node.is_libsignal && !filters.showNonLibsignal) return false;
      return true;
    });
  }

  // Apply depth filtering from selected nodes (click-based selection)
  if (filters.maxDepth !== null && filters.selectedNodes.size > 0) {
    const nodesAtDepth = computeDepthFromSelected(
      fullGraph.nodes,
      fullGraph.links,
      filters.selectedNodes,
      filters.maxDepth,
      hasSource || isSameQuery,  // include callees direction
      hasSink || isSameQuery     // include callers direction
    );
    
    filteredNodes = fullGraph.nodes.filter(node => nodesAtDepth.has(node.id));
    
    // Also apply libsignal filter to depth-expanded nodes
    if (!filters.showLibsignal || !filters.showNonLibsignal) {
      filteredNodes = filteredNodes.filter(node => {
        if (node.is_libsignal && !filters.showLibsignal) return false;
        if (!node.is_libsignal && !filters.showNonLibsignal) return false;
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
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    return validNodeIds.has(sourceId) && validNodeIds.has(targetId);
  });

  return {
    nodes: filteredNodes,
    links: filteredLinks,
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
 * Supports two modes:
 * - Substring match (default): "foo" matches anything containing "foo"
 * - Exact match: '"foo"' (with quotes) matches only nodes where:
 *   - display_name equals exactly, OR
 *   - the function name part of symbol equals exactly
 */
function matchesQuery(node: D3Node, query: string): boolean {
  // Check for exact match syntax: "query" (surrounded by quotes)
  if (query.startsWith('"') && query.endsWith('"') && query.length > 2) {
    const exactQuery = query.slice(1, -1).toLowerCase();
    
    // Check display_name
    if (node.display_name.toLowerCase() === exactQuery) {
      return true;
    }
    
    // Also check the function name extracted from symbol
    // Symbol format varies, but function name is usually after last `/` and before `(`
    // e.g., "rust-analyzer .../scalar.rs/impl Scalar52/as_bytes()."
    const symbolLower = node.symbol.toLowerCase();
    const funcNameMatch = symbolLower.match(/\/([^/(]+)\([^)]*\)[.`]?$/);
    if (funcNameMatch && funcNameMatch[1] === exactQuery) {
      return true;
    }
    
    return false;
  }
  
  // Default: substring match
  const matchesName = node.display_name.toLowerCase().includes(query);
  const matchesSymbol = node.symbol.toLowerCase().includes(query);
  const matchesFile = node.file_name.toLowerCase().includes(query);
  return matchesName || matchesSymbol || matchesFile;
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
  
  console.log('[DEBUG] DFS path finding complete. Nodes on paths:', nodesOnPaths.size);
  
  return nodesOnPaths;
}

/**
 * Get callees recursively up to maxDepth
 */
function getCalleesRecursive(
  graph: D3Graph,
  startNodeId: string,
  maxDepth: number | null
): Set<string> {
  const visited = new Set<string>();
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
    if (visited.has(id)) continue;
    visited.add(id);
    
    if (maxDepth !== null && depth >= maxDepth) continue;
    
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    
    for (const neighborId of neighbors) {
      if (!visited.has(neighborId)) {
        queue.push({ id: neighborId, depth: depth + 1 });
      }
    }
  }
  
  return visited;
}

/**
 * Get callers recursively up to maxDepth
 */
function getCallersRecursive(
  graph: D3Graph,
  startNodeId: string,
  maxDepth: number | null
): Set<string> {
  const visited = new Set<string>();
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
    if (visited.has(id)) continue;
    visited.add(id);
    
    if (maxDepth !== null && depth >= maxDepth) continue;
    
    const neighbors = adj.get(id);
    if (!neighbors) continue;
    
    for (const neighborId of neighbors) {
      if (!visited.has(neighborId)) {
        queue.push({ id: neighborId, depth: depth + 1 });
      }
    }
  }
  
  return visited;
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
