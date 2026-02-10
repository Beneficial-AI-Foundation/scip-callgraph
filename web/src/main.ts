import { D3Graph, D3Node, D3Link, GraphState, FilterOptions, SimplifiedNode, isSimplifiedFormat, isD3GraphFormat } from './types';
import { applyFilters, getCallers, getCallees, SelectedNodeOptions } from './filters';
import { CallGraphVisualization } from './graph';

// ============================================================================
// JSON Format Conversion
// ============================================================================

/**
 * Convert simplified JSON format (array of nodes with deps) to D3Graph format.
 * This handles JSON files like curve25519-dalek.json that have a simpler structure.
 * 
 * The simplified format:
 * - Is a flat array of nodes (no nodes/links/metadata wrapper)
 * - Uses 'identifier' instead of 'id'
 * - Uses 'deps' instead of 'dependencies'
 * - May have 'body' (code) instead of start_line/end_line
 * - Missing fields: symbol, is_libsignal, dependents, mode, links
 * 
 * This conversion:
 * - Maps field names to D3Graph format
 * - Computes 'dependents' as inverse of 'deps'
 * - Generates links from deps
 * - Creates minimal metadata
 * - Handles file:// prefix in full_path
 */
function convertSimplifiedToD3Graph(nodes: SimplifiedNode[]): D3Graph {
  // Build a set of known node IDs for filtering external dependencies
  const knownIds = new Set(nodes.map(n => n.identifier));
  
  // Build dependents map (inverse of deps)
  const dependentsMap = new Map<string, string[]>();
  for (const node of nodes) {
    // Initialize empty array for each node
    if (!dependentsMap.has(node.identifier)) {
      dependentsMap.set(node.identifier, []);
    }
    
    // For each dep, add this node as a dependent
    for (const dep of node.deps) {
      if (!dependentsMap.has(dep)) {
        dependentsMap.set(dep, []);
      }
      dependentsMap.get(dep)!.push(node.identifier);
    }
  }
  
  // Convert nodes
  const d3Nodes: D3Node[] = nodes.map(node => {
    // Clean up full_path (remove file:// prefix if present)
    let fullPath = node.full_path;
    if (fullPath.startsWith('file://')) {
      fullPath = fullPath.substring(7);  // Remove 'file://'
    }
    
    // Filter dependencies to only include known nodes
    const filteredDeps = node.deps.filter(dep => knownIds.has(dep));
    
    // Get dependents (inverse relationship)
    const dependents = dependentsMap.get(node.identifier) || [];
    
    return {
      id: node.identifier,
      display_name: node.display_name,
      symbol: node.identifier,  // Use identifier as symbol (no separate symbol in simplified format)
      full_path: fullPath,
      relative_path: node.relative_path,
      file_name: node.file_name,
      parent_folder: node.parent_folder,
      // No line numbers in simplified format
      start_line: undefined,
      end_line: undefined,
      is_libsignal: false,  // Default to false
      dependencies: filteredDeps,
      dependents: dependents.filter(dep => knownIds.has(dep)),  // Also filter dependents
      mode: 'exec' as const,  // Default mode (could try to infer from body/statement_type)
    };
  });
  
  // Generate links from dependencies
  const links: D3Link[] = [];
  for (const node of d3Nodes) {
    for (const dep of node.dependencies) {
      links.push({
        source: node.id,
        target: dep,
        type: 'inner',  // Default link type
      });
    }
  }
  
  // Create minimal metadata
  const metadata = {
    total_nodes: d3Nodes.length,
    total_edges: links.length,
    project_root: 'Simplified JSON (no project root)',
    generated_at: new Date().toISOString(),
    // No github_url available in simplified format
  };
  
  return {
    nodes: d3Nodes,
    links,
    metadata,
  };
}

/**
 * Parse JSON data and convert to D3Graph format if needed.
 * Supports both D3Graph format and simplified format (curve25519-dalek.json style).
 * 
 * @param data - Parsed JSON data (either D3Graph or SimplifiedNode[])
 * @returns D3Graph in the expected format
 */
function parseAndNormalizeGraph(data: unknown): D3Graph {
  // Check if it's already in D3Graph format
  if (isD3GraphFormat(data)) {
    console.log('Detected D3Graph format');
    return data;
  }
  
  // Check if it's in simplified format
  if (isSimplifiedFormat(data)) {
    console.log('Detected simplified format, converting to D3Graph');
    return convertSimplifiedToD3Graph(data);
  }
  
  // Unknown format - try to treat as D3Graph and hope for the best
  console.warn('Unknown JSON format, attempting to use as D3Graph');
  return data as D3Graph;
}

// ============================================================================
// VS Code Integration
// ============================================================================

/**
 * VS Code API interface for webview communication
 */
interface VSCodeAPI {
  postMessage(message: any): void;
  getState(): any;
  setState(state: any): void;
}

/**
 * Check if running inside VS Code webview
 */
function isVSCodeEnvironment(): boolean {
  return typeof (window as any).acquireVsCodeApi === 'function';
}

/**
 * Get VS Code API if available
 */
let vscodeApi: VSCodeAPI | null = null;
function getVSCodeAPI(): VSCodeAPI | null {
  if (vscodeApi) return vscodeApi;
  if (isVSCodeEnvironment()) {
    vscodeApi = (window as any).acquireVsCodeApi();
    return vscodeApi;
  }
  return null;
}

/**
 * Send a message to the VS Code extension
 */
function postMessageToExtension(message: any): void {
  const api = getVSCodeAPI();
  if (api) {
    api.postMessage(message);
  }
}

/**
 * Navigate to a file in VS Code (or GitHub in web mode)
 */
function navigateToSource(node: D3Node): void {
  const api = getVSCodeAPI();
  if (api) {
    // In VS Code: send message to extension to open the file
    api.postMessage({
      type: 'navigate',
      relativePath: node.relative_path,
      startLine: node.start_line,
      endLine: node.end_line,
      displayName: node.display_name
    });
  } else {
    // In web mode: open GitHub link if available
    const githubLink = buildGitHubLink(node);
    if (githubLink) {
      window.open(githubLink, '_blank');
    }
  }
}

// GitHub URL for source code links (configurable via env var, URL param, or graph metadata)
// Priority: URL param > graph metadata > env var
let githubBaseUrl: string | null = import.meta.env.VITE_GITHUB_URL || null;

// Path prefix to prepend to relative_path when building GitHub links
// (e.g., "curve25519-dalek" if repo structure is repo/curve25519-dalek/src/...)
let githubPathPrefix: string = import.meta.env.VITE_GITHUB_PATH_PREFIX || '';

// Performance threshold: if graph has more links than this, start with empty view
// to avoid browser freeze. User must filter first.
const LARGE_GRAPH_LINK_THRESHOLD = 10000;

// File size threshold (in bytes) - files larger than this won't auto-load
// 5MB is roughly the size where JSON.parse starts to noticeably freeze the browser
const LARGE_FILE_SIZE_THRESHOLD = 5 * 1024 * 1024;

// Track if we're deferring load due to large file
let deferredGraphUrl: string | null = null;

// Prevent multiple simultaneous deferred graph loads
let isDeferredLoadInProgress = false;

// Focus set URL (from ?focus= URL parameter)
let focusJsonUrl: string | null = null;

// Debounce timer for search inputs
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;
const SEARCH_DEBOUNCE_MS = 300; // Wait 300ms after user stops typing

// VS Code: Selected node ID for exact matching
// When set, filters will match only this exact node instead of all nodes with the same display name
let selectedNodeId: string | null = null;

/**
 * Debounced version of applyFiltersAndUpdate for large graphs.
 * Prevents UI freeze from filtering on every keystroke.
 */
function debouncedApplyFilters(): void {
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer);
  }
  searchDebounceTimer = setTimeout(() => {
    applyFiltersAndUpdate();
  }, SEARCH_DEBOUNCE_MS);
}

/**
 * Show a message in the stats panel for large graphs that need filtering
 */
function showLargeGraphPrompt(fileSize: number): void {
  const statsDiv = document.getElementById('stats');
  if (statsDiv) {
    const sizeMB = (fileSize / (1024 * 1024)).toFixed(1);
    statsDiv.innerHTML = `
      <div style="background: #fff3e0; padding: 12px; border-radius: 4px; margin-bottom: 8px;">
        <div style="color: #e65100; font-weight: bold; margin-bottom: 8px;">📊 Large Graph Detected (${sizeMB} MB)</div>
        <p style="margin: 0 0 8px 0; font-size: 0.9rem; color: #333;">
          Enter a <strong>Source</strong>, <strong>Sink</strong>, or <strong>Include Files</strong> filter, then press <strong>Enter</strong> or click <strong>Load & Search</strong>.
        </p>
        <button id="load-graph-btn" style="background: #1976d2; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-size: 0.9rem;">
          Load & Search
        </button>
      </div>
    `;
    
    // Add click handler for the button
    document.getElementById('load-graph-btn')?.addEventListener('click', () => {
      loadDeferredGraph();
    });
  }
}


/**
 * Build a GitHub link to the source code
 */
function buildGitHubLink(node: D3Node): string | null {
  if (!githubBaseUrl) return null;
  
  // Clean up the base URL (remove trailing slash)
  const baseUrl = githubBaseUrl.replace(/\/$/, '');
  
  // Clean up path prefix (remove leading/trailing slashes)
  const prefix = githubPathPrefix.replace(/^\/|\/$/g, '');
  
  // Build the full path, but avoid duplicating if relative_path already starts with prefix
  let fullPath: string;
  if (prefix) {
    // Check if relative_path already starts with the prefix (avoid duplication)
    if (node.relative_path.startsWith(prefix + '/') || node.relative_path === prefix) {
      fullPath = node.relative_path;
    } else {
      fullPath = `${prefix}/${node.relative_path}`;
    }
  } else {
    fullPath = node.relative_path;
  }
  
  // Build the link with line numbers if available
  let link = `${baseUrl}/blob/main/${fullPath}`;
  
  if (node.start_line) {
    link += `#L${node.start_line}`;
    // Only add end line if it's greater than start line (sanity check)
    if (node.end_line && node.end_line > node.start_line) {
      link += `-L${node.end_line}`;
    }
  }
  
  return link;
}

