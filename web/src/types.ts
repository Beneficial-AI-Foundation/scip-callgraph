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

/** Declaration kind (Verus: exec/proof/spec, Lean: theorem/def/axiom/...) */
export type DeclKind = string;

/** Detected project language based on kind values in the graph */
export type ProjectLanguage = 'verus' | 'lean' | 'unknown';

const VERUS_KINDS = new Set(['exec', 'proof', 'spec']);
const LEAN_KINDS = new Set([
  'theorem', 'def', 'abbrev', 'class', 'structure',
  'inductive', 'instance', 'axiom', 'opaque', 'quot',
]);

const SOURCE_LANGUAGE_MAP: Record<string, ProjectLanguage> = {
  rust: 'verus',
  verus: 'verus',
  lean: 'lean',
};

/**
 * Detect the project language for a graph.
 *
 * Uses the explicit `metadata.source_language` field when available (set from
 * the Schema 2.0 envelope or individual atom `language` fields), mapping
 * "rust" -> "verus" and "lean" -> "lean".
 *
 * Falls back to the heuristic of scanning node `kind` values when no explicit
 * language is present (legacy / D3Graph format files).
 */
export function detectProjectLanguage(graph: D3Graph): ProjectLanguage {
  const explicit = graph.metadata?.source_language;
  if (explicit && explicit in SOURCE_LANGUAGE_MAP) {
    return SOURCE_LANGUAGE_MAP[explicit];
  }

  const kinds = new Set<string>();
  for (const node of graph.nodes) {
    if (node.kind) kinds.add(node.kind);
  }
  const hasVerus = [...kinds].some(k => VERUS_KINDS.has(k));
  const hasLean = [...kinds].some(k => LEAN_KINDS.has(k) && !VERUS_KINDS.has(k));
  if (hasVerus && !hasLean) return 'verus';
  if (hasLean && !hasVerus) return 'lean';
  return 'unknown';
}

/**
 * Get the proof-category and spec-category kind sets for a given language.
 * Everything not in either set is treated as exec/definitions.
 */
export function getKindSetsForLanguage(lang: ProjectLanguage): {
  proofKinds: Set<string>;
  specKinds: Set<string>;
} {
  switch (lang) {
    case 'verus':
      return { proofKinds: new Set(['proof']), specKinds: new Set(['spec']) };
    case 'lean':
      return { proofKinds: new Set(['theorem']), specKinds: new Set(['axiom']) };
    default:
      return { proofKinds: new Set(['proof', 'theorem']), specKinds: new Set(['spec', 'axiom']) };
  }
}

/** Verification status from Verus verification results */
export type VerificationStatus = 'verified' | 'failed' | 'unverified';

/** Derived border status (this node's readiness to be verified) */
export type BorderStatus = 'verified' | 'ready' | 'blocked' | 'not_ready' | 'unknown';

/** Derived fill status (subtree completeness) */
export type FillStatus = 'fully_verified' | 'verified' | 'ready' | 'none';

export interface D3Node {
  id: string;
  display_name: string;
  symbol: string;
  full_path: string;
  relative_path: string;
  file_name: string;
  parent_folder: string;
  crate_name: string;
  // Note: body removed - use start_line/end_line to fetch code on demand
  start_line?: number;
  end_line?: number;
  is_libsignal: boolean;
  // Pre-computed for O(1) lookups in browser
  dependencies: string[];   // scip_names of functions this calls (outgoing)
  dependents: string[];     // scip_names of functions that call this (incoming)
  similar_lemmas?: SimilarLemma[];
  kind: DeclKind;  // Declaration kind: exec, proof, spec (Verus) or theorem, def, axiom, ... (Lean)
  verification_status?: VerificationStatus;  // Verification status: verified, failed, unverified
  // Derived statuses computed by DAG walk (used by Blueprint view)
  border_status?: BorderStatus;
  fill_status?: FillStatus;
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
  source_language?: string;
}

export interface D3Graph {
  nodes: D3Node[];
  links: D3Link[];
  metadata: D3GraphMetadata;
  nodeDepths?: Map<string, number>;
}

