import { D3Graph, D3Node, GraphState, FilterOptions } from './types';
import { applyFilters, getCallers, getCallees } from './filters';
import { CallGraphVisualization } from './graph';

// GitHub URL for source code links (configurable via env var, URL param, or graph metadata)
// Priority: URL param > graph metadata > env var
let githubBaseUrl: string | null = import.meta.env.VITE_GITHUB_URL || null;

// Path prefix to prepend to relative_path when building GitHub links
// (e.g., "curve25519-dalek" if repo structure is repo/curve25519-dalek/src/...)
let githubPathPrefix: string = import.meta.env.VITE_GITHUB_PATH_PREFIX || '';

/**
 * Get a short snippet from function body (first few lines)
 */
function getBodySnippet(body: string, maxLines: number = 5): string {
  const lines = body.split('\n');
  if (lines.length <= maxLines) {
    return body;
  }
  return lines.slice(0, maxLines).join('\n') + '\n// ...';
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
  
  // Build the full path
  const fullPath = prefix ? `${prefix}/${node.relative_path}` : node.relative_path;
  
  // Build the link with line numbers if available
  let link = `${baseUrl}/blob/main/${fullPath}`;
  
  if (node.start_line) {
    link += `#L${node.start_line}`;
    if (node.end_line && node.end_line !== node.start_line) {
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
  if (params.has('exclude')) filters.excludePatterns = params.get('exclude')!;
  
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
  
  // Hidden nodes (comma-separated display names, since full IDs are too long)
  if (params.has('hidden')) {
    const hiddenNames = params.get('hidden')!.split(',').map(s => s.trim()).filter(s => s);
    // We'll need to resolve these to IDs after graph loads
    (filters as any)._hiddenNames = hiddenNames;
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
  ['source', 'sink', 'exclude', 'depth', 'exec', 'proof', 'spec', 
   'inner', 'pre', 'post', 'libsignal', 'external', 'hidden'].forEach(k => params.delete(k));
  
  // Add current filter state
  if (state.filters.sourceQuery) params.set('source', state.filters.sourceQuery);
  if (state.filters.sinkQuery) params.set('sink', state.filters.sinkQuery);
  if (state.filters.excludePatterns) params.set('exclude', state.filters.excludePatterns);
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
  setInput('exclude-patterns', state.filters.excludePatterns);
  
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
  excludePatterns: '',            // Comma-separated patterns to exclude
  maxDepth: 1,
  sourceQuery: '',  // Source nodes - shows what they call (callees)
  sinkQuery: '',    // Sink nodes - shows who calls them (callers)
  selectedNodes: new Set(),
  expandedNodes: new Set(),
  hiddenNodes: new Set(),
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
  const excludeInput = document.getElementById('exclude-patterns') as HTMLInputElement;
  const depthInput = document.getElementById('depth-limit') as HTMLInputElement;
  
  if (sourceInput?.value) {
    state.filters.sourceQuery = sourceInput.value;
  }
  if (sinkInput?.value) {
    state.filters.sinkQuery = sinkInput.value;
  }
  if (excludeInput?.value) {
    state.filters.excludePatterns = excludeInput.value;
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

  // Try to auto-load graph.json if it exists
  autoLoadGraph();
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

  document.getElementById('source-input')?.addEventListener('input', (e) => {
    state.filters.sourceQuery = (e.target as HTMLInputElement).value;
    applyFiltersAndUpdate();
  });

  document.getElementById('sink-input')?.addEventListener('input', (e) => {
    state.filters.sinkQuery = (e.target as HTMLInputElement).value;
    applyFiltersAndUpdate();
  });

  document.getElementById('exclude-patterns')?.addEventListener('input', (e) => {
    state.filters.excludePatterns = (e.target as HTMLInputElement).value;
    applyFiltersAndUpdate();
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
      const response = await fetch(jsonUrl);
      
      if (!response.ok) {
        throw new Error(`Failed to fetch: ${response.status} ${response.statusText}`);
      }

      const text = await response.text();
      const graph: D3Graph = JSON.parse(text);
      
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
    // Use relative path to work with GitHub Pages base URL
    const response = await fetch('./graph.json');
    
    if (!response.ok) {
      console.log('No graph.json found in public directory. Waiting for manual file load.');
      return;
    }

    const text = await response.text();
    const graph: D3Graph = JSON.parse(text);
    
    loadGraph(graph, 'Auto-loaded from local file');
  } catch (error) {
    console.log('Could not auto-load graph.json:', error);
    // Silently fail - user can still manually load a file
  }
}

/**
 * Load a graph and update the UI
 */
function loadGraph(graph: D3Graph, message: string): void {
  state.fullGraph = graph;
  state.filters = { ...initialFilters };
  
  // Set GitHub URL from metadata if not already set via URL param
  if (!githubBaseUrl && graph.metadata.github_url) {
    githubBaseUrl = graph.metadata.github_url;
  }
  
  console.log('[DEBUG] Graph loaded:', graph.nodes.length, 'nodes,', graph.links.length, 'links');
  
  // Apply URL filter parameters (if any)
  const urlFilters = parseFiltersFromURL();
  if (Object.keys(urlFilters).length > 0) {
    console.log('[DEBUG] Applying URL filters:', urlFilters);
    applyURLFiltersToState(urlFilters);
    
    // Resolve hidden node names to IDs
    if ((urlFilters as any)._hiddenNames) {
      resolveHiddenNodeNames((urlFilters as any)._hiddenNames);
    }
  }
  
  applyFiltersAndUpdate();
  
  console.log(message, {
    nodes: graph.nodes.length,
    links: graph.links.length,
    metadata: graph.metadata,
  });

  // Show success message
  const statsDiv = document.getElementById('stats');
  if (statsDiv && graph.nodes.length > 0) {
    const successMsg = document.createElement('div');
    successMsg.style.cssText = 'background: #4caf50; color: white; padding: 0.5rem; border-radius: 4px; margin-bottom: 0.5rem; font-size: 0.85rem;';
    successMsg.textContent = `✓ ${message}`;
    statsDiv.insertBefore(successMsg, statsDiv.firstChild);
    
    // Remove message after 5 seconds
    setTimeout(() => successMsg.remove(), 5000);
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
    const graph: D3Graph = JSON.parse(text);
    
    loadGraph(graph, `Loaded from file: ${file.name}`);
  } catch (error) {
    console.error('Error loading graph:', error);
    showError(`Error loading graph file: ${error instanceof Error ? error.message : 'Invalid JSON'}`);
  }
}

/**
 * Apply filters and update visualization
 */
function applyFiltersAndUpdate(): void {
  if (!state.fullGraph) return;

  console.log('[DEBUG] applyFiltersAndUpdate called with source:', JSON.stringify(state.filters.sourceQuery), 'sink:', JSON.stringify(state.filters.sinkQuery));
  state.filteredGraph = applyFilters(state.fullGraph, state.filters);
  console.log('[DEBUG] Filtered graph:', state.filteredGraph.nodes.length, 'nodes,', state.filteredGraph.links.length, 'links');
  visualization?.update(state.filteredGraph);
  updateStats();
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
 */
function updateStats(): void {
  const statsDiv = document.getElementById('stats');
  if (!statsDiv) return;

  if (!state.fullGraph) {
    statsDiv.innerHTML = '<p>No graph loaded. Please load a JSON file.</p>';
    return;
  }

  const filtered = state.filteredGraph || state.fullGraph;
  const libsignalNodes = filtered.nodes.filter(n => n.is_libsignal).length;
  const nonLibsignalNodes = filtered.nodes.length - libsignalNodes;

  statsDiv.innerHTML = `
    <div class="stat-item">
      <span class="stat-label">Total Nodes:</span>
      <span class="stat-value">${filtered.nodes.length}</span>
      <span class="stat-detail">(of ${state.fullGraph.nodes.length})</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Total Edges:</span>
      <span class="stat-value">${filtered.links.length}</span>
      <span class="stat-detail">(of ${state.fullGraph.links.length})</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Libsignal:</span>
      <span class="stat-value" style="color: #4a90e2;">${libsignalNodes}</span>
    </div>
    <div class="stat-item">
      <span class="stat-label">Other:</span>
      <span class="stat-value" style="color: #7ed321;">${nonLibsignalNodes}</span>
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

  nodeInfoDiv.innerHTML = `
    <div class="node-detail">
      <h3>${node.display_name}</h3>
      <div class="node-badge ${node.is_libsignal ? 'badge-libsignal' : 'badge-other'}">
        ${node.is_libsignal ? 'Libsignal' : 'External'}
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
    ${githubLink ? `
      <div class="node-detail">
        <a href="${githubLink}" target="_blank" rel="noopener noreferrer" class="github-link">
          📂 View on GitHub
        </a>
      </div>
    ` : ''}
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
                ${lemma.file_path}${lemma.line_number ? `:${lemma.line_number}` : ''}
              </div>
            </li>
          `).join('')}
        </ul>
      </div>
    ` : ''}
    ${node.body ? `
      <div class="node-detail">
        <strong>Code Preview:</strong>
        <pre class="code-block"><code>${escapeHtml(getBodySnippet(node.body, 8))}</code></pre>
      </div>
    ` : ''}
  `;
}

/**
 * Reset all filters
 */
function resetFilters(): void {
  state.filters = { ...initialFilters };
  state.filters.selectedNodes.clear();
  state.filters.hiddenNodes.clear();
  state.selectedNode = null;
  
  // Reset UI controls
  (document.getElementById('show-libsignal') as HTMLInputElement).checked = true;
  (document.getElementById('show-non-libsignal') as HTMLInputElement).checked = true;
  (document.getElementById('show-inner-calls') as HTMLInputElement).checked = true;
  (document.getElementById('show-precondition-calls') as HTMLInputElement).checked = false;
  (document.getElementById('show-postcondition-calls') as HTMLInputElement).checked = false;
  (document.getElementById('show-exec-functions') as HTMLInputElement).checked = true;
  (document.getElementById('show-proof-functions') as HTMLInputElement).checked = true;
  (document.getElementById('show-spec-functions') as HTMLInputElement).checked = false;
  (document.getElementById('exclude-patterns') as HTMLInputElement).value = '';
  (document.getElementById('source-input') as HTMLInputElement).value = '';
  (document.getElementById('sink-input') as HTMLInputElement).value = '';
  (document.getElementById('depth-limit') as HTMLInputElement).value = '1';
  document.getElementById('depth-value')!.textContent = '1';
  
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

// Initialize on DOM load
document.addEventListener('DOMContentLoaded', init);