/**
 * Parse filter state from URL parameters
 */
function parseFiltersFromURL(): Partial<FilterOptions> {
  const params = new URLSearchParams(window.location.search);
  const filters: Partial<FilterOptions> = {};
  
  // String params
  if (params.has('source')) filters.sourceQuery = params.get('source')!;
  if (params.has('sink')) filters.sinkQuery = params.get('sink')!;
  if (params.has('files')) filters.includeFiles = params.get('files')!;
  
  // Number params
  if (params.has('depth')) {
    const depth = parseInt(params.get('depth')!);
    filters.maxDepth = isNaN(depth) || depth === 0 ? null : depth;
  }
  
  // Boolean params (1/0 or true/false)
  const parseBool = (key: string): boolean | undefined => {
    if (!params.has(key)) return undefined;
    const val = params.get(key)!.toLowerCase();
    return val === '1' || val === 'true';
  };
  
  const exec = parseBool('exec');
  const proof = parseBool('proof');
  const spec = parseBool('spec');
  const inner = parseBool('inner');
  const pre = parseBool('pre');
  const post = parseBool('post');
  const libsignal = parseBool('libsignal');
  const external = parseBool('external');
  
  if (exec !== undefined) filters.showExecFunctions = exec;
  if (proof !== undefined) filters.showProofFunctions = proof;
  if (spec !== undefined) filters.showSpecFunctions = spec;
  if (inner !== undefined) filters.showInnerCalls = inner;
  if (pre !== undefined) filters.showPreconditionCalls = pre;
  if (post !== undefined) filters.showPostconditionCalls = post;
  if (libsignal !== undefined) filters.showLibsignal = libsignal;
  if (external !== undefined) filters.showNonLibsignal = external;
  
  // Exclude patterns from URL
  if (params.has('excludeName')) {
    filters.excludeNamePatterns = params.get('excludeName')!;
  }
  if (params.has('excludePath')) {
    filters.excludePathPatterns = params.get('excludePath')!;
  }
  
  // Hidden nodes (comma-separated display names, since full IDs are too long)
  if (params.has('hidden')) {
    const hiddenNames = params.get('hidden')!.split(',').map(s => s.trim()).filter(s => s);
    // We'll need to resolve these to IDs after graph loads
    (filters as any)._hiddenNames = hiddenNames;
  }
  
  // Focus set URL (stored separately, fetched after graph loads)
  if (params.has('focus')) {
    focusJsonUrl = params.get('focus')!;
  }
  
  return filters;
}

/**
 * Generate a shareable URL with current filter state
 */
function generateShareableURL(): string {
  const url = new URL(window.location.href);
  const params = url.searchParams;
  
  // Clear existing filter params (keep json, github, etc.)
  ['source', 'sink', 'exclude', 'files', 'depth', 'exec', 'proof', 'spec', 
   'inner', 'pre', 'post', 'libsignal', 'external', 'hidden', 'focus'].forEach(k => params.delete(k));
  
  // Add current filter state
  if (state.filters.sourceQuery) params.set('source', state.filters.sourceQuery);
  if (state.filters.sinkQuery) params.set('sink', state.filters.sinkQuery);
  if (state.filters.includeFiles) params.set('files', state.filters.includeFiles);
  if (state.filters.maxDepth !== null) params.set('depth', state.filters.maxDepth.toString());
  
  // Only include non-default boolean values to keep URL short
  if (!state.filters.showExecFunctions) params.set('exec', '0');
  if (!state.filters.showProofFunctions) params.set('proof', '0');
  if (state.filters.showSpecFunctions) params.set('spec', '1');
  if (!state.filters.showInnerCalls) params.set('inner', '0');
  if (state.filters.showPreconditionCalls) params.set('pre', '1');
  if (state.filters.showPostconditionCalls) params.set('post', '1');
  if (!state.filters.showLibsignal) params.set('libsignal', '0');
  if (!state.filters.showNonLibsignal) params.set('external', '0');
  // Exclude patterns
  if (state.filters.excludeNamePatterns) {
    params.set('excludeName', state.filters.excludeNamePatterns);
  }
  if (state.filters.excludePathPatterns) {
    params.set('excludePath', state.filters.excludePathPatterns);
  }
  
  // Hidden nodes - use display names (shorter than full IDs)
  if (state.filters.hiddenNodes.size > 0 && state.fullGraph) {
    const hiddenNames: string[] = [];
    state.fullGraph.nodes.forEach(node => {
      if (state.filters.hiddenNodes.has(node.id)) {
        hiddenNames.push(node.display_name);
      }
    });
    if (hiddenNames.length > 0) {
      params.set('hidden', hiddenNames.join(','));
    }
  }
  
  // Focus set URL
  if (focusJsonUrl && state.filters.focusNodeIds.size > 0) {
    params.set('focus', focusJsonUrl);
  }
  
  return url.toString();
}

/**
 * Apply URL filter params to state and sync UI
 */
function applyURLFiltersToState(urlFilters: Partial<FilterOptions>): void {
  // Merge URL filters with current state
  Object.assign(state.filters, urlFilters);
  
  // Sync UI elements to match
  const setInput = (id: string, value: string) => {
    const el = document.getElementById(id) as HTMLInputElement;
    if (el) el.value = value;
  };
  const setCheckbox = (id: string, checked: boolean) => {
    const el = document.getElementById(id) as HTMLInputElement;
    if (el) el.checked = checked;
  };
  
  setInput('source-input', state.filters.sourceQuery);
  setInput('sink-input', state.filters.sinkQuery);
  setInput('exclude-name-patterns', state.filters.excludeNamePatterns);
  setInput('exclude-path-patterns', state.filters.excludePathPatterns);
  setInput('include-files', state.filters.includeFiles);
  
  // Update file list selection to match
  updateFileListSelection();
  
  const depthEl = document.getElementById('depth-limit') as HTMLInputElement;
  if (depthEl) {
    depthEl.value = state.filters.maxDepth?.toString() || '0';
    document.getElementById('depth-value')!.textContent = 
      state.filters.maxDepth !== null ? state.filters.maxDepth.toString() : 'All';
  }
  
  setCheckbox('show-exec-functions', state.filters.showExecFunctions);
  setCheckbox('show-proof-functions', state.filters.showProofFunctions);
  setCheckbox('show-spec-functions', state.filters.showSpecFunctions);
  setCheckbox('show-inner-calls', state.filters.showInnerCalls);
  setCheckbox('show-precondition-calls', state.filters.showPreconditionCalls);
  setCheckbox('show-postcondition-calls', state.filters.showPostconditionCalls);
  setCheckbox('show-libsignal', state.filters.showLibsignal);
  setCheckbox('show-non-libsignal', state.filters.showNonLibsignal);
}

/**
 * Resolve hidden node names to IDs (called after graph loads)
 */
function resolveHiddenNodeNames(hiddenNames: string[]): void {
  if (!state.fullGraph || hiddenNames.length === 0) return;
  
  for (const name of hiddenNames) {
    const node = state.fullGraph.nodes.find(n => n.display_name === name);
    if (node) {
      state.filters.hiddenNodes.add(node.id);
    }
  }
  updateHiddenNodesUI();
}

// Initialize state
const initialFilters: FilterOptions = {
  showLibsignal: true,
  showNonLibsignal: true,
  showInnerCalls: true,           // Show body calls by default
  showPreconditionCalls: false,   // Hide requires calls by default
  showPostconditionCalls: false,  // Hide ensures calls by default
  showExecFunctions: true,        // Show exec functions by default
  showProofFunctions: true,       // Show proof functions by default
  showSpecFunctions: false,       // Hide spec functions by default
  excludeNamePatterns: '',        // Exclude by function name (e.g., *_comm*)
  excludePathPatterns: '',        // Exclude by path (e.g., */specs/*)
  includeFiles: '',               // Comma-separated file patterns to include (empty = all)
  maxDepth: 1,
  sourceQuery: '',  // Source nodes - shows what they call (callees)
  sinkQuery: '',    // Sink nodes - shows who calls them (callers)
  selectedNodes: new Set(),
  expandedNodes: new Set(),
  hiddenNodes: new Set(),
  focusNodeIds: new Set(),        // Focus set: when non-empty, restricts view to these node IDs
};

let state: GraphState = {
  fullGraph: null,
  filteredGraph: null,
  filters: initialFilters,
  selectedNode: null,
  hoveredNode: null,
};

let visualization: CallGraphVisualization | null = null;

/**
 * Sync input field values to state (handles browser auto-fill after refresh)
 */
function syncInputsToState(): void {
  // Sync text inputs that might have been auto-filled by the browser
  const sourceInput = document.getElementById('source-input') as HTMLInputElement;
  const sinkInput = document.getElementById('sink-input') as HTMLInputElement;
  const excludeNameInput = document.getElementById('exclude-name-patterns') as HTMLInputElement;
  const excludePathInput = document.getElementById('exclude-path-patterns') as HTMLInputElement;
  const includeFilesInput = document.getElementById('include-files') as HTMLInputElement;
  const depthInput = document.getElementById('depth-limit') as HTMLInputElement;
  
  if (sourceInput?.value) {
    state.filters.sourceQuery = sourceInput.value;
  }
  if (sinkInput?.value) {
    state.filters.sinkQuery = sinkInput.value;
  }
  if (excludeNameInput?.value) {
    state.filters.excludeNamePatterns = excludeNameInput.value;
  }
  if (excludePathInput?.value) {
    state.filters.excludePathPatterns = excludePathInput.value;
  }
  if (includeFilesInput?.value) {
    state.filters.includeFiles = includeFilesInput.value;
  }
  if (depthInput?.value) {
    const value = parseInt(depthInput.value);
    state.filters.maxDepth = value > 0 ? value : null;
    document.getElementById('depth-value')!.textContent = 
      state.filters.maxDepth !== null ? state.filters.maxDepth.toString() : 'All';
  }
  
  // Sync checkboxes
  state.filters.showLibsignal = (document.getElementById('show-libsignal') as HTMLInputElement)?.checked ?? true;
  state.filters.showNonLibsignal = (document.getElementById('show-non-libsignal') as HTMLInputElement)?.checked ?? true;
  state.filters.showInnerCalls = (document.getElementById('show-inner-calls') as HTMLInputElement)?.checked ?? true;
  state.filters.showPreconditionCalls = (document.getElementById('show-precondition-calls') as HTMLInputElement)?.checked ?? false;
  state.filters.showPostconditionCalls = (document.getElementById('show-postcondition-calls') as HTMLInputElement)?.checked ?? false;
  state.filters.showExecFunctions = (document.getElementById('show-exec-functions') as HTMLInputElement)?.checked ?? true;
  state.filters.showProofFunctions = (document.getElementById('show-proof-functions') as HTMLInputElement)?.checked ?? true;
  state.filters.showSpecFunctions = (document.getElementById('show-spec-functions') as HTMLInputElement)?.checked ?? false;
}

