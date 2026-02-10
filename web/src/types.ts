// Type definitions matching the Rust D3Graph structure

export interface SimilarLemma {
  name: string;
  score: number;
  file_path: string;
  line_number: number | null;
  signature: string;
  source: string;  // "project" or "vstd"
}

// ============================================================================
// Simplified JSON Format (curve25519-dalek.json style)
// ============================================================================

/**
 * Node format used in simplified JSON files (like curve25519-dalek.json)
 * This is a flat array format without explicit links/metadata
 */
export interface SimplifiedNode {
  identifier: string;           // Maps to id
  statement_type?: string;      // e.g., "function" - informational only
  deps: string[];               // Maps to dependencies
  body?: string;                // Function body (code) - optional
  display_name: string;
  full_path: string;            // May have file:// prefix
  relative_path: string;
  file_name: string;
  parent_folder: string;
}

/**
 * Type guard to check if data is in simplified format (array of SimplifiedNode)
 */
export function isSimplifiedFormat(data: unknown): data is SimplifiedNode[] {
  if (!Array.isArray(data)) return false;
  if (data.length === 0) return true;  // Empty array could be either, default to simplified
  
  const firstItem = data[0];
  // Check for simplified format markers: has 'identifier' and 'deps' fields
  return (
    typeof firstItem === 'object' &&
    firstItem !== null &&
    'identifier' in firstItem &&
    'deps' in firstItem &&
    !('id' in firstItem)  // Make sure it's not a D3Node that happens to have deps
  );
}

/**
 * Type guard to check if data is in D3Graph format
 */
export function isD3GraphFormat(data: unknown): data is D3Graph {
  return (
    typeof data === 'object' &&
    data !== null &&
    'nodes' in data &&
    Array.isArray((data as any).nodes)
  );
}

/** Verus function mode */
export type FunctionMode = 'exec' | 'proof' | 'spec';

/** Verification status from Verus verification results */
export type VerificationStatus = 'verified' | 'failed' | 'unverified';

export interface D3Node {
  id: string;
  display_name: string;
  symbol: string;
  full_path: string;
  relative_path: string;
  file_name: string;
  parent_folder: string;
  // Note: body removed - use start_line/end_line to fetch code on demand
  start_line?: number;
  end_line?: number;
  is_libsignal: boolean;
  // Pre-computed for O(1) lookups in browser
  dependencies: string[];   // scip_names of functions this calls (outgoing)
  dependents: string[];     // scip_names of functions that call this (incoming)
  similar_lemmas?: SimilarLemma[];
  mode: FunctionMode;  // Verus function mode: exec, proof, or spec
  verification_status?: VerificationStatus;  // Verification status: verified, failed, unverified
  // D3-specific properties added during simulation
  x?: number;
  y?: number;
  vx?: number;
  vy?: number;
  fx?: number | null;
  fy?: number | null;
}

/** The type of a call/dependency link */
export type LinkType = 'inner' | 'precondition' | 'postcondition';

export interface D3Link {
  source: string | D3Node;
  target: string | D3Node;
  type: LinkType | string;  // 'inner' | 'precondition' | 'postcondition' (or legacy 'calls')
}

export interface D3GraphMetadata {
  total_nodes: number;
  total_edges: number;
  project_root: string;
  generated_at: string;
  github_url?: string;
}

export interface D3Graph {
  nodes: D3Node[];
  links: D3Link[];
  metadata: D3GraphMetadata;
}

export interface FilterOptions {
  showLibsignal: boolean;
  showNonLibsignal: boolean;
  showInnerCalls: boolean;         // Show calls from function body (default: true)
  showPreconditionCalls: boolean;  // Show calls from requires clauses (default: false)
  showPostconditionCalls: boolean; // Show calls from ensures clauses (default: false)
  // Function mode filters (Verus)
  showExecFunctions: boolean;      // Show executable functions (default: true)
  showProofFunctions: boolean;     // Show proof functions/lemmas (default: true)
  showSpecFunctions: boolean;      // Show spec functions (default: false)
  // Pattern-based exclusion (comma-separated glob patterns)
  excludeNamePatterns: string;     // Matches display_name, e.g., "*_comm*, lemma_mul_*"
  excludePathPatterns: string;     // Matches node ID path, e.g., "*/specs/*, */common_lemmas/*"
  // File-based inclusion (comma-separated file names or glob patterns)
  includeFiles: string;            // e.g., "edwards.rs, decompress*.rs" - empty means all files
  maxDepth: number | null;
  sourceQuery: string;  // Source node(s) - shows what they call (callees direction)
  sinkQuery: string;    // Sink node(s) - shows who calls them (callers direction)
  // When both source and sink are set, shows paths between them
  selectedNodes: Set<string>;
  expandedNodes: Set<string>;
  hiddenNodes: Set<string>;  // Nodes hidden by user (Shift+click)
  focusNodeIds: Set<string>;  // When non-empty, restricts initial view to these node IDs (loaded via ?focus= URL param)
}

export interface GraphState {
  fullGraph: D3Graph | null;
  filteredGraph: D3Graph | null;
  filters: FilterOptions;
  selectedNode: D3Node | null;
  hoveredNode: D3Node | null;
}

