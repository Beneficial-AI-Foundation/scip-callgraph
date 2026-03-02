import { D3Node, D3Link } from './types';

/**
 * Compute the transitive reduction of a DAG.
 * Removes edge u->v if v is reachable from u via another path.
 */
export function transitiveReduction(nodes: D3Node[], links: D3Link[]): D3Link[] {
  const nodeIds = new Set(nodes.map(n => n.id));

  const adj = new Map<string, Set<string>>();
  for (const id of nodeIds) adj.set(id, new Set());

  for (const link of links) {
    const s = typeof link.source === 'string' ? link.source : link.source.id;
    const t = typeof link.target === 'string' ? link.target : link.target.id;
    if (nodeIds.has(s) && nodeIds.has(t)) {
      adj.get(s)!.add(t);
    }
  }

  const redundant = new Set<string>();

  for (const link of links) {
    const s = typeof link.source === 'string' ? link.source : link.source.id;
    const t = typeof link.target === 'string' ? link.target : link.target.id;
    if (!nodeIds.has(s) || !nodeIds.has(t)) continue;

    // BFS from s, skipping the direct s->t edge, to see if t is reachable
    const visited = new Set<string>();
    const queue: string[] = [];
    for (const next of adj.get(s)!) {
      if (next !== t) {
        queue.push(next);
        visited.add(next);
      }
    }

    let reachable = false;
    while (queue.length > 0) {
      const cur = queue.shift()!;
      if (cur === t) { reachable = true; break; }
      for (const next of adj.get(cur) || []) {
        if (!visited.has(next)) {
          visited.add(next);
          queue.push(next);
        }
      }
    }

    if (reachable) {
      const key = `${s}\0${t}\0${(link as any).type || 'inner'}`;
      redundant.add(key);
    }
  }

  return links.filter(link => {
    const s = typeof link.source === 'string' ? link.source : link.source.id;
    const t = typeof link.target === 'string' ? link.target : link.target.id;
    const key = `${s}\0${t}\0${(link as any).type || 'inner'}`;
    return !redundant.has(key);
  });
}
