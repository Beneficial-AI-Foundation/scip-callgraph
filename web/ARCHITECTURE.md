# Interactive Call Graph Viewer - Architecture

## System Overview

The viewer consumes JSON produced by **language-specific probes** — standalone
tools that extract call graphs and verification metadata from formally verified
codebases.  Two probes are currently supported:

| Probe | Language | Repository |
|-------|----------|------------|
| **probe-verus** | Verus / Rust | [Beneficial-AI-Foundation/probe-verus](https://github.com/Beneficial-AI-Foundation/probe-verus) |
| **probe-lean** | Lean 4 | [Beneficial-AI-Foundation/probe-lean](https://github.com/Beneficial-AI-Foundation/probe-lean) |

```
┌─────────────────────────────────────────────────────────────┐
│                   LANGUAGE-SPECIFIC PROBES                  │
│                                                             │
│  ┌───────────────┐          ┌──────────────┐                │
│  │  probe-verus  │          │  probe-lean  │                │
│  │ (Rust / Verus)│          │  (Lean 4)    │                │
│  └──────┬────────┘          └──────┬───────┘                │
│         │                          │                        │
│         │  SCIP index → atoms      │  Lean env → atoms      │
│         ▼                          ▼                        │
│  ┌────────────────────────────────────────┐                 │
│  │          Probe Atom Dict JSON          │                 │
│  │  (or Schema 2.0 envelope with atoms)   │                 │
│  │                                        │                 │
│  │  { "probe:fn_name": {                  │                 │
│  │      "display-name": "fn_name",        │                 │
│  │      "dependencies": [...],            │                 │
│  │      "code-path": "src/lib.rs",        │                 │
│  │      "code-text": { lines-start, … },  │                 │
│  │      "kind": "exec" | "theorem" | …,   │                 │
│  │      "verification-status": "verified" │                 │
│  │    }, …                                │                 │
│  │  }                                     │                 │
│  └──────────────────┬─────────────────────┘                 │
│                     │ graph.json                            │
└─────────────────────┼───────────────────────────────────────┘
                      │
          OR  ────────┼──── probe-verus Rust pipeline
                      │     also emits D3 format directly
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              TYPESCRIPT + D3.JS FRONTEND                    │
│                                                             │
│  ┌──────────────┐                                           │
│  │ Graph Loader │──→ Normalize probe output to D3Graph      │
│  │              │    - Atom dict → convertAtomDictToD3Graph │
│  │              │    - Schema 2.0 envelope → unwrap + recurse│
│  │              │    - D3Graph → pass-through               │
│  └──────┬───────┘    - URL parameter / file upload          │
│         │            - Large file detection                 │
│         ▼                                                   │
│  ┌──────────────┐   ┌───────────────┐                       │
│  │    Query     │◄──│  UI Controls  │                       │
│  │   Pipeline   │   └───────────────┘                       │
│  └──────┬───────┘    - Source/sink queries                  │
│         │            - Function mode toggles                │
│         │            - Call type toggles                    │
│         │            - Exclude/include patterns             │
│         │            - Depth slider                         │
│         ▼                                                   │
│  ┌──────────────┐                                           │
│  │  View Layer  │──→ Three visualization modes              │
│  │              │    - Call Graph (D3 force-directed)       │
│  │              │    - File Map (Dagre, grouped by file)    │
│  │              │    - Crate Map (Dagre, crate-level)       │
│  └──────┬───────┘    - Zoom/pan controls                    │
│         │            - Interactive highlighting             │
│         ▼                                                   │
│  ┌──────────────┐                                           │
│  │  SVG Canvas  │──→ Rendered graph                         │
│  └──────────────┘                                           │
│                                                             │
│  ┌──────────────┐                                           │
│  │  VS Code     │──→ Optional webview integration           │
│  │  Integration │    - Bidirectional messaging              │
│  └──────────────┘    - Navigate to source files             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Probe Phase (Language-Specific)

Each probe extracts a call graph from its target language and writes a JSON file.

**Verus / Rust path** (probe-verus):
```
Source .rs → verus-analyzer scip → index.scip → scip print --json → index.scip.json
                                                                          ↓
                                              probe-verus (Rust library)
                                              parse_scip_json → build_call_graph
                                              → convert_to_atoms → atoms_to_d3_graph
                                                                          ↓
                                                                    graph.json (D3 format)
```

**Lean path** (probe-lean):
```
Source .lean → probe-lean extract → graph.json (atom dict or Schema 2.0 envelope)
```

Both paths produce a JSON file consumed by the web viewer.

### 2. Visualization Phase (TypeScript)

```
Load JSON → Normalize → Initial State → User Interaction → Filter → View Dispatch → Render
    ↓           ↓            ↓                ↓               ↓           ↓            ↓
graph.json  Detect       Full Graph      Source/Sink     Path Finding  Call Graph   New SVG
            format:      + crate_name    Depth Slider    BFS/DFS      File Map     Layout
            atom dict    backfill        Crate Frontier  Boundary      Crate Map
            / D3 / 2.0                   Hide Node
```

## Input Formats

The web viewer accepts three JSON formats, all auto-detected by the graph loader:

### 1. Probe Atom Dict (primary input from probes)

The native output of probe-verus and probe-lean. A flat object keyed by atom ID:

```json
{
  "probe:fn_name": {
    "display-name": "fn_name",
    "dependencies": ["probe:other_fn"],
    "code-path": "src/lib.rs",
    "code-text": { "lines-start": 10, "lines-end": 25 },
    "code-module": "my_crate::module",
    "kind": "exec",
    "verification-status": "verified",
    "dependencies-with-locations": [
      { "code-name": "probe:other_fn", "location": "inner", "line": 15 }
    ]
  }
}
```

### 2. Schema 2.0 Envelope (probe output with metadata)

Wraps an atom dict with provenance metadata. The loader unwraps and recurses:

```json
{
  "schema": "probe-output",
  "schema-version": "2.0",
  "tool": { "name": "probe-lean", "version": "0.3.0", "command": "extract" },
  "source": { "repo": "...", "commit": "...", "language": "lean", ... },
  "timestamp": "2026-03-10T...",
  "data": { /* atom dict */ }
}
```

### 3. D3Graph (legacy / direct Rust pipeline output)

The Verus Rust pipeline (`atoms_to_d3_graph`) can emit this format directly:

```json
{
  "nodes": [{ "id": "...", "display_name": "...", ... }],
  "links": [{ "source": "...", "target": "...", "type": "inner" }],
  "metadata": { "total_nodes": 42, "total_edges": 60, ... }
}
```

## Component Breakdown

### Rust Components (probe-verus pipeline)

The Rust backend is used only on the **Verus path**. probe-lean writes its JSON
directly; no Rust processing is needed.

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
- Convert probe-verus atoms to D3 format
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

#### 2. **Crate-Level Types** (`src/types.ts`)

In addition to `D3Node` and `D3Link`, the type system includes crate-level aggregation types:

```typescript
interface CrateNode {
  name: string;
  functionCount: number;
  fileCount: number;
  nodeIds: string[];
  isExternal: boolean;
}

interface CrateEdge {
  source: string;          // Source crate name
  target: string;          // Target crate name
  callCount: number;
  calls: Array<{ sourceId: string; targetId: string; type: string }>;
}

interface CrateGraph {
  nodes: CrateNode[];
  edges: CrateEdge[];
}
```

Each `D3Node` also carries a `crate_name` field, backfilled on load via `extractCrateName()` which parses the crate name from the node's SCIP/probe ID prefix.

#### 3. **Query Pipeline** (`src/query.ts`)

The filter/traversal system is a composable pipeline following a compile → execute pattern:

```
FilterOptions → compileQuery() → CompiledQuery → executeQuery() → D3Graph
```

- **Compiler** (`compileQuery`): Pure function that translates `FilterOptions` into a `CompiledQuery` containing a `GraphQuery` AST (7 discriminated-union variants), traversal predicates, display predicates, focus config, and link-type filter. No graph access.
- **Executor** (`executeQuery`): 7-step pipeline that evaluates the compiled query against a full graph:
  1. Build traversable subgraph (traversal predicates)
  2. Resolve node matchers (patterns → concrete IDs)
  3. Dispatch traversal (BFS/DFS/boundary scan)
  4. Assemble result nodes
  5. Apply display predicates
  6. Filter links (endpoint, depth-tree, link-type)
  7. Remove isolated nodes, build depth metadata
- **9 operators**: Pure functions (`selectNodes`, `traverseForward`, `traverseBackward`, `traverseBidirectional`, `findPaths`, `crateBoundary`, `filterLinksByType`, `depthFilterLinks`, `removeIsolated`).

The public entry point is `applyFilters()` in `filters.ts`, which calls `compileQuery` then `executeQuery`. See `QUERY_PIPELINE.md` for full details.

#### 3a. **Pattern Utilities** (`src/filters.ts`)

Provides query-matching functions used by the pipeline's resolver:

- `matchesQuery()`: Substring, glob, path-qualified (`edwards::decompress`), and crate-qualified (`crate:name`) matching
- `globToRegex()`: Convert glob patterns to anchored regexes
- `pathPatternToRegex()`: Convert path patterns with `**` / `*` / `?` to regexes

Also exports `getCallers()` and `getCallees()` for immediate (non-recursive) neighbor lookup.

#### 3b. **Graph Loader** (`src/graph-loader.ts`)

The central adapter that normalizes probe output into a unified `D3Graph`.
`parseAndNormalizeGraph()` auto-detects the input format and dispatches:

- **Probe atom dict** → `convertAtomDictToD3Graph()` (primary path for probe-verus / probe-lean)
- **Schema 2.0 envelope** → unwrap `data` payload, recurse
- **D3Graph format** → direct pass-through (legacy Rust pipeline output)
- **Simplified format** → `convertSimplifiedToD3Graph()` (array of nodes with `deps`)

#### 4. **D3 Visualization** (`src/graph.ts`)

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

#### 4b. **File Map View** (`src/blueprint.ts`)

Dagre-based hierarchical layout that groups functions by file using compound graph nodes. Each file becomes a box containing its function nodes, with cross-file edges drawn between them. Uses the same node coloring as Call Graph but provides a cleaner file-level view.

#### 4c. **Crate Map View** (`src/crate-map.ts`)

Aggregates the function-level graph into a crate-level overview:

- `buildCrateGraph()`: Transforms `D3Graph` into `CrateGraph` by grouping nodes by `crate_name` and counting cross-crate edges.
- **Collapsed mode**: Renders crates as boxes with function/file counts, edges weighted by call count.
- **Edge expansion**: Clicking a cross-crate edge expands both crates inline to show the individual function calls, using Dagre compound layout.
- **Crate frontier**: Selecting two crates (by click or dropdown) renders an inline view of all functions at their interface, with a "View in Call Graph" navigation button.
- **Dependency-aware dropdowns**: Bidirectional filtering — when a source crate is selected, the target dropdown is filtered to only crates the source actually calls into; conversely, when a target crate is selected, the source dropdown is filtered to only crates that call into the target (both derived from `CrateGraph` edges).

#### 5. **Main Application** (`src/main.ts`)

**State Management:**
```typescript
let state: GraphState = {
  fullGraph: D3Graph | null,      // Original unfiltered data
  filteredGraph: D3Graph | null,  // After all filters applied
  filters: FilterOptions,         // Current filter state
  selectedNode: D3Node | null,    // For details panel
  hoveredNode: D3Node | null,     // For highlighting
}

// View management
let activeView: 'callgraph' | 'blueprint' | 'crate-map';
let visualization: CallGraphVisualization | BlueprintVisualization | CrateMapVisualization;
let crateDependencyMap: Map<string, Set<string>>;         // source crate → set of target crates
let crateReverseDependencyMap: Map<string, Set<string>>;  // target crate → set of source crates
```

**URL Integration:**
- Shareable URLs with filter state encoded
- `?json=URL` to load graph from external URL
- `?github=URL` to set GitHub base for source links
- `?view=crate-map` or `?view=blueprint` to set the active view
- `?source=query&sink=query&depth=N` for filter presets
- `?source-crate=A&target-crate=B` for crate frontier selection

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

### 2. Query Pipeline (7 Steps)
**Complexity:** O(N + E) per step

```
1. selectNodes        — traversal predicates → traversable subgraph
2. resolveNodeMatcher — patterns → concrete IDs (full graph, then intersect traversable)
3. dispatch traversal — BFS / DFS / boundary scan depending on GraphQuery type
4. assemble nodes     — filter fullGraph.nodes to traversal result
5. display predicates — libsignal toggle, re-apply kind & hidden filters
6. filter links       — endpoint, depth-tree, link-type passes
7. cleanup            — remove isolated nodes, build nodeDepths metadata
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
2. Add the predicate to `TraversalPredicates` or `DisplayPredicates` in `query.ts`
3. Wire it in `compileQuery()` (compiler) and apply it in `executeQuery()` (executor)
4. Add UI control in `index.html`
5. Add event handler in `main.ts`
6. Update URL generation/parsing for shareable links

### Adding New Node Metadata
1. Add field to Rust `D3Node` struct (Verus path) or `ProbeAtom` interface (all probes)
2. Export data in converter function (`atoms_to_d3_graph` / `convertAtomDictToD3Graph`)
3. Add field to TypeScript `D3Node` interface
4. Use in visualization (node info panel, coloring, etc.)

### Supporting a New Probe
1. Have the probe emit **atom dict JSON** (or a Schema 2.0 envelope wrapping one)
2. Add any new fields to `ProbeAtom` in `types.ts`
3. Map the new fields to `D3Node` in `convertAtomDictToD3Graph()` (`graph-loader.ts`)
4. Add a CI workflow (see `generate-lean-callgraph.yml` as a template)

### Custom Visualizations
Three views exist — Call Graph (`graph.ts`), File Map (`blueprint.ts`), and Crate Map (`crate-map.ts`) — each implementing `update()`, `destroy()`, `resize()`, `clear()`, and `highlightNodes()`. To add a new view:
1. Create a new class following the same interface pattern
2. Add a button in `index.html` and a case in `createVisualization()` in `main.ts`
3. Register the view name in the `ActiveView` type and URL parameter handling

---

**Architecture designed for:** Extensibility, Performance, Type Safety, Developer Experience

**Last updated:** March 2026
