import type { D3Graph, D3Node, FilterOptions, ProjectLanguage } from '../types';

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
// View Context (current state snapshot)
// ============================================================================

export interface ViewContext {
  activeView: string;
  filters: FilterOptions;
  displayedNodeCount: number;
  displayedEdgeCount: number;
  selectedNode: {
    id: string;
    displayName: string;
    kind: string;
    verificationStatus: string | undefined;
    callerCount: number;
    calleeCount: number;
    crateName: string;
    filePath: string;
  } | null;
}

// ============================================================================
// Chat Types
// ============================================================================

export interface ChatMessage {
  id: string;
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  toolCalls?: ToolCall[];
  toolCallId?: string;
  timestamp: number;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export interface ToolResult {
  success: boolean;
  matchedNodes?: number;
  data?: unknown;
  suggestion?: string;
  error?: string;
}

export interface ToolDef {
  name: string;
  description: string;
  parameters: {
    type: 'object';
    properties: Record<string, {
      type: string;
      description: string;
      enum?: string[];
      items?: { type: string };
    }>;
    required?: string[];
  };
}

// ============================================================================
// LLM Provider Interface
// ============================================================================

export interface ChatResponse {
  content: string;
  toolCalls?: ToolCall[];
  finishReason: 'stop' | 'tool_calls' | 'length' | 'error';
  usage?: {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
  };
}

export interface LLMProvider {
  id: string;
  name: string;
  available(): Promise<boolean>;
  chat(
    messages: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string; name?: string }>,
    tools: ToolDef[],
  ): Promise<ChatResponse>;
}

// ============================================================================
// State accessor for tool bridge
// ============================================================================

export interface ViewerStateAccessor {
  getFullGraph(): D3Graph | null;
  getFilteredGraph(): D3Graph | null;
  getFilters(): FilterOptions;
  getSelectedNode(): D3Node | null;
  getProjectLanguage(): ProjectLanguage;
  getActiveView(): string;
  setFilters(updates: Partial<FilterOptions>): void;
  setSource(query: string): void;
  setSink(query: string): void;
  setDepth(depth: number | null): void;
  switchView(view: string): void;
  selectNodeByName(name: string): boolean;
  resetFilters(): void;
  applyFiltersAndUpdate(): void;
}
