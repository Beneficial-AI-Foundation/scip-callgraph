# Interactive Call Graph Viewer - Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      RUST BACKEND                           │
│                                                             │
│  ┌──────────────┐                                          │
│  │ SCIP Parser  │──→ Parse index_scip.json                │
│  └──────┬───────┘                                          │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │  Call Graph  │──→ Build graph with nodes & edges       │
│  │   Builder    │    - Extract function symbols            │
│  └──────┬───────┘    - Identify callers/callees           │
│         │            - Detect Verus function modes         │
│         ▼            - Classify call locations             │
│  ┌──────────────┐                                          │
│  │  D3 Exporter │──→ Export to JSON                       │
│  └──────┬───────┘    - Nodes array (with metadata)        │
│         │            - Links array (source → target)       │
│         │            - Link types (inner/pre/post)         │
│         │            - Metadata (counts, timestamps)       │
└─────────┼─────────────────────────────────────────────────┘
          │
          ▼ call_graph_d3.json
          │
┌─────────┼─────────────────────────────────────────────────┐
│         │           TYPESCRIPT + D3.JS FRONTEND           │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │  JSON Loader │──→ Load graph data (auto or manual)     │
│  └──────┬───────┘    - URL parameter support              │
│         │            - Large file detection               │
│         ▼                                                   │
│  ┌──────────────┐    ┌──────────────┐                     │
│  │   Filter     │◄──│  UI Controls │                      │
│  │   Engine     │   └──────────────┘                      │
│  └──────┬───────┘    - Source/sink queries                │
│         │            - Function mode toggles              │
│         │            - Call type toggles                  │
│         │            - Exclude/include patterns           │
│         │            - Depth slider                       │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │     D3.js    │──→ Topological force-directed layout    │
│  │  Simulation  │    - Layered by call depth              │
│  └──────┬───────┘    - Zoom/pan controls                  │
│         │            - Node dragging                       │
│         │            - Interactive highlighting            │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │  SVG Canvas  │──→ Rendered graph                       │
│  └──────────────┘                                          │
│                                                             │
│  ┌──────────────┐                                          │
│  │  VS Code     │──→ Optional webview integration         │
│  │  Integration │    - Bidirectional messaging            │
│  └──────────────┘    - Navigate to source files           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Export Phase (Rust)

```
SCIP JSON → Parse → Build Graph → Enhance with Metadata → Export D3 JSON
              ↓          ↓                  ↓                    ↓
         Documents   Functions         is_libsignal          nodes[]
         Symbols     Callers/Callees   function mode          links[]
                     Call locations    verification_status    metadata
```

### 2. Visualization Phase (TypeScript)

```
Load JSON → Initial State → User Interaction → Filter Graph → Update D3 → Render
    ↓           ↓                ↓                 ↓            ↓          ↓
Full Graph   Display        Source/Sink       Path Finding   Recompute   New SVG
            Statistics      Depth Slider      BFS/DFS        Forces      Layout
                           Hide Node
```

## Component Breakdown

### Rust Components

#### 1. **Data Structures** (`crates/scip-core/src/scip_to_call_graph_json.rs`)
```rust
/// Verus function modes
enum FunctionMode {
    Exec,   // Executable code (default)
    Proof,  // Proof functions (lemmas)
    Spec,   // Specification functions
}

struct D3Node {
    id: String,
    display_name: String,
    symbol: String,
    full_path: String,
    relative_path: String,
    file_name: String,
    parent_folder: String,
    start_line: Option<usize>,   // Line range for source navigation
    end_line: Option<usize>,
    is_libsignal: bool,
    dependencies: Vec<String>,   // Outgoing edges (what I call)
    dependents: Vec<String>,     // Incoming edges (who calls me)
    mode: FunctionMode,          // Verus function mode
    verification_status: Option<String>,  // verified/failed/unverified
    similar_lemmas: Option<Vec<SimilarLemma>>,
}

struct D3Link {
    source: String,        // Node ID
    target: String,        // Node ID
    type: String,          // "inner" | "precondition" | "postcondition"
}

struct D3Graph {
    nodes: Vec<D3Node>,
    links: Vec<D3Link>,
    metadata: D3GraphMetadata,
}
```

