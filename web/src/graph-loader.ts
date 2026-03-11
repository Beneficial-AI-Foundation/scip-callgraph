import {
  D3Graph, D3Node, D3Link, SimplifiedNode, ProbeAtom,
  isSimplifiedFormat, isD3GraphFormat, isAtomDictFormat, isSchema2Envelope,
  VerificationStatus,
} from './types';

/**
 * Convert simplified JSON format (array of nodes with deps) to D3Graph format.
 */
export function convertSimplifiedToD3Graph(nodes: SimplifiedNode[]): D3Graph {
  const knownIds = new Set(nodes.map(n => n.identifier));

  const dependentsMap = new Map<string, string[]>();
  for (const node of nodes) {
    if (!dependentsMap.has(node.identifier)) {
      dependentsMap.set(node.identifier, []);
    }
    for (const dep of node.deps) {
      if (!dependentsMap.has(dep)) {
        dependentsMap.set(dep, []);
      }
      dependentsMap.get(dep)!.push(node.identifier);
    }
  }

  const d3Nodes: D3Node[] = nodes.map(node => {
    let fullPath = node.full_path;
    if (fullPath.startsWith('file://')) {
      fullPath = fullPath.substring(7);
    }
    const filteredDeps = node.deps.filter(dep => knownIds.has(dep));
    const dependents = dependentsMap.get(node.identifier) || [];

    return {
      id: node.identifier,
      display_name: node.display_name,
      symbol: node.identifier,
      full_path: fullPath,
      relative_path: node.relative_path,
      file_name: node.file_name,
      parent_folder: node.parent_folder,
      crate_name: '',
      start_line: undefined,
      end_line: undefined,
      is_libsignal: false,
      dependencies: filteredDeps,
      dependents: dependents.filter(dep => knownIds.has(dep)),
      kind: 'exec' as const,
    };
  });

  const links: D3Link[] = [];
  for (const node of d3Nodes) {
    for (const dep of node.dependencies) {
      links.push({ source: node.id, target: dep, type: 'inner' });
    }
  }

  return {
    nodes: d3Nodes,
    links,
    metadata: {
      total_nodes: d3Nodes.length,
      total_edges: links.length,
      project_root: 'Simplified JSON (no project root)',
      generated_at: new Date().toISOString(),
    },
  };
}

/**
 * Convert probe atom dict format (probe-verus / probe-lean atoms.json) to D3Graph format.
 */
export function convertAtomDictToD3Graph(atoms: Record<string, ProbeAtom>): D3Graph {
  const knownIds = new Set(Object.keys(atoms));

  const dependentsMap = new Map<string, string[]>();
  for (const [atomName, atom] of Object.entries(atoms)) {
    if (!dependentsMap.has(atomName)) {
      dependentsMap.set(atomName, []);
    }
    for (const dep of atom.dependencies) {
      if (!dependentsMap.has(dep)) {
        dependentsMap.set(dep, []);
      }
      dependentsMap.get(dep)!.push(atomName);
    }
  }

  const d3Nodes: D3Node[] = Object.entries(atoms).map(([atomName, atom]) => {
    const codePath = atom["code-path"] || '';
    const parts = codePath.split('/');
    const fileName = parts[parts.length - 1] || 'unknown';
    const parentFolder = parts.length >= 2 ? parts[parts.length - 2] : 'unknown';

    const filteredDeps = atom.dependencies.filter(dep => knownIds.has(dep));
    const dependents = (dependentsMap.get(atomName) || []).filter(dep => knownIds.has(dep));

    const codeText = atom["code-text"];

    return {
      id: atomName,
      display_name: atom["display-name"],
      symbol: atomName,
      full_path: codePath,
      relative_path: codePath,
      file_name: fileName,
      parent_folder: parentFolder,
      crate_name: '',
      start_line: codeText ? codeText["lines-start"] : undefined,
      end_line: codeText ? codeText["lines-end"] : undefined,
      is_libsignal: false,
      dependencies: filteredDeps,
      dependents,
      kind: atom.kind || 'exec',
      verification_status: atom["verification-status"] as VerificationStatus | undefined,
    };
  });

  const links: D3Link[] = [];
  for (const [atomName, atom] of Object.entries(atoms)) {
    if (atom["dependencies-with-locations"] && atom["dependencies-with-locations"].length > 0) {
      for (const dep of atom["dependencies-with-locations"]) {
        if (knownIds.has(dep["code-name"])) {
          links.push({
            source: atomName,
            target: dep["code-name"],
            type: dep.location || 'inner',
          });
        }
      }
    } else {
      for (const dep of atom.dependencies) {
        if (knownIds.has(dep)) {
          links.push({ source: atomName, target: dep, type: 'inner' });
        }
      }
    }
  }

  return {
    nodes: d3Nodes,
    links,
    metadata: {
      total_nodes: d3Nodes.length,
      total_edges: links.length,
      project_root: 'Probe atom dict',
      generated_at: new Date().toISOString(),
    },
  };
}

/**
 * Parse JSON data and convert to D3Graph format if needed.
 * Supports D3Graph format, simplified format, probe atom dict format,
 * and Schema 2.0 envelopes.
 */
export function parseAndNormalizeGraph(data: unknown): D3Graph {
  if (isSchema2Envelope(data)) {
    return parseAndNormalizeGraph(data.data);
  }
  if (isD3GraphFormat(data)) {
    return data;
  }
  if (isAtomDictFormat(data)) {
    return convertAtomDictToD3Graph(data);
  }
  if (isSimplifiedFormat(data)) {
    return convertSimplifiedToD3Graph(data);
  }
  console.warn('Unknown JSON format, attempting to use as D3Graph');
  return data as D3Graph;
}
