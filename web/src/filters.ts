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

  // Identify nodes that match the search query
  const query = filters.searchQuery.trim().toLowerCase();
  const searchMatchIds = new Set<string>();
  
  if (query !== '') {
    fullGraph.nodes.forEach(node => {
      if (
        node.display_name.toLowerCase().includes(query) ||
        node.symbol.toLowerCase().includes(query) ||
        node.file_name.toLowerCase().includes(query)
      ) {
        searchMatchIds.add(node.id);
      }
    });
  }

  // Build the set of nodes to include
  const includedNodeIds = new Set<string>();

  // If there's a search query, include matching nodes and their callers/callees
  if (query !== '') {
    // Add all search matches
    searchMatchIds.forEach(id => includedNodeIds.add(id));

    // Add callers if enabled
    if (filters.includeCallers) {
      searchMatchIds.forEach(nodeId => {
        const callers = getCallers(fullGraph, nodeId);
        callers.forEach(caller => includedNodeIds.add(caller.id));
      });
    }

    // Add callees if enabled
    if (filters.includeCallees) {
      searchMatchIds.forEach(nodeId => {
        const callees = getCallees(fullGraph, nodeId);
        callees.forEach(callee => includedNodeIds.add(callee.id));
      });
    }

    // Filter to only included nodes
    filteredNodes = filteredNodes.filter(node => includedNodeIds.has(node.id));
  }
  // If no search, show all nodes (includeCallers/Callees don't apply without search)

  // Filter by libsignal flag
  if (!filters.showLibsignal || !filters.showNonLibsignal) {
    filteredNodes = filteredNodes.filter(node => {
      if (node.is_libsignal && !filters.showLibsignal) return false;
      if (!node.is_libsignal && !filters.showNonLibsignal) return false;
      return true;
    });
  }

  // Create a set of valid node IDs for efficient lookup
  const validNodeIds = new Set(filteredNodes.map(node => node.id));

  // Filter links to only include those between valid nodes
  filteredLinks = filteredLinks.filter(link => {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    return validNodeIds.has(sourceId) && validNodeIds.has(targetId);
  });

  // Apply depth filtering if specified
  if (filters.maxDepth !== null && filters.selectedNodes.size > 0) {
    const nodesAtDepth = computeDepthFromSelected(
      filteredNodes,
      filteredLinks,
      filters.selectedNodes,
      filters.maxDepth
    );
    
    filteredNodes = filteredNodes.filter(node => nodesAtDepth.has(node.id));
    
    const depthNodeIds = new Set(filteredNodes.map(node => node.id));
    filteredLinks = filteredLinks.filter(link => {
      const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
      const targetId = typeof link.target === 'string' ? link.target : link.target.id;
      return depthNodeIds.has(sourceId) && depthNodeIds.has(targetId);
    });
  }

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
 * Compute nodes within a certain depth from selected nodes
 */
function computeDepthFromSelected(
  nodes: D3Node[],
  links: D3Link[],
  selectedNodes: Set<string>,
  maxDepth: number
): Set<string> {
  const adjacencyList = new Map<string, Set<string>>();

  // Build adjacency list (bidirectional for depth traversal)
  for (const link of links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;

    if (!adjacencyList.has(sourceId)) adjacencyList.set(sourceId, new Set());
    if (!adjacencyList.has(targetId)) adjacencyList.set(targetId, new Set());

    adjacencyList.get(sourceId)!.add(targetId);
    adjacencyList.get(targetId)!.add(sourceId);
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
 * Get callers of a node
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
 * Get callees of a node
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