#### 2. **Export Function**
```rust
pub fn atoms_to_d3_graph(
    atoms: &HashMap<String, AtomWithLines>,
    call_graph: &HashMap<String, FunctionNode>,
    project_root: &str,
    github_url: Option<String>,
) -> D3Graph
```

**Responsibilities:**
- Convert internal call graph to D3 format
- Detect Verus function modes (exec/proof/spec)
- Pre-compute dependencies and dependents for O(1) browser lookups
- Classify call locations (body, requires, ensures)
- Add metadata (timestamps, totals, GitHub URL)

### TypeScript Components

#### 1. **Type System** (`src/types.ts`)
```typescript
type FunctionMode = 'exec' | 'proof' | 'spec';
type VerificationStatus = 'verified' | 'failed' | 'unverified';
type LinkType = 'inner' | 'precondition' | 'postcondition';

interface D3Node {
  id: string;
  display_name: string;
  symbol: string;
  full_path: string;
  relative_path: string;
  file_name: string;
  parent_folder: string;
  start_line?: number;
  end_line?: number;
  is_libsignal: boolean;
  dependencies: string[];    // Pre-computed for O(1) lookup
  dependents: string[];
  mode: FunctionMode;
  verification_status?: VerificationStatus;
  similar_lemmas?: SimilarLemma[];
  // D3-specific properties (added during simulation):
  x?: number;
  y?: number;
  vx?: number;
  vy?: number;
  fx?: number | null;
  fy?: number | null;
}

interface FilterOptions {
  // Source type filters
  showLibsignal: boolean;
  showNonLibsignal: boolean;
  // Call type filters
  showInnerCalls: boolean;         // Body calls (default: true)
  showPreconditionCalls: boolean;  // requires clauses (default: false)
  showPostconditionCalls: boolean; // ensures clauses (default: false)
  // Function mode filters (Verus)
  showExecFunctions: boolean;      // Executable functions (default: true)
  showProofFunctions: boolean;     // Proof/lemma functions (default: true)
  showSpecFunctions: boolean;      // Spec functions (default: false)
  // Pattern-based exclusion
  excludeNamePatterns: string;     // Glob patterns for function names
  excludePathPatterns: string;     // Glob patterns for file paths
  includeFiles: string;            // Only show functions from these files
  // Graph traversal
  maxDepth: number | null;
  sourceQuery: string;             // Source nodes (shows callees)
  sinkQuery: string;               // Sink nodes (shows callers)
  // Selection state
  selectedNodes: Set<string>;
  expandedNodes: Set<string>;
  hiddenNodes: Set<string>;        // User-hidden nodes (Shift+click)
}
```

#### 2. **Filter Engine** (`src/filters.ts`)

**Query Syntax:**
- Substring: `decompress` matches any function containing "decompress"
- Glob patterns: `*decomp*` for contains, `lemma_*` for prefix
- Path-qualified: `edwards::decompress` matches decompress in edwards.rs

**Algorithm: Source→Sink Path Finding**
```typescript
function findPathNodes(
  graph: D3Graph,
  sourceIds: Set<string>,
  sinkIds: Set<string>,
  maxDepth: number | null
): Set<string>
```

Uses DFS with backtracking to find all nodes on any path from sources to sinks.

**Algorithm: Depth-Limited BFS**
```typescript
function getCalleesRecursive(
  graph: D3Graph,
  startNodeId: string,
  maxDepth: number | null
): DepthTraversalResult  // { nodeIds, nodeDepths }
```

#### 3. **D3 Visualization** (`src/graph.ts`)

**Topological Layout:**
The graph uses a layered layout based on call depth:
1. Compute topological depth for each node (roots at depth 0)
2. Use `forceX` to keep nodes at their depth layer
3. Use `forceY` for vertical spread within layers
4. Standard collision and charge forces for spacing

