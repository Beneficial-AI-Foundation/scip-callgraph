import { D3Graph, GraphState, FilterOptions } from './types';
import { applyFilters, getCallers, getCallees } from './filters';
import { CallGraphVisualization } from './graph';

// Initialize state
const initialFilters: FilterOptions = {
  showLibsignal: true,
  showNonLibsignal: true,
  maxDepth: null,
  searchQuery: '',
  includeCallers: true,
  includeCallees: true,
  selectedNodes: new Set(),
  expandedNodes: new Set(),
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

  document.getElementById('search-input')?.addEventListener('input', (e) => {
    state.filters.searchQuery = (e.target as HTMLInputElement).value;
    applyFiltersAndUpdate();
  });

  document.getElementById('include-callers')?.addEventListener('change', (e) => {
    state.filters.includeCallers = (e.target as HTMLInputElement).checked;
    applyFiltersAndUpdate();
  });

  document.getElementById('include-callees')?.addEventListener('change', (e) => {
    state.filters.includeCallees = (e.target as HTMLInputElement).checked;
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

  // Window resize
  window.addEventListener('resize', handleResize);
}

/**
 * Auto-load graph from URL parameter or local graph.json
 */
async function autoLoadGraph(): Promise<void> {
  // First, check for URL parameter
  const urlParams = new URLSearchParams(window.location.search);
  const jsonUrl = urlParams.get('json') || urlParams.get('url');
  
  if (jsonUrl) {
    try {
      console.log('Loading graph from URL:', jsonUrl);
      const response = await fetch(jsonUrl);
      
      if (!response.ok) {
        throw new Error(`Failed to fetch: ${response.status} ${response.statusText}`);
      }

      const text = await response.text();
      const graph: D3Graph = JSON.parse(text);
      
      loadGraph(graph, `Loaded from URL: ${jsonUrl}`);
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

  state.filteredGraph = applyFilters(state.fullGraph, state.filters);
  visualization?.update(state.filteredGraph);
  updateStats();
  updateNodeInfo();
}

/**
 * Handle state changes from visualization
 */
function handleStateChange(newState: GraphState): void {
  state = newState;
  updateNodeInfo();
  
  // If depth filtering is enabled and we have selected nodes, re-apply filters
  if (state.filters.maxDepth !== null && state.filters.selectedNodes.size > 0) {
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

  const callers = getCallers(state.filteredGraph, node.id);
  const callees = getCallees(state.filteredGraph, node.id);

  const callersHtml = callers.length > 0
    ? callers.map(n => `<li>${n.display_name}</li>`).join('')
    : '<li><em>None</em></li>';

  const calleesHtml = callees.length > 0
    ? callees.map(n => `<li>${n.display_name}</li>`).join('')
    : '<li><em>None</em></li>';

  nodeInfoDiv.innerHTML = `
    <div class="node-detail">
      <h3>${node.display_name}</h3>
      <div class="node-badge ${node.is_libsignal ? 'badge-libsignal' : 'badge-other'}">
        ${node.is_libsignal ? 'Libsignal' : 'External'}
      </div>
    </div>
    <div class="node-detail">
      <strong>File:</strong> ${node.file_name}
    </div>
    <div class="node-detail">
      <strong>Path:</strong> 
      <code class="code-block">${node.relative_path}</code>
    </div>
    <div class="node-detail">
      <strong>Symbol:</strong>
      <code class="code-block">${node.symbol}</code>
    </div>
    <div class="node-detail">
      <strong>Callers (${callers.length}):</strong>
      <ul class="node-list">${callersHtml}</ul>
    </div>
    <div class="node-detail">
      <strong>Callees (${callees.length}):</strong>
      <ul class="node-list">${calleesHtml}</ul>
    </div>
    ${node.body ? `
      <div class="node-detail">
        <strong>Body:</strong>
        <pre class="code-block"><code>${escapeHtml(node.body)}</code></pre>
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
  state.selectedNode = null;
  
  // Reset UI controls
  (document.getElementById('show-libsignal') as HTMLInputElement).checked = true;
  (document.getElementById('show-non-libsignal') as HTMLInputElement).checked = true;
  (document.getElementById('search-input') as HTMLInputElement).value = '';
  (document.getElementById('include-callers') as HTMLInputElement).checked = true;
  (document.getElementById('include-callees') as HTMLInputElement).checked = true;
  (document.getElementById('depth-limit') as HTMLInputElement).value = '0';
  document.getElementById('depth-value')!.textContent = 'All';
  
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

