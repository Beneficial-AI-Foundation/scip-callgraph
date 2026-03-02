import { D3Graph, D3Node, BorderStatus, FillStatus } from './types';

/**
 * Compute derived border_status and fill_status for each node in the graph.
 *
 * border_status (this node's own readiness):
 *   - 'verified':  this node is verified
 *   - 'blocked':   this node failed verification
 *   - 'ready':     all dependencies are verified, this one is not yet
 *   - 'not_ready': some dependency is not yet verified
 *   - 'unknown':   no verification data
 *
 * fill_status (subtree completeness):
 *   - 'fully_verified': this node AND all ancestors are verified
 *   - 'verified':       this node is verified but some ancestor is not
 *   - 'ready':          all dependencies are fully_verified, node not yet verified
 *   - 'none':           not yet ready
 *
 * Mutates nodes in place.
 */
export function computeDerivedStatuses(graph: D3Graph): void {
  const nodeMap = new Map<string, D3Node>();
  for (const node of graph.nodes) nodeMap.set(node.id, node);

  // Build dependency adjacency (node -> its dependencies)
  const deps = new Map<string, string[]>();
  for (const node of graph.nodes) {
    deps.set(node.id, node.dependencies.filter(d => nodeMap.has(d)));
  }

  // Topological sort (Kahn's algorithm) so we process leaves first
  const inDegree = new Map<string, number>();
  const reverseAdj = new Map<string, string[]>();
  for (const node of graph.nodes) {
    inDegree.set(node.id, 0);
    reverseAdj.set(node.id, []);
  }
  for (const [id, depList] of deps) {
    for (const dep of depList) {
      if (reverseAdj.has(dep)) {
        reverseAdj.get(dep)!.push(id);
      }
      inDegree.set(id, (inDegree.get(id) || 0) + 1);
    }
  }
  // Reset in-degree based on our filtered deps
  for (const node of graph.nodes) inDegree.set(node.id, 0);
  for (const [_, depList] of deps) {
    for (const dep of depList) {
      if (inDegree.has(dep)) {
        // dep is depended-upon by _, so _'s in-degree should count deps going FROM _ TO dep
      }
    }
  }

  // Actually: in-degree here means "number of deps this node has"
  // We process nodes whose deps are all already computed
  const computed = new Set<string>();
  const queue: string[] = [];

  // Nodes with no dependencies can be computed immediately
  for (const node of graph.nodes) {
    const d = deps.get(node.id) || [];
    if (d.length === 0) queue.push(node.id);
  }

  const borderOf = new Map<string, BorderStatus>();
  const fillOf = new Map<string, FillStatus>();

  while (queue.length > 0) {
    const id = queue.shift()!;
    if (computed.has(id)) continue;
    computed.add(id);

    const node = nodeMap.get(id)!;
    const nodeDeps = deps.get(id) || [];

    // Compute border_status
    if (!node.verification_status) {
      const allDepsVerified = nodeDeps.length > 0 &&
        nodeDeps.every(d => borderOf.get(d) === 'verified');
      borderOf.set(id, allDepsVerified ? 'ready' : 'unknown');
    } else if (node.verification_status === 'verified') {
      borderOf.set(id, 'verified');
    } else if (node.verification_status === 'failed') {
      borderOf.set(id, 'blocked');
    } else {
      const allDepsVerified = nodeDeps.length === 0 ||
        nodeDeps.every(d => borderOf.get(d) === 'verified');
      borderOf.set(id, allDepsVerified ? 'ready' : 'not_ready');
    }

    // Compute fill_status
    if (node.verification_status === 'verified') {
      const allDepsFullyVerified = nodeDeps.length === 0 ||
        nodeDeps.every(d => fillOf.get(d) === 'fully_verified');
      fillOf.set(id, allDepsFullyVerified ? 'fully_verified' : 'verified');
    } else {
      const allDepsFullyVerified = nodeDeps.length > 0 &&
        nodeDeps.every(d => fillOf.get(d) === 'fully_verified');
      fillOf.set(id, allDepsFullyVerified ? 'ready' : 'none');
    }

    // Enqueue dependents whose deps are now all computed
    for (const dependent of reverseAdj.get(id) || []) {
      if (computed.has(dependent)) continue;
      const depDeps = deps.get(dependent) || [];
      if (depDeps.every(d => computed.has(d))) {
        queue.push(dependent);
      }
    }
  }

  // Handle any unvisited nodes (cycles)
  for (const node of graph.nodes) {
    if (!computed.has(node.id)) {
      borderOf.set(node.id, 'unknown');
      fillOf.set(node.id, 'none');
    }
  }

  // Write back to nodes
  for (const node of graph.nodes) {
    node.border_status = borderOf.get(node.id);
    node.fill_status = fillOf.get(node.id);
  }
}
