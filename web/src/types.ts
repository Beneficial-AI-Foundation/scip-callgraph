// Type definitions matching the Rust D3Graph structure

export interface SimilarLemma {
  name: string;
  score: number;
  file_path: string;
  line_number: number | null;
  signature: string;
  source: string;  // "project" or "vstd"
}

export interface D3Node {
  id: string;
  display_name: string;
  symbol: string;
  full_path: string;
  relative_path: string;
  file_name: string;
  parent_folder: string;
  body?: string;
  start_line?: number;
  end_line?: number;
  is_libsignal: boolean;
  caller_count: number;
  callee_count: number;
  similar_lemmas?: SimilarLemma[];
  // D3-specific properties added during simulation
  x?: number;
  y?: number;
  vx?: number;
  vy?: number;
  fx?: number | null;
  fy?: number | null;
}

export interface D3Link {
  source: string | D3Node;
  target: string | D3Node;
  type: string;
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
  maxDepth: number | null;
  sourceQuery: string;  // Source node(s) - shows what they call (callees direction)
  sinkQuery: string;    // Sink node(s) - shows who calls them (callers direction)
  // When both source and sink are set, shows paths between them
  selectedNodes: Set<string>;
  expandedNodes: Set<string>;
  hiddenNodes: Set<string>;  // Nodes hidden by user (Shift+click)
}

export interface GraphState {
  fullGraph: D3Graph | null;
  filteredGraph: D3Graph | null;
  filters: FilterOptions;
  selectedNode: D3Node | null;
  hoveredNode: D3Node | null;
}

