import type { FilterOptions, ProjectLanguage } from '../types';

// ============================================================================
// Graph Summary (static analysis output)
// ============================================================================

export interface CrateSummary {
  name: string;
  nodeCount: number;
  fileCount: number;
  isExternal: boolean;
}

export interface VerificationBreakdown {
  verified: number;
  failed: number;
  unverified: number;
}

export interface KindBreakdown {
  [kind: string]: number;
}

export interface NodeRank {
  id: string;
  displayName: string;
  crateName: string;
  kind: string;
  verificationStatus: string | undefined;
  dependentCount: number;
  dependencyCount: number;
}

export interface SuggestedQuery {
  label: string;
  description: string;
  action: SuggestedAction;
}

export type SuggestedAction =
  | { type: 'setSource'; query: string }
  | { type: 'setSink'; query: string }
  | { type: 'setSourceAndSink'; source: string; sink: string }
  | { type: 'filterVerification'; statuses: ('verified' | 'failed' | 'unverified')[] }
  | { type: 'setCrateBoundary'; source: string; target: string }
  | { type: 'switchView'; view: string };

export interface GraphSummary {
  projectLanguage: ProjectLanguage;
  totalNodes: number;
  totalEdges: number;
  crates: CrateSummary[];
  files: string[];
  verification: VerificationBreakdown;
  kinds: KindBreakdown;
  topConnected: NodeRank[];
  unverifiedHotspots: NodeRank[];
  failedNodes: NodeRank[];
  suggestedQueries: SuggestedQuery[];
}

// ============================================================================
// Viewer actions the guide panel can trigger
// ============================================================================

export interface GuideActions {
  setFilters(updates: Partial<FilterOptions>): void;
  setSource(query: string): void;
  setSink(query: string): void;
  setDepth(depth: number | null): void;
  switchView(view: string): void;
  applyFiltersAndUpdate(): void;
}
