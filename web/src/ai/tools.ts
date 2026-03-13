/**
 * AI tool definitions and execution bridge.
 *
 * Each tool maps to a viewer state mutation. The AI calls tools via function
 * calling; the execution bridge validates parameters, performs the action,
 * and returns structured feedback including match counts and suggestions.
 */

import type { D3Node } from '../types';
import type { ToolDef, ToolResult, ToolCall, ViewerStateAccessor } from './types';
import { buildGraphSummary, formatSummaryText } from './static-analysis';

// ============================================================================
// Tool Definitions (sent to the LLM)
// ============================================================================

export const TOOL_DEFINITIONS: ToolDef[] = [
  {
    name: 'set_source',
    description: 'Set the source query to explore what a function calls (its callees). Supports substring match, glob patterns (p_*), path-qualified (edwards::decompress), and crate-level (crate:name).',
    parameters: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Source query string' },
      },
      required: ['query'],
    },
  },
  {
    name: 'set_sink',
    description: 'Set the sink query to explore who calls a function (its callers). Same pattern syntax as set_source.',
    parameters: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Sink query string' },
      },
      required: ['query'],
    },
  },
  {
    name: 'set_depth',
    description: 'Set the maximum traversal depth (number of hops from source/sink). Use 0 for unlimited.',
    parameters: {
      type: 'object',
      properties: {
        depth: { type: 'number', description: 'Max depth (0 = unlimited, 1-10 = specific depth)' },
      },
      required: ['depth'],
    },
  },
  {
    name: 'toggle_kind',
    description: 'Show or hide a declaration kind (exec/proof/spec for Verus, or def/theorem/axiom for Lean).',
    parameters: {
      type: 'object',
      properties: {
        kind: { type: 'string', description: 'Declaration kind', enum: ['exec', 'proof', 'spec'] },
        show: { type: 'string', description: 'Whether to show (true) or hide (false)', enum: ['true', 'false'] },
      },
      required: ['kind', 'show'],
    },
  },
  {
    name: 'filter_verification',
    description: 'Filter nodes by verification status. Only nodes with the specified statuses will be shown.',
    parameters: {
      type: 'object',
      properties: {
        statuses: {
          type: 'array',
          description: 'Array of statuses to show',
          items: { type: 'string' },
        },
      },
      required: ['statuses'],
    },
  },
  {
    name: 'exclude_by_name',
    description: 'Exclude functions matching a glob pattern by display name. Comma-separated for multiple patterns.',
    parameters: {
      type: 'object',
      properties: {
        pattern: { type: 'string', description: 'Glob pattern(s) to exclude, e.g. "*_comm*, lemma_mul_*"' },
      },
      required: ['pattern'],
    },
  },
  {
    name: 'include_files',
    description: 'Only show functions from specific files. Empty string means all files.',
    parameters: {
      type: 'object',
      properties: {
        files: { type: 'string', description: 'Comma-separated file names or glob patterns, e.g. "edwards.rs, field.rs"' },
      },
      required: ['files'],
    },
  },
  {
    name: 'switch_view',
    description: 'Switch the visualization mode.',
    parameters: {
      type: 'object',
      properties: {
        view: { type: 'string', description: 'View to switch to', enum: ['callgraph', 'blueprint', 'crate-map'] },
      },
      required: ['view'],
    },
  },
  {
    name: 'select_node',
    description: 'Select a node by name to show its details in the sidebar.',
    parameters: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Display name (or substring) of the node to select' },
      },
      required: ['name'],
    },
  },
  {
    name: 'set_crate_boundary',
    description: 'Show the boundary between two crates: functions in the source crate that are called by the target crate.',
    parameters: {
      type: 'object',
      properties: {
        source: { type: 'string', description: 'Source crate name' },
        target: { type: 'string', description: 'Target crate name' },
      },
      required: ['source', 'target'],
    },
  },
  {
    name: 'reset_filters',
    description: 'Reset all filters to their defaults and show the full graph.',
    parameters: { type: 'object', properties: {} },
  },
  {
    name: 'get_node_details',
    description: 'Get detailed information about a specific function/node by name.',
    parameters: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Display name (or substring) of the node' },
      },
      required: ['name'],
    },
  },
  {
    name: 'get_graph_stats',
    description: 'Get current graph statistics: node counts, verification breakdown, crate list.',
    parameters: { type: 'object', properties: {} },
  },
  {
    name: 'list_crates',
    description: 'List all crates/namespaces in the graph with their function counts.',
    parameters: { type: 'object', properties: {} },
  },
  {
    name: 'list_files',
    description: 'List files in the graph, optionally filtered to a specific crate.',
    parameters: {
      type: 'object',
      properties: {
        crate: { type: 'string', description: 'Optional crate name to filter files' },
      },
    },
  },
  {
    name: 'list_unverified_hotspots',
    description: 'List unverified functions that are called by verified code (good verification targets).',
    parameters: {
      type: 'object',
      properties: {
        limit: { type: 'number', description: 'Max results to return (default 10)' },
      },
    },
  },
];