export interface FilterOptions {
  showLibsignal: boolean;
  showNonLibsignal: boolean;
  showInnerCalls: boolean;         // Show calls from function body (default: true)
  showPreconditionCalls: boolean;  // Show calls from requires clauses (default: false)
  showPostconditionCalls: boolean; // Show calls from ensures clauses (default: false)
  // Declaration kind filters
  showExecFunctions: boolean;      // Show exec/def/class/structure/... (default: true)
  showProofFunctions: boolean;     // Show proof/theorem (default: true)
  showSpecFunctions: boolean;      // Show spec/axiom (default: false)
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

// ============================================================================
// Probe Atom Dict Format (shared by probe-verus and probe-lean)
// ============================================================================

/**
 * Atom format used in probe-verus and probe-lean atoms.json output.
 * This is a dict keyed by atom name (e.g., "probe:double_zero").
 */
export interface ProbeAtom {
  "display-name": string;
  dependencies: string[];
  "code-text": { "lines-start": number; "lines-end": number } | null;
  "code-path": string;
  "code-module": string;
  kind: string;
  language?: string;
  "verification-status"?: string;
  "specified"?: boolean;
  "dependencies-with-locations"?: Array<{
    "code-name": string;
    location: string;
    line: number;
  }>;
}

/**
 * Unwrap a Schema 2.0 metadata envelope if present.
 *
 * probe-verus and probe-lean 2.0 wrap their JSON output in an envelope with
 * `schema` (containing '/'), `tool`, `source`, `timestamp`, and `data` fields.
 * This function extracts the `data` payload. If the input is not an envelope
 * (bare dict, array, etc.) it is returned unchanged.
 */
export function unwrapEnvelope(data: unknown): unknown {
  if (
    typeof data === 'object' && data !== null && !Array.isArray(data) &&
    'schema' in data && 'data' in data &&
    typeof (data as any).schema === 'string' &&
    (data as any).schema.includes('/')
  ) {
    return (data as any).data;
  }
  return data;
}

/**
 * Extract the source language from a Schema 2.0 envelope before unwrapping.
 *
 * Returns `source.language` (e.g. "rust", "lean") if the input is an envelope,
 * or undefined if it is not.
 */
export function extractEnvelopeLanguage(data: unknown): string | undefined {
  if (
    typeof data === 'object' && data !== null && !Array.isArray(data) &&
    'schema' in data && 'source' in data &&
    typeof (data as any).schema === 'string' &&
    (data as any).schema.includes('/')
  ) {
    const source = (data as any).source;
    if (typeof source === 'object' && source !== null && typeof source.language === 'string') {
      return source.language;
    }
  }
  return undefined;
}

/**
 * Type guard to check if data is in probe atom dict format
 * (object keyed by atom name, where values have 'dependencies' and 'display-name')
 */
export function isAtomDictFormat(data: unknown): data is Record<string, ProbeAtom> {
  if (typeof data !== 'object' || data === null || Array.isArray(data)) return false;
  if ('nodes' in data || 'links' in data) return false;
  const values = Object.values(data);
  if (values.length === 0) return false;
  const first = values[0] as any;
  return typeof first === 'object' && first !== null
    && 'dependencies' in first && 'display-name' in first;
}

export interface GraphState {
  fullGraph: D3Graph | null;
  filteredGraph: D3Graph | null;
  filters: FilterOptions;
  selectedNode: D3Node | null;
  hoveredNode: D3Node | null;
  projectLanguage: ProjectLanguage;
}

// ============================================================================
// Crate Map Types
// ============================================================================

export interface CrateNode {
  name: string;
  functionCount: number;
  fileCount: number;
  nodeIds: string[];
  isExternal: boolean;
}

export interface CrateEdge {
  source: string;
  target: string;
  callCount: number;
  calls: Array<{ sourceId: string; targetId: string; type: string }>;
}

export interface CrateGraph {
  nodes: CrateNode[];
  edges: CrateEdge[];
}

/**
 * Extract module/crate name from a D3Node based on its ID format.
 *
 * For Rust/Verus (or unknown): returns the top-level crate name (first path segment).
 * For Lean: returns the first two path segments from relative_path (e.g. "ArkLib/Data")
 * to give a more granular module hierarchy. Falls back to one segment if only one exists.
 */
export function extractCrateName(
  node: Pick<D3Node, 'id' | 'relative_path'>,
  language: ProjectLanguage = 'unknown',
): string {
  if (language === 'lean' && node.relative_path) {
    const parts = node.relative_path.split('/');
    if (parts.length >= 2) return `${parts[0]}/${parts[1]}`;
    return parts[0] || 'unknown';
  }

  // scip:crate_name/version/... → crate_name
  if (node.id.startsWith('scip:')) {
    const afterPrefix = node.id.slice(5);
    const slashIdx = afterPrefix.indexOf('/');
    return slashIdx > 0 ? afterPrefix.slice(0, slashIdx) : afterPrefix;
  }
  // probe:crate_name/... → crate_name (works for both internal and external atoms)
  if (node.id.startsWith('probe:')) {
    const afterPrefix = node.id.slice(6);
    const slashIdx = afterPrefix.indexOf('/');
    return slashIdx > 0 ? afterPrefix.slice(0, slashIdx) : afterPrefix;
  }
  if (node.relative_path) {
    const firstSlash = node.relative_path.indexOf('/');
    return firstSlash > 0 ? node.relative_path.slice(0, firstSlash) : node.relative_path;
  }
  return 'unknown';
}