/**
 * Initialize the application
 */
function init(): void {
  const graphContainer = document.getElementById('graph-container');
  if (!graphContainer) {
    console.error('Graph container not found');
    return;
  }

  // Initialize visualization
  visualization = new CallGraphVisualization(graphContainer, state, handleStateChange);

  // Set up UI event handlers
  setupUIHandlers();
  
  // Sync any auto-filled input values to state
  syncInputsToState();

  // Update stats display
  updateStats();
  updateNodeInfo();

  // Setup VS Code integration if running in webview
  setupVSCodeIntegration();

  // Try to auto-load graph.json if it exists (skipped in VS Code mode)
  if (!isVSCodeEnvironment()) {
    autoLoadGraph();
  }
}

/**
 * Set up UI event handlers
 */
function setupUIHandlers(): void {
  // File input
  const fileInput = document.getElementById('file-input') as HTMLInputElement;
  fileInput?.addEventListener('change', handleFileLoad);

  // Filter controls
  document.getElementById('show-libsignal')?.addEventListener('change', (e) => {
    state.filters.showLibsignal = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  document.getElementById('show-non-libsignal')?.addEventListener('change', (e) => {
    state.filters.showNonLibsignal = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  // Call type filters
  document.getElementById('show-inner-calls')?.addEventListener('change', (e) => {
    state.filters.showInnerCalls = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  document.getElementById('show-precondition-calls')?.addEventListener('change', (e) => {
    state.filters.showPreconditionCalls = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  document.getElementById('show-postcondition-calls')?.addEventListener('change', (e) => {
    state.filters.showPostconditionCalls = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  // Function mode filters (Verus)
  document.getElementById('show-exec-functions')?.addEventListener('change', (e) => {
    state.filters.showExecFunctions = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  document.getElementById('show-proof-functions')?.addEventListener('change', (e) => {
    state.filters.showProofFunctions = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  document.getElementById('show-spec-functions')?.addEventListener('change', (e) => {
    state.filters.showSpecFunctions = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  // Exclude name patterns input
  document.getElementById('exclude-name-patterns')?.addEventListener('input', (e) => {
    state.filters.excludeNamePatterns = (e.target as HTMLInputElement).value;
    if (state.fullGraph) {
      if (isLargeGraph(state.fullGraph)) {
        debouncedApplyFilters();
      } else {
        applyFiltersAndUpdate();
      }
    }
  });

  // Exclude path patterns input
  document.getElementById('exclude-path-patterns')?.addEventListener('input', (e) => {
    state.filters.excludePathPatterns = (e.target as HTMLInputElement).value;
    if (state.fullGraph) {
      if (isLargeGraph(state.fullGraph)) {
        debouncedApplyFilters();
      } else {
        applyFiltersAndUpdate();
      }
    }
  });

  // Exclude path presets dropdown - adds selected pattern to the exclude path patterns field
  document.getElementById('exclude-path-presets')?.addEventListener('change', (e) => {
    const select = e.target as HTMLSelectElement;
    const preset = select.value;
    if (preset) {
      const excludeInput = document.getElementById('exclude-path-patterns') as HTMLInputElement;
      const currentPatterns = excludeInput.value.trim();
      
      // Add preset patterns (avoid duplicates)
      const existingPatterns = currentPatterns ? currentPatterns.split(',').map(p => p.trim()) : [];
      const presetPatterns = preset.split(',').map(p => p.trim());
      
      for (const p of presetPatterns) {
        if (!existingPatterns.includes(p)) {
          existingPatterns.push(p);
        }
      }
      
      excludeInput.value = existingPatterns.join(', ');
      state.filters.excludePathPatterns = excludeInput.value;
      
      // Reset dropdown
      select.value = '';
      
      applyFiltersAndUpdate();
    }
  });

  document.getElementById('source-input')?.addEventListener('input', (e) => {
    state.filters.sourceQuery = (e.target as HTMLInputElement).value;
    // Clear exact node selection when user manually types (allows normal query matching)
    selectedNodeId = null;
    // Only auto-apply if graph is already loaded, use debounce for large graphs
    if (state.fullGraph) {
      if (isLargeGraph(state.fullGraph)) {
        debouncedApplyFilters();
      } else {
        applyFiltersAndUpdate();
      }
    }
  });
  
  // Handle Enter key to trigger immediate filter or deferred graph loading
  document.getElementById('source-input')?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (deferredGraphUrl) {
        loadDeferredGraph();
      } else if (state.fullGraph) {
        // Cancel debounce and apply immediately
        if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
        applyFiltersAndUpdate();
      }
    }
  });

  document.getElementById('sink-input')?.addEventListener('input', (e) => {
    state.filters.sinkQuery = (e.target as HTMLInputElement).value;
    // Clear exact node selection when user manually types (allows normal query matching)
    selectedNodeId = null;
    // Only auto-apply if graph is already loaded, use debounce for large graphs
    if (state.fullGraph) {
      if (isLargeGraph(state.fullGraph)) {
        debouncedApplyFilters();
      } else {
        applyFiltersAndUpdate();
      }
    }
  });
  
  // Handle Enter key to trigger immediate filter or deferred graph loading
  document.getElementById('sink-input')?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (deferredGraphUrl) {
        loadDeferredGraph();
      } else if (state.fullGraph) {
        // Cancel debounce and apply immediately
        if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
        applyFiltersAndUpdate();
      }
    }
  });

  document.getElementById('include-files')?.addEventListener('input', (e) => {
    state.filters.includeFiles = (e.target as HTMLInputElement).value;
    updateFileListSelection();  // Update file list checkmarks
    // Only auto-apply if graph is already loaded, use debounce for large graphs
    if (state.fullGraph) {
      if (isLargeGraph(state.fullGraph)) {
        debouncedApplyFilters();
      } else {
        applyFiltersAndUpdate();
      }
    }
  });
  
  // Handle Enter key to trigger immediate filter or deferred graph loading
  // Also check for ambiguous file patterns and show disambiguation dropdown
  document.getElementById('include-files')?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      console.log('Enter pressed in include-files. deferredGraphUrl:', deferredGraphUrl, 'state.fullGraph:', !!state.fullGraph);
      
      // Hide any existing dropdown first
      hideDisambiguationDropdown();
      
      if (deferredGraphUrl) {
        // Graph not loaded yet - load it first, then check for disambiguation
        console.log('Loading deferred graph...');
        loadDeferredGraphWithDisambiguation();
      } else if (state.fullGraph) {
        // Check for ambiguous patterns before applying
        const hasAmbiguity = checkAndShowDisambiguation();
        if (!hasAmbiguity) {
          // No ambiguity - apply filters immediately
          if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
          applyFiltersAndUpdate();
        }
        // If ambiguous, dropdown is shown and user will select
      } else {
        console.log('No graph loaded and no deferred URL');
      }
    } else if (e.key === 'Escape') {
      // Allow Escape to close the dropdown
      hideDisambiguationDropdown();
    }
  });
  
  // Close dropdown when clicking outside
  document.addEventListener('click', (e) => {
    const dropdown = document.getElementById('file-disambiguation-dropdown');
    const input = document.getElementById('include-files');
    if (dropdown && dropdown.style.display !== 'none') {
      const target = e.target as HTMLElement;
      if (!dropdown.contains(target) && target !== input) {
        hideDisambiguationDropdown();
      }
    }
  });

  document.getElementById('depth-limit')?.addEventListener('input', (e) => {
    const value = parseInt((e.target as HTMLInputElement).value);
    state.filters.maxDepth = value > 0 ? value : null;
    document.getElementById('depth-value')!.textContent = 
      state.filters.maxDepth !== null ? state.filters.maxDepth.toString() : 'All';
    applyFiltersAndUpdate();
  });

  document.getElementById('reset-filters')?.addEventListener('click', () => {
    resetFilters();
  });

  document.getElementById('clear-selection')?.addEventListener('click', () => {
    state.filters.selectedNodes.clear();
    state.selectedNode = null;
    applyFiltersAndUpdate();
  });

  document.getElementById('show-all-hidden')?.addEventListener('click', () => {
    showAllHiddenNodes();
  });

  // Copy link button
  document.getElementById('copy-link')?.addEventListener('click', () => {
    const url = generateShareableURL();
    navigator.clipboard.writeText(url).then(() => {
      // Show feedback
      const btn = document.getElementById('copy-link') as HTMLButtonElement;
      const originalText = btn.textContent;
      btn.textContent = '✓ Copied!';
      btn.style.background = '#4caf50';
      setTimeout(() => {
        btn.textContent = originalText;
        btn.style.background = '';
      }, 2000);
    }).catch(err => {
      console.error('Failed to copy link:', err);
      // Fallback: show URL in a prompt
      prompt('Copy this link:', url);
    });
  });

  // Window resize
  window.addEventListener('resize', handleResize);
}

/**
 * Auto-load graph from URL parameter, env var, or local graph.json
 * Priority: URL param > env var > local file
 */
async function autoLoadGraph(): Promise<void> {
  // Check for URL parameters (highest priority)
  const urlParams = new URLSearchParams(window.location.search);
  const jsonUrlParam = urlParams.get('json') || urlParams.get('url');
  
  // Check for GitHub URL parameter (overrides metadata and env var)
  const githubParam = urlParams.get('github');
  if (githubParam) {
    githubBaseUrl = githubParam;
  }
  
  // Check for GitHub path prefix parameter (e.g., "curve25519-dalek")
  const prefixParam = urlParams.get('github_prefix') || urlParams.get('prefix');
  if (prefixParam) {
    githubPathPrefix = prefixParam;
  }
  
  // Determine which JSON URL to use: URL param > env var > local file
  const jsonUrl = jsonUrlParam || import.meta.env.VITE_GRAPH_JSON_URL || null;
  
  if (jsonUrl) {
    try {
      console.log('Loading graph from URL:', jsonUrl);
      
      // Check file size first with HEAD request
      const headResponse = await fetch(jsonUrl, { method: 'HEAD' });
      const contentLength = parseInt(headResponse.headers.get('Content-Length') || '0');
      
      if (contentLength > LARGE_FILE_SIZE_THRESHOLD) {
        console.log(`Large file detected (${(contentLength / 1024 / 1024).toFixed(1)} MB), deferring load`);
        deferredGraphUrl = jsonUrl;
        showLargeGraphPrompt(contentLength);
        return;
      }
      
      const response = await fetch(jsonUrl);
      if (!response.ok) {
        throw new Error(`Failed to fetch: ${response.status} ${response.statusText}`);
      }

      const text = await response.text();
      const rawData = JSON.parse(text);
      const graph = parseAndNormalizeGraph(rawData);
      
      const source = jsonUrlParam ? 'URL parameter' : 'configured default';
      loadGraph(graph, `Loaded from ${source}: ${jsonUrl}`);
      return;
    } catch (error) {
      console.error('Failed to load graph from URL:', error);
      showError(`Failed to load graph from URL: ${error instanceof Error ? error.message : 'Unknown error'}`);
      // Continue to try local graph.json
    }
  }
  
  // Fall back to local graph.json
  try {
    // Check file size first with HEAD request
    const headResponse = await fetch('./graph.json', { method: 'HEAD' });
    const contentLength = parseInt(headResponse.headers.get('Content-Length') || '0');
    
    console.log(`graph.json size: ${(contentLength / 1024 / 1024).toFixed(1)} MB`);
    
    if (contentLength > LARGE_FILE_SIZE_THRESHOLD) {
      console.log(`Large file detected, deferring load until user searches`);
      deferredGraphUrl = './graph.json';
      showLargeGraphPrompt(contentLength);
      return;
    }
    
    // Use relative path to work with GitHub Pages base URL
    const response = await fetch('./graph.json');
    
    if (!response.ok) {
      console.log('No graph.json found in public directory. Waiting for manual file load.');
      return;
    }

    const text = await response.text();
    const rawData = JSON.parse(text);
    const graph = parseAndNormalizeGraph(rawData);
    
    loadGraph(graph, 'Auto-loaded from local file');
  } catch (error) {
    console.log('Could not auto-load graph.json:', error);
    // Silently fail - user can still manually load a file
  }
}

/**
 * Load a deferred graph (for large files that weren't auto-loaded)
 * Only called when user explicitly requests it after entering a search query
 */
async function loadDeferredGraph(): Promise<void> {
  if (!deferredGraphUrl) return;
  
  // Prevent multiple simultaneous loads
  if (isDeferredLoadInProgress) {
    console.log('Deferred graph load already in progress, skipping');
    return;
  }
  
  // Check if user has entered a search query
  if (!hasSearchFilters()) {
    showError('Please enter a Source, Sink, or Include Files filter first to filter the large graph.');
    return;
  }
  
  isDeferredLoadInProgress = true;
  
  const statsDiv = document.getElementById('stats');
  if (statsDiv) {
    statsDiv.innerHTML = `
      <div style="padding: 1rem; text-align: center;">
        <div style="margin-bottom: 0.5rem;">⏳ Loading and filtering graph...</div>
        <div style="font-size: 0.8rem; color: #666;">This may take a few seconds...</div>
      </div>
    `;
  }
  
  try {
    const response = await fetch(deferredGraphUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch: ${response.status}`);
    }
    
    const text = await response.text();
    const rawData = JSON.parse(text);
    const graph = parseAndNormalizeGraph(rawData);
    
    deferredGraphUrl = null; // Clear the deferred URL
    loadGraph(graph, 'Loaded from deferred graph');
  } catch (error) {
    console.error('Failed to load deferred graph:', error);
    showError(`Failed to load graph: ${error instanceof Error ? error.message : 'Unknown error'}`);
  } finally {
    isDeferredLoadInProgress = false;
  }
}

/**
 * Load a deferred graph and then check for disambiguation
 * This is used when pressing Enter in the include-files input
 */
async function loadDeferredGraphWithDisambiguation(): Promise<void> {
  if (!deferredGraphUrl) return;
  
  // Prevent multiple simultaneous loads
  if (isDeferredLoadInProgress) {
    console.log('Deferred graph load already in progress, skipping');
    return;
  }
  
  // Check if user has entered a search query
  if (!hasSearchFilters()) {
    showError('Please enter a Source, Sink, or Include Files filter first to filter the large graph.');
    return;
  }
  
  isDeferredLoadInProgress = true;
  
  const statsDiv = document.getElementById('stats');
  if (statsDiv) {
    statsDiv.innerHTML = `
      <div style="padding: 1rem; text-align: center;">
        <div style="margin-bottom: 0.5rem;">⏳ Loading graph...</div>
        <div style="font-size: 0.8rem; color: #666;">This may take a few seconds...</div>
      </div>
    `;
  }
  
  try {
    const response = await fetch(deferredGraphUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch: ${response.status}`);
    }
    
    const text = await response.text();
    const rawData = JSON.parse(text);
    const graph = parseAndNormalizeGraph(rawData);
    
    deferredGraphUrl = null; // Clear the deferred URL
    
    // Load the graph but DON'T apply filters yet
    state.fullGraph = {
      nodes: graph.nodes.map(n => ({ ...n })),
      links: graph.links.map(l => ({ ...l })),
      metadata: { ...graph.metadata },
    };
    
    // Populate the file list so disambiguation has data
    populateFileList();
    
    console.log('Graph loaded, checking for disambiguation...');
    
    // Now check for disambiguation
    const hasAmbiguity = checkAndShowDisambiguation();
    if (!hasAmbiguity) {
      // No ambiguity - apply filters
      console.log('No ambiguity, applying filters');
      loadGraph(graph, 'Loaded from deferred graph');
    } else {
      // Show success message but don't apply filters yet (user selecting)
      console.log('Ambiguity found, waiting for user selection');
      const statsDiv = document.getElementById('stats');
      if (statsDiv) {
        statsDiv.innerHTML = `
          <div style="padding: 1rem; text-align: center;">
            <div style="margin-bottom: 0.5rem;">📊 Graph loaded (${graph.nodes.length.toLocaleString()} nodes)</div>
            <div style="font-size: 0.8rem; color: #666;">Select files from the dropdown above...</div>
          </div>
        `;
      }
    }
  } catch (error) {
    console.error('Failed to load deferred graph:', error);
    showError(`Failed to load graph: ${error instanceof Error ? error.message : 'Unknown error'}`);
  } finally {
    isDeferredLoadInProgress = false;
  }
}

/**
 * Check if graph is too large to render without filters
 */
function isLargeGraph(graph: D3Graph): boolean {
  return graph.links.length > LARGE_GRAPH_LINK_THRESHOLD;
}

/**
 * Check if user has specified meaningful filters (source, sink, or include files)
 */
function hasSearchFilters(): boolean {
  return state.filters.sourceQuery.trim() !== '' || 
         state.filters.sinkQuery.trim() !== '' ||
         state.filters.includeFiles.trim() !== '' ||
         state.filters.focusNodeIds.size > 0;
}

/**
 * Fetch and load a focus set JSON file.
 * 
 * The JSON should have:
 * - focus_nodes: string[] - SCIP node IDs for exact matching
 * - focus_functions: Array<{display_name, relative_path}> - for fuzzy matching
 *   across different analyzers (rust-analyzer vs verus-analyzer produce different IDs)
 * 
 * The loader first tries exact ID matching. If few IDs match (suggesting an analyzer
 * mismatch), it falls back to matching by (display_name, relative_path).
 * 
 * @param url - URL to the focus set JSON file
 * @returns Promise that resolves when focus set is loaded
 */
async function loadFocusSet(url: string): Promise<void> {
  try {
    console.log('Loading focus set from:', url);
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`Failed to fetch focus set: ${response.status} ${response.statusText}`);
    }
    
    const data = await response.json();
    
    if (!data.focus_nodes || !Array.isArray(data.focus_nodes)) {
      throw new Error('Invalid focus set JSON: missing "focus_nodes" array');
    }
    
    // Try exact ID matching first
    const focusIdSet = new Set<string>(data.focus_nodes);
    const resolvedIds = new Set<string>();
    
    if (state.fullGraph) {
      const graphIds = new Set(state.fullGraph.nodes.map(n => n.id));
      
      // Count exact matches
      for (const id of focusIdSet) {
        if (graphIds.has(id)) {
          resolvedIds.add(id);
        }
      }
      
      console.log(`Focus set: ${resolvedIds.size}/${focusIdSet.size} exact ID matches`);
      
      // If less than half matched and we have focus_functions, fall back to fuzzy matching
      if (resolvedIds.size < focusIdSet.size / 2 && data.focus_functions && Array.isArray(data.focus_functions)) {
        console.log('Few exact matches - falling back to (display_name, relative_path) matching');
        
        // Build index of graph nodes by (display_name, relative_path)
        const graphByNamePath = new Map<string, string[]>();
        for (const node of state.fullGraph.nodes) {
          const key = `${node.display_name}\0${node.relative_path}`;
          if (!graphByNamePath.has(key)) {
            graphByNamePath.set(key, []);
          }
          graphByNamePath.get(key)!.push(node.id);
        }
        
        // Match each focus function by name+path
        let fuzzyMatched = 0;
        for (const func of data.focus_functions) {
          const key = `${func.display_name}\0${func.relative_path}`;
          const matches = graphByNamePath.get(key);
          if (matches) {
            for (const id of matches) {
              resolvedIds.add(id);
            }
            fuzzyMatched++;
          }
        }
        console.log(`Fuzzy matching: ${fuzzyMatched}/${data.focus_functions.length} functions resolved to ${resolvedIds.size} node IDs`);
      }
    } else {
      // No graph loaded yet - use raw IDs and hope for the best
      for (const id of focusIdSet) {
        resolvedIds.add(id);
      }
    }
    
    // Populate focusNodeIds in state
    state.filters.focusNodeIds = resolvedIds;
    
    const description = data.metadata?.description || 'unknown';
    console.log(`Focus set active: ${resolvedIds.size} nodes (${description})`);
    
    // Update the focus indicator UI
    updateFocusIndicator();
    
    // Re-apply filters with the focus set active
    applyFiltersAndUpdate();
  } catch (error) {
    console.error('Failed to load focus set:', error);
    showError(`Failed to load focus set: ${error instanceof Error ? error.message : 'Unknown error'}`);
  }
}

/**
 * Load a graph and update the UI
 */
function loadGraph(graph: D3Graph, message: string): void {
  // Deep copy the graph to prevent D3 from mutating original data
  // D3 modifies link.source/target from string IDs to node object references
  state.fullGraph = {
    nodes: graph.nodes.map(n => ({ ...n })),
    links: graph.links.map(l => ({ ...l })),
    metadata: { ...graph.metadata },
  };
  // Deep copy filters - spread only does shallow copy, so Sets would be shared!
  // Preserve focusNodeIds across graph reloads (it's set from URL param, not graph data)
  const preservedFocusNodes = state.filters.focusNodeIds;
  state.filters = { 
    ...initialFilters,
    selectedNodes: new Set(),
    expandedNodes: new Set(),
    hiddenNodes: new Set(),
    focusNodeIds: preservedFocusNodes.size > 0 ? new Set(preservedFocusNodes) : new Set(),
  };
  
  // Set GitHub URL from metadata if not already set via URL param
  if (!githubBaseUrl && graph.metadata.github_url) {
    githubBaseUrl = graph.metadata.github_url;
  }
  
  const isLarge = isLargeGraph(state.fullGraph);
  
  // Populate the file list panel
  populateFileList();
  
  // Apply URL filter parameters (if any)
  const urlFilters = parseFiltersFromURL();
  if (Object.keys(urlFilters).length > 0) {
    applyURLFiltersToState(urlFilters);
    
    // Resolve hidden node names to IDs
    if ((urlFilters as any)._hiddenNames) {
      resolveHiddenNodeNames((urlFilters as any)._hiddenNames);
    }
  }
  
  applyFiltersAndUpdate();
  
  // Load focus set if URL parameter is present and not already loaded
  if (focusJsonUrl && state.filters.focusNodeIds.size === 0) {
    loadFocusSet(focusJsonUrl);
  } else if (state.filters.focusNodeIds.size > 0) {
    // Focus set already loaded (preserved from previous graph load) - update indicator
    updateFocusIndicator();
  }
  
  console.log(message, {
    nodes: graph.nodes.length,
    links: graph.links.length,
    metadata: graph.metadata,
  });

  // Show success message
  const statsDiv = document.getElementById('stats');
  if (statsDiv && graph.nodes.length > 0) {
    const successMsg = document.createElement('div');
    
    if (isLarge && !hasSearchFilters()) {
      // Show warning for large graphs
      successMsg.style.cssText = 'background: #ff9800; color: white; padding: 0.5rem; border-radius: 4px; margin-bottom: 0.5rem; font-size: 0.85rem;';
      successMsg.innerHTML = `⚠️ Large graph (${graph.nodes.length.toLocaleString()} nodes, ${graph.links.length.toLocaleString()} links). Use <strong>Source</strong>, <strong>Sink</strong>, or <strong>Include Files</strong> filters to search.`;
    } else {
      successMsg.style.cssText = 'background: #4caf50; color: white; padding: 0.5rem; border-radius: 4px; margin-bottom: 0.5rem; font-size: 0.85rem;';
      successMsg.textContent = `✓ ${message}`;
      // Remove success message after 5 seconds (but keep warning visible)
      setTimeout(() => successMsg.remove(), 5000);
    }
    
    statsDiv.insertBefore(successMsg, statsDiv.firstChild);
  }
}

/**
 * Show error message
 */
function showError(message: string): void {
  const statsDiv = document.getElementById('stats');
  if (statsDiv) {
    const errorMsg = document.createElement('div');
    errorMsg.style.cssText = 'background: #f44336; color: white; padding: 0.5rem; border-radius: 4px; margin-bottom: 0.5rem; font-size: 0.85rem;';
    errorMsg.textContent = `❌ ${message}`;
    statsDiv.insertBefore(errorMsg, statsDiv.firstChild);
    
    // Remove message after 8 seconds
    setTimeout(() => errorMsg.remove(), 8000);
  }
}

/**
 * Handle file load
 */
async function handleFileLoad(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  
  if (!file) return;

  try {
    const text = await file.text();
    const rawData = JSON.parse(text);
    const graph = parseAndNormalizeGraph(rawData);
    
    loadGraph(graph, `Loaded from file: ${file.name}`);
  } catch (error) {
    console.error('Error loading graph:', error);
    showError(`Error loading graph file: ${error instanceof Error ? error.message : 'Invalid JSON'}`);
  }
}

// Maximum nodes to render to prevent D3 from freezing
const MAX_RENDERED_NODES = 200;

/**
 * Apply filters and update visualization
 */
function applyFiltersAndUpdate(): void {
  if (!state.fullGraph) return;

  // For large graphs, require a search filter to render anything
  if (isLargeGraph(state.fullGraph) && !hasSearchFilters()) {
    state.filteredGraph = { nodes: [], links: [], metadata: state.fullGraph.metadata };
    visualization?.update(state.filteredGraph);
    updateStats();
    updateNodeInfo();
    updateHiddenNodesUI();
    updateURLWithFilters();
    return;
  }
  
  // Pass selectedNodeId for exact matching (VS Code integration)
  const nodeOptions: SelectedNodeOptions = { selectedNodeId };
  let filtered = applyFilters(state.fullGraph, state.filters, nodeOptions);
  
  // Limit rendered nodes for large results to prevent D3 freeze
  let wasTruncated = false;
  if (filtered.nodes.length > MAX_RENDERED_NODES) {
    wasTruncated = true;
    
    // Keep nodes with highest connectivity (most relevant)
    const sortedNodes = [...filtered.nodes].sort((a, b) => 
      (b.dependents.length + (b.dependencies?.length || 0)) - (a.dependents.length + (a.dependencies?.length || 0))
    );
    const keptNodes = sortedNodes.slice(0, MAX_RENDERED_NODES);
    const keptNodeIds = new Set(keptNodes.map(n => n.id));
    
    // Filter links to only include those between kept nodes
    const keptLinks = filtered.links.filter(link => {
      const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
      const targetId = typeof link.target === 'string' ? link.target : link.target.id;
      return keptNodeIds.has(sourceId) && keptNodeIds.has(targetId);
    });
    
    filtered = { nodes: keptNodes, links: keptLinks, metadata: filtered.metadata };
  }
  
  state.filteredGraph = filtered;
  visualization?.update(state.filteredGraph);
  updateStats(wasTruncated ? filtered.nodes.length : undefined);
  updateNodeInfo();
  updateHiddenNodesUI();
  
  // Update URL with current filter state (without adding to history)
  updateURLWithFilters();
}

/**
 * Update the browser URL with current filter state
 * Uses replaceState to avoid polluting browser history
 */
function updateURLWithFilters(): void {
  const url = generateShareableURL();
  window.history.replaceState({}, '', url);
}

/**
 * Handle state changes from visualization
 */
function handleStateChange(newState: GraphState, selectionChanged: boolean = false): void {
  state = newState;
  updateNodeInfo();
  
  // Re-apply filters if selection/hidden nodes changed (not just hover)
  // This ensures hidden nodes are properly filtered out
  if (selectionChanged) {
    applyFiltersAndUpdate();
  }
}

/**
 * Update statistics display
 * @param truncatedTo - if provided, indicates results were truncated to this count
 */
function updateStats(truncatedTo?: number): void {
  const statsDiv = document.getElementById('stats');
  if (!statsDiv) return;

  if (!state.fullGraph) {
    statsDiv.innerHTML = '<p>No graph loaded. Please load a JSON file.</p>';
    return;
  }

  const filtered = state.filteredGraph || state.fullGraph;
  const isLarge = isLargeGraph(state.fullGraph);
  const needsFilter = isLarge && !hasSearchFilters();
  const wasTruncated = truncatedTo !== undefined;
  
  // Count verification statuses
  const verifiedCount = filtered.nodes.filter(n => n.verification_status === 'verified').length;
  const failedCount = filtered.nodes.filter(n => n.verification_status === 'failed').length;
  const unverifiedCount = filtered.nodes.filter(n => n.verification_status === 'unverified').length;
  const unknownCount = filtered.nodes.filter(n => !n.verification_status).length;

  statsDiv.innerHTML = `
    ${needsFilter ? `
    <div class="stat-item" style="background: #fff3e0; padding: 8px; border-radius: 4px; margin-bottom: 8px;">
      <span style="color: #e65100; font-weight: bold;">📊 Large Graph</span>
      <p style="margin: 4px 0 0 0; font-size: 0.85rem; color: #666;">
        Enter a <strong>Source</strong>, <strong>Sink</strong>, or <strong>Include Files</strong> filter to explore the call graph.
      </p>
    </div>
    ` : ''}
    ${wasTruncated ? `
    <div class="stat-item" style="background: #e3f2fd; padding: 8px; border-radius: 4px; margin-bottom: 8px;">
      <span style="color: #1565c0; font-weight: bold;">✂️ Results Limited</span>
      <p style="margin: 4px 0 0 0; font-size: 0.85rem; color: #666;">
        Showing top ${truncatedTo} nodes by connectivity. Use a more specific query or reduce <strong>Depth</strong> to narrow results.
      </p>
    </div>
    ` : ''}
    <div class="stat-item">
      <span class="stat-label">Total Nodes:</span>
      <span class="stat-value">${filtered.nodes.length}</span>
      <span class="stat-detail">(of ${state.fullGraph.nodes.length.toLocaleString()})</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Total Edges:</span>
      <span class="stat-value">${filtered.links.length}</span>
      <span class="stat-detail">(of ${state.fullGraph.links.length.toLocaleString()})</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Verified:</span>
      <span class="stat-value" style="color: #22c55e;">${verifiedCount}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Failed:</span>
      <span class="stat-value" style="color: #ef4444;">${failedCount}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Unverified:</span>
      <span class="stat-value" style="color: #9ca3af;">${unverifiedCount}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Unknown:</span>
      <span class="stat-value" style="color: #3b82f6;">${unknownCount}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Project:</span>
      <span class="stat-value" style="font-size: 10px;">${state.fullGraph.metadata.project_root}</span>
    </div>
  `;
}

/**
 * Update node information panel
 */
function updateNodeInfo(): void {
  const nodeInfoDiv = document.getElementById('node-info');
  if (!nodeInfoDiv) return;

  const node = state.selectedNode || state.hoveredNode;

  if (!node || !state.filteredGraph) {
    nodeInfoDiv.innerHTML = '<p class="placeholder">Click or hover over a node to see details</p>';
    return;
  }

  // Get callers/callees from FULL graph (not filtered) to show complete info
  const allCallers = state.fullGraph ? getCallers(state.fullGraph, node.id) : [];
  const allCallees = state.fullGraph ? getCallees(state.fullGraph, node.id) : [];
  
  // Also get visible callers/callees for the list (only show what's in view)
  const visibleCallers = getCallers(state.filteredGraph, node.id);
  const visibleCallees = getCallees(state.filteredGraph, node.id);

  const callersHtml = allCallers.length > 0
    ? allCallers.map(n => {
        const isVisible = visibleCallers.some(vc => vc.id === n.id);
        return `<li${!isVisible ? ' style="opacity: 0.5;"' : ''}>${n.display_name}${!isVisible ? ' <em>(filtered)</em>' : ''}</li>`;
      }).join('')
    : '<li><em>None</em></li>';

  const calleesHtml = allCallees.length > 0
    ? allCallees.map(n => {
        const isVisible = visibleCallees.some(vc => vc.id === n.id);
        return `<li${!isVisible ? ' style="opacity: 0.5;"' : ''}>${n.display_name}${!isVisible ? ' <em>(filtered)</em>' : ''}</li>`;
      }).join('')
    : '<li><em>None</em></li>';

  const githubLink = buildGitHubLink(node);
  const lineInfo = node.start_line 
    ? (node.end_line && node.end_line !== node.start_line 
        ? `Lines ${node.start_line}-${node.end_line}` 
        : `Line ${node.start_line}`)
    : '';

  // Get verification status badge
  const getVerificationBadge = (status: string | undefined): string => {
    switch (status) {
      case 'verified':
        return '<div class="node-badge badge-verified">✓ Verified</div>';
      case 'failed':
        return '<div class="node-badge badge-failed">✗ Failed</div>';
      case 'unverified':
        return '<div class="node-badge badge-unverified">○ Unverified</div>';
      default:
        return '<div class="node-badge badge-unknown">? Unknown</div>';
    }
  };

  nodeInfoDiv.innerHTML = `
    <div class="node-detail">
      <h3>${node.display_name}</h3>
      <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
        <div class="node-badge ${node.is_libsignal ? 'badge-libsignal' : 'badge-other'}">
          ${node.is_libsignal ? 'Libsignal' : 'External'}
        </div>
        ${getVerificationBadge(node.verification_status)}
      </div>
    </div>
    <div class="node-detail">
      <strong>File:</strong> ${node.file_name}
      ${lineInfo ? `<span style="color: #888; margin-left: 0.5rem;">(${lineInfo})</span>` : ''}
    </div>
    <div class="node-detail">
      <strong>Path:</strong> 
      <code class="code-block">${node.relative_path}</code>
    </div>
    <div class="node-detail">
      <button id="navigate-to-source-btn" class="github-link" style="background: none; border: none; cursor: pointer; padding: 0; text-decoration: underline; color: inherit;">
        ${isVSCodeEnvironment() ? '📂 Open in Editor' : (githubLink ? '📂 View on GitHub' : '')}
      </button>
    </div>
    <div class="node-detail">
      <strong>Callers (${allCallers.length}):</strong>
      <ul class="node-list">${callersHtml}</ul>
    </div>
    <div class="node-detail">
      <strong>Callees (${allCallees.length}):</strong>
      <ul class="node-list">${calleesHtml}</ul>
    </div>
    ${node.similar_lemmas && node.similar_lemmas.length > 0 ? `
      <div class="node-detail">
        <strong>🔍 Similar Lemmas:</strong>
        <ul class="similar-lemmas-list">
          ${node.similar_lemmas.map(lemma => `
            <li class="similar-lemma-item">
              <span class="similar-lemma-name">${escapeHtml(lemma.name)}</span>
              <span class="similar-lemma-score">${(lemma.score * 100).toFixed(0)}%</span>
              <div class="similar-lemma-meta">
                ${escapeHtml(lemma.file_path)}${lemma.line_number ? `:${lemma.line_number}` : ''}
              </div>
            </li>
          `).join('')}
        </ul>
      </div>
    ` : ''}
  `;
  
  // Add click handler for navigate button
  const navigateBtn = document.getElementById('navigate-to-source-btn');
  if (navigateBtn && node) {
    navigateBtn.addEventListener('click', () => {
      navigateToSource(node);
    });
  }
}

/**
 * Information about a file entry for the file list
 */
interface FileListEntry {
  fileName: string;           // Base file name (e.g., "edwards.rs")
  relativePath: string;       // Full relative path (e.g., "curve25519-dalek/src/edwards.rs")
  displayName: string;        // Display name with disambiguation if needed
  filterPattern: string;      // Pattern to use in the filter (may include path for disambiguation)
  count: number;              // Number of functions in this file
  isDuplicate: boolean;       // Whether this filename has duplicates
}

/**
 * Compute the shortest disambiguating path suffix for a set of paths
 * For example, given:
 *   - "curve25519-dalek/src/edwards.rs"
 *   - "curve25519-dalek/src/backend/vector/ifma/edwards.rs"
 * Returns:
 *   - "src/edwards.rs"
 *   - "ifma/edwards.rs"
 */
function computeDisambiguatedPaths(paths: string[]): Map<string, string> {
  const result = new Map<string, string>();
  
  if (paths.length <= 1) {
    // No disambiguation needed
    paths.forEach(p => {
      const fileName = p.split('/').pop() || p;
      result.set(p, fileName);
    });
    return result;
  }
  
  // Split each path into segments (reversed for bottom-up comparison)
  const pathSegments = paths.map(p => p.split('/').reverse());
  
  // For each path, find the minimum number of segments needed to be unique
  for (let i = 0; i < paths.length; i++) {
    const segments = pathSegments[i];
    let numSegments = 1;  // Start with just the filename
    
    // Increase segments until this path is unique among all paths
    while (numSegments < segments.length) {
      const suffix = segments.slice(0, numSegments).reverse().join('/');
      
      // Check if any other path has the same suffix
      let isUnique = true;
      for (let j = 0; j < paths.length; j++) {
        if (i === j) continue;
        const otherSuffix = pathSegments[j].slice(0, numSegments).reverse().join('/');
        if (suffix === otherSuffix) {
          isUnique = false;
          break;
        }
      }
      
      if (isUnique) break;
      numSegments++;
    }
    
    const disambiguatedPath = segments.slice(0, numSegments).reverse().join('/');
    result.set(paths[i], disambiguatedPath);
  }
  
  return result;
}

/**
 * Populate the file list panel with unique files from the graph
 * Shows disambiguated names for files with duplicate names
 */
function populateFileList(): void {
  if (!state.fullGraph) return;
  
  const fileListDiv = document.getElementById('file-list');
  const fileCountSpan = document.getElementById('file-count');
  if (!fileListDiv) return;

  // Group nodes by file_name and relative_path
  // Key: relative_path, Value: { fileName, count }
  const filesByPath = new Map<string, { fileName: string; count: number }>();
  state.fullGraph.nodes.forEach(node => {
    const fileName = node.file_name || 'unknown';
    const relativePath = node.relative_path || fileName;
    
    const existing = filesByPath.get(relativePath);
    if (existing) {
      existing.count++;
    } else {
      filesByPath.set(relativePath, { fileName, count: 1 });
    }
  });

  // Group by filename to detect duplicates
  const pathsByFileName = new Map<string, string[]>();
  for (const [relativePath, { fileName }] of filesByPath) {
    if (!pathsByFileName.has(fileName)) {
      pathsByFileName.set(fileName, []);
    }
    pathsByFileName.get(fileName)!.push(relativePath);
  }

  // Build file list entries with disambiguation
  const entries: FileListEntry[] = [];
  
  for (const [fileName, paths] of pathsByFileName) {
    const isDuplicate = paths.length > 1;
    
    if (isDuplicate) {
      // Compute disambiguated paths
      const disambiguated = computeDisambiguatedPaths(paths);
      
      for (const relativePath of paths) {
        const { count } = filesByPath.get(relativePath)!;
        const disambiguatedPath = disambiguated.get(relativePath)!;
        
        entries.push({
          fileName,
          relativePath,
          displayName: fileName,
          filterPattern: disambiguatedPath,  // Use disambiguated path for filtering
          count,
          isDuplicate: true,
        });
      }
    } else {
      // No duplication, use simple filename
      const relativePath = paths[0];
      const { count } = filesByPath.get(relativePath)!;
      
      entries.push({
        fileName,
        relativePath,
        displayName: fileName,
        filterPattern: fileName,  // Simple filename for filtering
        count,
        isDuplicate: false,
      });
    }
  }

  // Sort entries: first by filename, then by path for duplicates
  entries.sort((a, b) => {
    const nameCompare = a.fileName.localeCompare(b.fileName);
    if (nameCompare !== 0) return nameCompare;
    return a.relativePath.localeCompare(b.relativePath);
  });

  // Update count (unique relative paths)
  if (fileCountSpan) {
    fileCountSpan.textContent = entries.length.toString();
  }

  // Build file list HTML
  fileListDiv.innerHTML = entries.map(entry => {
    const disambigHtml = entry.isDuplicate 
      ? `<span class="file-disambig">(${escapeHtml(entry.filterPattern.replace('/' + entry.fileName, ''))})</span>`
      : '';
    
    return `
      <div class="file-list-item${entry.isDuplicate ? ' has-duplicates' : ''}" 
           data-file="${escapeHtml(entry.filterPattern)}"
           data-path="${escapeHtml(entry.relativePath)}"
           title="${escapeHtml(entry.relativePath)}">
        <span class="file-icon">📄</span>
        <span class="file-name">${escapeHtml(entry.fileName)}</span>
        ${disambigHtml}
        <span class="file-count">${entry.count}</span>
      </div>
    `;
  }).join('');

  // Add click handlers to toggle file selection
  fileListDiv.querySelectorAll('.file-list-item').forEach(item => {
    item.addEventListener('click', () => {
      const filterPattern = item.getAttribute('data-file');
      if (filterPattern) {
        toggleFileInFilter(filterPattern);
      }
    });
  });

  // Update selection state based on current filter
  updateFileListSelection();
}

/**
 * Information about a file that matches an ambiguous pattern
 */
interface AmbiguousFileMatch {
  fileName: string;
  relativePath: string;
  disambiguatedPath: string;
  count: number;
}

/**
 * Find files that match an ambiguous pattern (filename without path)
 * Returns null if not ambiguous, or array of matches if multiple files match
 */
function findAmbiguousMatches(pattern: string): AmbiguousFileMatch[] | null {
  if (!state.fullGraph) return null;
  
  // Only check for ambiguity if pattern is a simple filename (no path separators, no complex globs)
  if (pattern.includes('/') || pattern.includes('**')) {
    return null;  // Already disambiguated or complex pattern
  }
  
  // Convert simple glob to regex
  let regexStr = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  regexStr = regexStr.replace(/\*/g, '.*');
  regexStr = regexStr.replace(/\?/g, '.');
  const regex = new RegExp('^' + regexStr + '$', 'i');
  
  // Find all files that match this pattern
  const filesByPath = new Map<string, { fileName: string; count: number }>();
  state.fullGraph.nodes.forEach(node => {
    const fileName = node.file_name || '';
    const relativePath = node.relative_path || fileName;
    
    if (regex.test(fileName)) {
      const existing = filesByPath.get(relativePath);
      if (existing) {
        existing.count++;
      } else {
        filesByPath.set(relativePath, { fileName, count: 1 });
      }
    }
  });
  
  // If only one file matches, not ambiguous
  if (filesByPath.size <= 1) {
    return null;
  }
  
  // Multiple files match - compute disambiguated paths
  const paths = Array.from(filesByPath.keys());
  const disambiguated = computeDisambiguatedPaths(paths);
  
  const matches: AmbiguousFileMatch[] = [];
  for (const [relativePath, { fileName, count }] of filesByPath) {
    matches.push({
      fileName,
      relativePath,
      disambiguatedPath: disambiguated.get(relativePath) || relativePath,
      count,
    });
  }
  
  // Sort by path for consistent display
  matches.sort((a, b) => a.relativePath.localeCompare(b.relativePath));
  
  return matches;
}

/**
 * Current disambiguation state
 */
let disambiguationState: {
  pattern: string;
  matches: AmbiguousFileMatch[];
  selectedPaths: Set<string>;
} | null = null;

/**
 * Show the disambiguation dropdown for an ambiguous pattern
 */
function showDisambiguationDropdown(pattern: string, matches: AmbiguousFileMatch[]): void {
  const dropdown = document.getElementById('file-disambiguation-dropdown');
  if (!dropdown) return;
  
  disambiguationState = {
    pattern,
    matches,
    selectedPaths: new Set(),
  };
  
  // Build dropdown HTML
  dropdown.innerHTML = `
    <div class="dropdown-header">
      Multiple files match "<strong>${escapeHtml(pattern)}</strong>" - select which to include:
    </div>
    ${matches.map((match, index) => `
      <div class="dropdown-item" data-path="${escapeHtml(match.disambiguatedPath)}" data-index="${index}">
        <input type="checkbox" class="checkbox" />
        <span class="file-name">${escapeHtml(match.fileName)}</span>
        <span class="file-path" title="${escapeHtml(match.relativePath)}">${escapeHtml(match.disambiguatedPath.replace('/' + match.fileName, ''))}</span>
        <span class="file-count" style="background: #f0f0f0; padding: 2px 6px; border-radius: 10px; font-size: 0.7rem; color: #666;">${match.count}</span>
      </div>
    `).join('')}
    <div class="dropdown-actions">
      <button class="btn-all" title="Include all matching files">All</button>
      <button class="btn-cancel">Cancel</button>
      <button class="btn-apply">Apply</button>
    </div>
  `;
  
  // Show dropdown
  dropdown.style.display = 'block';
  
  // Add click handlers for items
  dropdown.querySelectorAll('.dropdown-item').forEach(item => {
    item.addEventListener('click', (e) => {
      const target = e.target as HTMLElement;
      // Don't toggle if clicking the checkbox directly (it will toggle itself)
      if (target.tagName === 'INPUT') return;
      
      const checkbox = item.querySelector('input[type="checkbox"]') as HTMLInputElement;
      if (checkbox) {
        checkbox.checked = !checkbox.checked;
        item.classList.toggle('selected', checkbox.checked);
        
        const path = item.getAttribute('data-path');
        if (path) {
          if (checkbox.checked) {
            disambiguationState?.selectedPaths.add(path);
          } else {
            disambiguationState?.selectedPaths.delete(path);
          }
        }
      }
    });
    
    // Handle checkbox changes directly
    const checkbox = item.querySelector('input[type="checkbox"]') as HTMLInputElement;
    checkbox?.addEventListener('change', () => {
      item.classList.toggle('selected', checkbox.checked);
      const path = item.getAttribute('data-path');
      if (path) {
        if (checkbox.checked) {
          disambiguationState?.selectedPaths.add(path);
        } else {
          disambiguationState?.selectedPaths.delete(path);
        }
      }
    });
  });
  
  // Add button handlers
  dropdown.querySelector('.btn-all')?.addEventListener('click', () => {
    // Select all and apply
    dropdown.querySelectorAll('.dropdown-item').forEach(item => {
      const checkbox = item.querySelector('input[type="checkbox"]') as HTMLInputElement;
      if (checkbox) {
        checkbox.checked = true;
        item.classList.add('selected');
        const path = item.getAttribute('data-path');
        if (path) disambiguationState?.selectedPaths.add(path);
      }
    });
    applyDisambiguationSelection();
  });
  
  dropdown.querySelector('.btn-cancel')?.addEventListener('click', () => {
    hideDisambiguationDropdown();
  });
  
  dropdown.querySelector('.btn-apply')?.addEventListener('click', () => {
    applyDisambiguationSelection();
  });
}

/**
 * Hide the disambiguation dropdown
 */
function hideDisambiguationDropdown(): void {
  const dropdown = document.getElementById('file-disambiguation-dropdown');
  if (dropdown) {
    dropdown.style.display = 'none';
  }
  disambiguationState = null;
}

/**
 * Apply the selection from the disambiguation dropdown
 */
function applyDisambiguationSelection(): void {
  if (!disambiguationState || disambiguationState.selectedPaths.size === 0) {
    hideDisambiguationDropdown();
    return;
  }
  
  const input = document.getElementById('include-files') as HTMLInputElement;
  if (!input) {
    hideDisambiguationDropdown();
    return;
  }
  
  // Get current patterns and replace the ambiguous one with selected paths
  const currentPatterns = input.value
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0);
  
  // Find and replace the ambiguous pattern
  const patternIndex = currentPatterns.findIndex(
    p => p.toLowerCase() === disambiguationState!.pattern.toLowerCase()
  );
  
  if (patternIndex >= 0) {
    // Replace the ambiguous pattern with selected paths
    currentPatterns.splice(patternIndex, 1, ...disambiguationState.selectedPaths);
  } else {
    // Pattern not found (maybe already modified), just add selected paths
    currentPatterns.push(...disambiguationState.selectedPaths);
  }
  
  // Update input and filter
  input.value = currentPatterns.join(', ');
  state.filters.includeFiles = input.value;
  
  hideDisambiguationDropdown();
  updateFileListSelection();
  applyFiltersAndUpdate();
}

/**
 * Check for ambiguous patterns in the include files input and show disambiguation if needed
 * Returns true if disambiguation dropdown was shown
 */
function checkAndShowDisambiguation(): boolean {
  console.log('checkAndShowDisambiguation called');
  if (!state.fullGraph) {
    console.log('No fullGraph loaded');
    return false;
  }
  
  const input = document.getElementById('include-files') as HTMLInputElement;
  if (!input) {
    console.log('No include-files input found');
    return false;
  }
  
  const patterns = input.value
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0);
  
  console.log('Checking patterns:', patterns);
  
  // Check each pattern for ambiguity
  for (const pattern of patterns) {
    const matches = findAmbiguousMatches(pattern);
    console.log(`Pattern "${pattern}" has ${matches?.length || 0} matches:`, matches);
    if (matches && matches.length > 1) {
      console.log('Showing disambiguation dropdown');
      showDisambiguationDropdown(pattern, matches);
      return true;
    }
  }
  
  console.log('No ambiguous patterns found');
  return false;
}

/**
 * Toggle a file in the include filter
 */
function toggleFileInFilter(fileName: string): void {
  const input = document.getElementById('include-files') as HTMLInputElement;
  if (!input) return;

  const currentPatterns = input.value
    .split(',')
    .map(p => p.trim())
    .filter(p => p.length > 0);

  const fileIndex = currentPatterns.indexOf(fileName);
  if (fileIndex >= 0) {
    // Remove the file
    currentPatterns.splice(fileIndex, 1);
  } else {
    // Add the file
    currentPatterns.push(fileName);
  }

  // Update input and filter
  input.value = currentPatterns.join(', ');
  state.filters.includeFiles = input.value;
  updateFileListSelection();
  applyFiltersAndUpdate();
}

/**
 * Update the file list to show which files are currently selected
 */
function updateFileListSelection(): void {
  const fileListDiv = document.getElementById('file-list');
  if (!fileListDiv) return;

  const currentPatterns = state.filters.includeFiles
    .split(',')
    .map(p => p.trim().toLowerCase())
    .filter(p => p.length > 0);

  fileListDiv.querySelectorAll('.file-list-item').forEach(item => {
    const filterPattern = item.getAttribute('data-file')?.toLowerCase() || '';
    const relativePath = item.getAttribute('data-path')?.toLowerCase() || '';
    
    const isSelected = currentPatterns.some(pattern => {
      // Check for exact match with the filter pattern first
      if (filterPattern === pattern) return true;
      
      // Check if pattern matches the relative path (for path-based patterns)
      if (pattern.includes('/')) {
        // Path pattern - check if relative path ends with or contains this pattern
        if (relativePath.endsWith(pattern) || relativePath.includes('/' + pattern)) {
          return true;
        }
      }
      
      // Check glob pattern matching
      if (pattern.includes('*') || pattern.includes('?')) {
        // Convert glob to regex for matching
        const regexStr = pattern
          .replace(/[.+^${}()|[\]\\]/g, '\\$&')
          .replace(/\*\*/g, '<<<DOUBLESTAR>>>')
          .replace(/\*/g, '[^/]*')
          .replace(/\?/g, '[^/]')
          .replace(/<<<DOUBLESTAR>>>/g, '.*');
        
        // For path patterns, match against relative path
        if (pattern.includes('/')) {
          const pathRegex = new RegExp('(^|/)' + regexStr + '$', 'i');
          return pathRegex.test(relativePath);
        }
        // For filename patterns, match against the filter pattern (filename or disambig path)
        return new RegExp('^' + regexStr + '$', 'i').test(filterPattern);
      }
      
      // Simple filename match (no glob, no path)
      // Extract just the filename from the filter pattern for comparison
      const justFileName = filterPattern.split('/').pop() || filterPattern;
      return justFileName === pattern;
    });
    
    if (currentPatterns.length === 0) {
      // No filter = nothing selected (all shown)
      item.classList.remove('selected');
    } else {
      item.classList.toggle('selected', isSelected);
    }
  });
}

/**
 * Reset all filters
 */
function resetFilters(): void {
  // Deep copy filters - spread only does shallow copy, so Sets would be shared!
  state.filters = { 
    ...initialFilters,
    selectedNodes: new Set(),
    expandedNodes: new Set(),
    hiddenNodes: new Set(),
    focusNodeIds: new Set(),  // Clear focus set on reset
  };
  state.selectedNode = null;
  focusJsonUrl = null;  // Clear focus URL on reset
  
  // Reset UI controls
  (document.getElementById('show-libsignal') as HTMLInputElement).checked = true;
  (document.getElementById('show-non-libsignal') as HTMLInputElement).checked = true;
  (document.getElementById('show-inner-calls') as HTMLInputElement).checked = true;
  (document.getElementById('show-precondition-calls') as HTMLInputElement).checked = false;
  (document.getElementById('show-postcondition-calls') as HTMLInputElement).checked = false;
  (document.getElementById('show-exec-functions') as HTMLInputElement).checked = true;
  (document.getElementById('show-proof-functions') as HTMLInputElement).checked = true;
  (document.getElementById('show-spec-functions') as HTMLInputElement).checked = false;
  (document.getElementById('exclude-name-patterns') as HTMLInputElement).value = '';
  (document.getElementById('exclude-path-patterns') as HTMLInputElement).value = '';
  (document.getElementById('include-files') as HTMLInputElement).value = '';
  (document.getElementById('source-input') as HTMLInputElement).value = '';
  (document.getElementById('sink-input') as HTMLInputElement).value = '';
  (document.getElementById('depth-limit') as HTMLInputElement).value = '1';
  document.getElementById('depth-value')!.textContent = '1';
  
  // Update file list selection to clear all
  updateFileListSelection();
  
  // Update focus indicator
  updateFocusIndicator();
  
  applyFiltersAndUpdate();
}

/**
 * Update the focus set indicator in the UI
 */
function updateFocusIndicator(): void {
  const container = document.getElementById('focus-indicator');
  if (!container) return;
  
  const focusCount = state.filters.focusNodeIds.size;
  
  if (focusCount === 0) {
    container.style.display = 'none';
    container.innerHTML = '';
    return;
  }
  
  container.style.display = 'block';
  container.innerHTML = `
    <div style="background: #e3f2fd; padding: 10px; border-radius: 6px; border-left: 4px solid #1976d2;">
      <div style="display: flex; justify-content: space-between; align-items: center;">
        <div>
          <strong style="color: #1565c0;">Focus Set Active</strong>
          <div style="font-size: 0.85rem; color: #555; margin-top: 2px;">
            Showing <strong>${focusCount}</strong> entry-point functions
          </div>
        </div>
        <button id="clear-focus-btn" style="background: #1976d2; color: white; border: none; padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 0.8rem;">
          Clear
        </button>
      </div>
      <div style="font-size: 0.75rem; color: #888; margin-top: 4px;">
        Use Source/Sink to expand beyond the focus set
      </div>
    </div>
  `;
  
  // Add click handler for Clear button
  document.getElementById('clear-focus-btn')?.addEventListener('click', () => {
    clearFocusSet();
  });
}

/**
 * Clear the focus set and return to the full graph view
 */
function clearFocusSet(): void {
  state.filters.focusNodeIds = new Set();
  focusJsonUrl = null;
  updateFocusIndicator();
  applyFiltersAndUpdate();
}

/**
 * Update the hidden nodes UI
 */
function updateHiddenNodesUI(): void {
  const container = document.getElementById('hidden-nodes-container');
  const list = document.getElementById('hidden-nodes-list');
  const count = document.getElementById('hidden-count');
  
  if (!container || !list || !count) return;
  
  const hiddenCount = state.filters.hiddenNodes.size;
  count.textContent = hiddenCount.toString();
  
  if (hiddenCount === 0) {
    container.style.display = 'none';
    return;
  }
  
  container.style.display = 'block';
  
  // Build list of hidden nodes
  const nodeMap = new Map(state.fullGraph?.nodes.map(n => [n.id, n]) || []);
  list.innerHTML = '';
  
  state.filters.hiddenNodes.forEach(nodeId => {
    const node = nodeMap.get(nodeId);
    if (node) {
      const item = document.createElement('li');
      item.className = 'hidden-node-item';
      item.textContent = node.display_name;
      item.title = 'Click to unhide';
      item.style.cursor = 'pointer';
      item.addEventListener('click', () => {
        state.filters.hiddenNodes.delete(nodeId);
        applyFiltersAndUpdate();
      });
      list.appendChild(item);
    }
  });
}

/**
 * Show all hidden nodes
 */
function showAllHiddenNodes(): void {
  state.filters.hiddenNodes.clear();
  applyFiltersAndUpdate();
}

/**
 * Handle window resize
 */
function handleResize(): void {
  const graphContainer = document.getElementById('graph-container');
  if (!graphContainer || !visualization) return;
  
  const rect = graphContainer.getBoundingClientRect();
  visualization.resize(rect.width, rect.height);
}

/**
 * Escape HTML for safe display
 */
function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// ============================================================================
// VS Code Message Handler
// ============================================================================

/**
 * Handle messages from VS Code extension
 */
function handleVSCodeMessage(event: MessageEvent): void {
  const message = event.data;
  
  switch (message.type) {
    case 'loadGraph':
      // Load graph data sent from extension
      if (message.graph) {
        // Normalize the graph format (supports both D3Graph and simplified formats)
        const normalizedGraph = parseAndNormalizeGraph(message.graph);
        console.log('[VS Code] Received graph data:', normalizedGraph.nodes?.length, 'nodes');
        
        // Store the selected node ID for exact matching
        if (message.selectedNodeId) {
          selectedNodeId = message.selectedNodeId;
          console.log('[VS Code] Selected node ID:', message.selectedNodeId.slice(-60));
        } else {
          selectedNodeId = null;
        }
        
        loadGraph(normalizedGraph, 'Loaded from VS Code extension');
        
        // Apply initial query if provided
        if (message.initialQuery) {
          const sourceInput = document.getElementById('source-input') as HTMLInputElement;
          const sinkInput = document.getElementById('sink-input') as HTMLInputElement;
          
          if (sourceInput && message.initialQuery.source) {
            sourceInput.value = message.initialQuery.source;
            state.filters.sourceQuery = message.initialQuery.source;
          }
          if (sinkInput && message.initialQuery.sink) {
            sinkInput.value = message.initialQuery.sink;
            state.filters.sinkQuery = message.initialQuery.sink;
          }
          if (message.initialQuery.depth !== undefined) {
            const depthInput = document.getElementById('depth-limit') as HTMLInputElement;
            if (depthInput) {
              depthInput.value = message.initialQuery.depth.toString();
              state.filters.maxDepth = message.initialQuery.depth || null;
              document.getElementById('depth-value')!.textContent = 
                message.initialQuery.depth > 0 ? message.initialQuery.depth.toString() : 'All';
            }
          }
          
          // Apply filters after setting initial query
          applyFiltersAndUpdate();
        }
      }
      break;
      
    case 'setQuery':
      // Update the query (e.g., user clicked on a different function)
      if (message.source !== undefined) {
        const sourceInput = document.getElementById('source-input') as HTMLInputElement;
        if (sourceInput) {
          sourceInput.value = message.source;
          state.filters.sourceQuery = message.source;
        }
      }
      if (message.sink !== undefined) {
        const sinkInput = document.getElementById('sink-input') as HTMLInputElement;
        if (sinkInput) {
          sinkInput.value = message.sink;
          state.filters.sinkQuery = message.sink;
        }
      }
      applyFiltersAndUpdate();
      break;
      
    case 'refresh':
      // Reload the graph (extension will send new data)
      postMessageToExtension({ type: 'requestRefresh' });
      break;
  }
}

/**
 * Setup VS Code integration if running in webview
 */
function setupVSCodeIntegration(): void {
  if (!isVSCodeEnvironment()) {
    return;
  }
  
  console.log('[VS Code] Running in VS Code webview');
  
  // Listen for messages from extension
  window.addEventListener('message', handleVSCodeMessage);
  
  // Hide file input (not needed in VS Code)
  const fileInputContainer = document.querySelector('.file-input-container') as HTMLElement;
  if (fileInputContainer) {
    fileInputContainer.style.display = 'none';
  }
  
  // Update title
  const header = document.querySelector('.header h1');
  if (header) {
    header.textContent = '📊 Call Graph Explorer';
  }
  
  // Notify extension that we're ready
  postMessageToExtension({ type: 'ready' });
}

// Initialize on DOM load
document.addEventListener('DOMContentLoaded', init);