// ============================================================================
// Tool Execution
// ============================================================================

/**
 * Execute a tool call against the viewer state.
 * Returns structured feedback for the AI.
 */
export function executeTool(
  call: ToolCall,
  accessor: ViewerStateAccessor,
): ToolResult {
  const args = call.arguments;
  const graph = accessor.getFullGraph();

  if (!graph) {
    return { success: false, error: 'No graph loaded' };
  }

  try {
    switch (call.name) {
      case 'set_source': {
        const query = validateString(args.query, 'query');
        accessor.setSource(query);
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return {
          success: true,
          matchedNodes: filtered?.nodes.length ?? 0,
          data: { query },
          suggestion: (filtered?.nodes.length ?? 0) === 0
            ? suggestAlternative(query, graph.nodes)
            : undefined,
        };
      }

      case 'set_sink': {
        const query = validateString(args.query, 'query');
        accessor.setSink(query);
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return {
          success: true,
          matchedNodes: filtered?.nodes.length ?? 0,
          data: { query },
          suggestion: (filtered?.nodes.length ?? 0) === 0
            ? suggestAlternative(query, graph.nodes)
            : undefined,
        };
      }

      case 'set_depth': {
        const depth = validateNumber(args.depth, 'depth', 0, 10);
        accessor.setDepth(depth === 0 ? null : depth);
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'toggle_kind': {
        const kind = validateEnum(args.kind, 'kind', ['exec', 'proof', 'spec']);
        const show = args.show === 'true' || args.show === true;
        const updates: Record<string, boolean> = {};
        if (kind === 'exec') updates.showExecFunctions = show;
        if (kind === 'proof') updates.showProofFunctions = show;
        if (kind === 'spec') updates.showSpecFunctions = show;
        accessor.setFilters(updates);
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'filter_verification': {
        const statuses = validateArray(args.statuses, 'statuses') as string[];
        const validStatuses = new Set(['verified', 'failed', 'unverified']);
        for (const s of statuses) {
          if (!validStatuses.has(s)) {
            return { success: false, error: `Invalid status: ${s}. Valid: verified, failed, unverified` };
          }
        }
        accessor.setFilters({
          showVerifiedNodes: statuses.includes('verified'),
          showFailedNodes: statuses.includes('failed'),
          showUnverifiedNodes: statuses.includes('unverified'),
        });
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'exclude_by_name': {
        const pattern = validateString(args.pattern, 'pattern');
        accessor.setFilters({ excludeNamePatterns: pattern });
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'include_files': {
        const files = typeof args.files === 'string' ? args.files : '';
        accessor.setFilters({ includeFiles: files });
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'switch_view': {
        const view = validateEnum(args.view, 'view', ['callgraph', 'blueprint', 'crate-map']);
        accessor.switchView(view);
        return { success: true };
      }

      case 'select_node': {
        const name = validateString(args.name, 'name');
        const found = accessor.selectNodeByName(name);
        if (!found) {
          return {
            success: false,
            error: `Node "${name}" not found in current view`,
            suggestion: suggestAlternative(name, accessor.getFilteredGraph()?.nodes ?? []),
          };
        }
        const selected = accessor.getSelectedNode();
        return {
          success: true,
          data: selected ? formatNodeDetails(selected) : undefined,
        };
      }

      case 'set_crate_boundary': {
        const source = validateString(args.source, 'source');
        const target = validateString(args.target, 'target');
        accessor.setSource(`crate:${source}`);
        accessor.setSink(`crate:${target}`);
        accessor.applyFiltersAndUpdate();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'reset_filters': {
        accessor.resetFilters();
        const filtered = accessor.getFilteredGraph();
        return { success: true, matchedNodes: filtered?.nodes.length ?? 0 };
      }

      case 'get_node_details': {
        const name = validateString(args.name, 'name');
        const node = findNodeByName(name, graph.nodes);
        if (!node) {
          return {
            success: false,
            error: `Node "${name}" not found`,
            suggestion: suggestAlternative(name, graph.nodes),
          };
        }
        return { success: true, data: formatNodeDetails(node) };
      }

      case 'get_graph_stats': {
        const summary = buildGraphSummary(graph);
        return { success: true, data: formatSummaryText(summary) };
      }

      case 'list_crates': {
        const summary = buildGraphSummary(graph);
        const crateList = summary.crates.map(c =>
          `${c.name}: ${c.nodeCount} functions, ${c.fileCount} files${c.isExternal ? ' (external)' : ''}`
        );
        return { success: true, data: crateList.join('\n') };
      }

      case 'list_files': {
        const crateName = typeof args.crate === 'string' ? args.crate : undefined;
        let nodes = graph.nodes;
        if (crateName) {
          const lc = crateName.toLowerCase();
          nodes = nodes.filter(n => n.crate_name.toLowerCase().includes(lc));
        }
        const files = new Set(nodes.map(n => n.relative_path || n.file_name).filter(Boolean));
        return { success: true, data: [...files].sort().join('\n') };
      }

      case 'list_unverified_hotspots': {
        const limit = typeof args.limit === 'number' ? args.limit : 10;
        const summary = buildGraphSummary(graph);
        const hotspots = summary.unverifiedHotspots.slice(0, limit);
        if (hotspots.length === 0) {
          return { success: true, data: 'No unverified hotspots found (no unverified functions called by verified code).' };
        }
        const text = hotspots.map(h =>
          `${h.displayName} (${h.kind}, ${h.crateName}, ${h.dependentCount} callers)`
        ).join('\n');
        return { success: true, data: text };
      }

      default:
        return { success: false, error: `Unknown tool: ${call.name}` };
    }
  } catch (err) {
    return { success: false, error: err instanceof Error ? err.message : String(err) };
  }
}

// ============================================================================
// Helpers
// ============================================================================

function validateString(value: unknown, name: string): string {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`Parameter "${name}" must be a non-empty string`);
  }
  return value.trim();
}

function validateNumber(value: unknown, name: string, min: number, max: number): number {
  const n = typeof value === 'number' ? value : parseInt(String(value), 10);
  if (isNaN(n) || n < min || n > max) {
    throw new Error(`Parameter "${name}" must be a number between ${min} and ${max}`);
  }
  return n;
}

function validateEnum(value: unknown, name: string, allowed: string[]): string {
  const s = String(value);
  if (!allowed.includes(s)) {
    throw new Error(`Parameter "${name}" must be one of: ${allowed.join(', ')}`);
  }
  return s;
}

function validateArray(value: unknown, name: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`Parameter "${name}" must be an array`);
  }
  return value;
}

function findNodeByName(name: string, nodes: D3Node[]): D3Node | undefined {
  const lower = name.toLowerCase();
  return nodes.find(n => n.display_name.toLowerCase() === lower)
    || nodes.find(n => n.display_name.toLowerCase().includes(lower));
}

function suggestAlternative(query: string, nodes: D3Node[]): string {
  const lower = query.toLowerCase();
  const scored = nodes
    .map(n => ({ name: n.display_name, score: fuzzyScore(lower, n.display_name.toLowerCase()) }))
    .filter(s => s.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, 3);

  if (scored.length === 0) return 'No similar functions found.';
  return `Did you mean: ${scored.map(s => s.name).join(', ')}?`;
}

function fuzzyScore(query: string, target: string): number {
  if (target.includes(query)) return 3;
  let score = 0;
  let qi = 0;
  for (let ti = 0; ti < target.length && qi < query.length; ti++) {
    if (target[ti] === query[qi]) { score++; qi++; }
  }
  return qi === query.length ? score / query.length : 0;
}

function formatNodeDetails(node: D3Node): string {
  const lines = [
    `Name: ${node.display_name}`,
    `Kind: ${node.kind || 'unknown'}`,
    `Verification: ${node.verification_status || 'unknown'}`,
    `Crate: ${node.crate_name}`,
    `File: ${node.relative_path || node.file_name}`,
  ];
  if (node.start_line != null) {
    lines.push(`Lines: ${node.start_line}${node.end_line ? '-' + node.end_line : ''}`);
  }
  lines.push(`Callers: ${node.dependents?.length ?? 0}`);
  lines.push(`Callees: ${node.dependencies?.length ?? 0}`);

  if (node.dependents && node.dependents.length > 0) {
    const callerNames = node.dependents.slice(0, 10).map(id => {
      const lastSlash = id.lastIndexOf('/');
      return lastSlash >= 0 ? id.slice(lastSlash + 1) : id;
    });
    lines.push(`Top callers: ${callerNames.join(', ')}`);
  }
  if (node.dependencies && node.dependencies.length > 0) {
    const calleeNames = node.dependencies.slice(0, 10).map(id => {
      const lastSlash = id.lastIndexOf('/');
      return lastSlash >= 0 ? id.slice(lastSlash + 1) : id;
    });
    lines.push(`Top callees: ${calleeNames.join(', ')}`);
  }

  return lines.join('\n');
}