**Node Coloring (by verification status):**
- ✓ Verified: Green (#22c55e)
- ✗ Failed: Red (#ef4444)
- ○ Unverified: Grey (#9ca3af)
- ? Unknown: Blue (#3b82f6)

**Link Styling (by call type):**
- Inner (body): Solid grey line
- Precondition (requires): Dashed orange line
- Postcondition (ensures): Dashed pink line

#### 4. **Main Application** (`src/main.ts`)

**State Management:**
```typescript
let state: GraphState = {
  fullGraph: D3Graph | null,      // Original unfiltered data
  filteredGraph: D3Graph | null,  // After all filters applied
  filters: FilterOptions,         // Current filter state
  selectedNode: D3Node | null,    // For details panel
  hoveredNode: D3Node | null,     // For highlighting
}
```

**URL Integration:**
- Shareable URLs with filter state encoded
- `?json=URL` to load graph from external URL
- `?github=URL` to set GitHub base for source links
- `?source=query&sink=query&depth=N` for filter presets

#### 5. **VS Code Integration** (`src/main.ts`)

When running as a VS Code webview:
- Receives graph data via `postMessage`
- Sends navigation requests back to extension
- Supports exact node ID matching for precise function selection
- Hides file upload UI (data comes from extension)

## Key Algorithms

### 1. Source → Sink Path Finding
**Complexity:** O(N + E) with DFS

```typescript
1. Build forward adjacency (caller → callees)
2. For each source, run DFS:
   a. Track current path
   b. When sink reached, mark all path nodes
   c. Backtrack and continue exploration
3. Return all nodes on any valid path
```

### 2. Graph Filtering Pipeline
**Complexity:** O(N + E)

```typescript
1. Pre-filter: Build set of mode-allowed nodes
   - Apply function mode filters (exec/proof/spec)
   - Apply exclude name/path patterns
   - Apply include file patterns
   - Exclude hidden nodes
2. Source/Sink traversal (if queries provided)
   - Source only: BFS forward for callees
   - Sink only: BFS backward for callers
   - Both: DFS path finding
3. Apply remaining filters (libsignal, call types)
4. Remove isolated nodes (no edges)
5. Filter links to valid node pairs
```

### 3. Topological Depth Computation
**Complexity:** O(N + E)

```typescript
1. Find root nodes (no incoming edges)
2. BFS from roots, assigning increasing depth
3. Handle cycles by using lowest in-degree as root
4. Return Map<nodeId, depth>
```

## Performance Considerations

### Large Graph Handling
- **Deferred loading:** Files > 5MB prompt user before loading
- **Result limiting:** Maximum 200 nodes rendered to prevent freeze
- **Debounced search:** 300ms delay on keystroke for large graphs
- **Link threshold:** Graphs > 10K links require filter before rendering

### Optimizations
- Pre-computed `dependencies` and `dependents` arrays (O(1) lookup)
- Path-based link filtering (only show edges "on the path")
- Deep copy of nodes/links prevents D3 mutation of original data

## VS Code Integration

The viewer can run as a VS Code webview panel:

```typescript
// Extension sends graph data
webview.postMessage({
  type: 'loadGraph',
  graph: graphData,
  selectedNodeId: 'scip:...#function()',  // Exact match
  initialQuery: { source: 'function_name', depth: 2 }
});

// Webview requests navigation
vscode.postMessage({
  type: 'navigate',
  relativePath: 'src/lib.rs',
  startLine: 42,
  endLine: 50
});
```

## Extension Points

### Adding New Filters
1. Add property to `FilterOptions` interface in `types.ts`
2. Implement filter logic in `applyFilters()` in `filters.ts`
3. Add UI control in `index.html`
4. Add event handler in `main.ts`
5. Update URL generation/parsing for shareable links

### Adding New Node Metadata
1. Add field to Rust `D3Node` struct
2. Export data in converter function
3. Add field to TypeScript `D3Node` interface
4. Use in visualization (node info panel, coloring, etc.)

### Custom Visualizations
Extend `graph.ts`:
- Modify force parameters in `update()`
- Change `getNodeColor()` for different coloring schemes
- Add new interaction modes in click/hover handlers
- Implement alternative layouts

---

**Architecture designed for:** Extensibility, Performance, Type Safety, Developer Experience

**Last updated:** December 2024
