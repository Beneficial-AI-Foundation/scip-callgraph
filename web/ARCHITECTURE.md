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
│         │            - Detect libsignal sources            │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │  D3 Exporter │──→ Export to JSON                       │
│  └──────┬───────┘    - Nodes array (with metadata)        │
│         │            - Links array (source → target)       │
│         │            - Metadata (counts, timestamps)       │
└─────────┼─────────────────────────────────────────────────┘
          │
          ▼ call_graph_d3.json
          │
┌─────────┼─────────────────────────────────────────────────┐
│         │           TYPESCRIPT + D3.JS FRONTEND           │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │  JSON Loader │──→ Load graph data                      │
│  └──────┬───────┘                                          │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────┐    ┌──────────────┐                     │
│  │   Filter     │◄──│  UI Controls │                      │
│  │   Engine     │   └──────────────┘                      │
│  └──────┬───────┘    - Source type toggles                │
│         │            - Search input                        │
│         │            - Depth slider                        │
│         │            - Threshold controls                  │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │     D3.js    │──→ Force-directed visualization         │
│  │  Simulation  │    - Physics-based layout               │
│  └──────┬───────┘    - Zoom/pan controls                  │
│         │            - Node dragging                       │
│         │            - Interactive highlighting            │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │  SVG Canvas  │──→ Rendered graph                       │
│  └──────────────┘                                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Export Phase (Rust)

```
SCIP JSON → Parse → Build Graph → Enhance with Metadata → Export D3 JSON
              ↓          ↓                  ↓                    ↓
         Documents   Functions         is_libsignal          nodes[]
         Symbols     Callers/Callees   caller_count          links[]
                                       callee_count          metadata
```

### 2. Visualization Phase (TypeScript)

```
Load JSON → Initial State → User Interaction → Filter Graph → Update D3 → Render
    ↓           ↓                ↓                 ↓            ↓          ↓
Full Graph   Display        Toggle Filter      Apply BFS    Recompute   New SVG
            Statistics      Search Term        Algorithms   Forces      Layout
                           Click Node
```

## Component Breakdown

### Rust Components

#### 1. **Data Structures** (`src/scip_to_call_graph_json.rs`)
```rust
struct D3Node {
    id: String,
    display_name: String,
    symbol: String,
    full_path: String,
    relative_path: String,
    file_name: String,
    parent_folder: String,
    body: Option<String>,
    is_libsignal: bool,    // ← Key for filtering
    caller_count: usize,   // ← Precomputed for performance
    callee_count: usize,
}

struct D3Link {
    source: String,        // Node ID
    target: String,        // Node ID
    type: String,          // "calls"
}

struct D3Graph {
    nodes: Vec<D3Node>,
    links: Vec<D3Link>,
    metadata: D3GraphMetadata,
}
```

#### 2. **Export Function**
```rust
pub fn export_call_graph_d3(
    call_graph: &HashMap<String, FunctionNode>,
    scip_data: &ScipIndex,
    output_path: P,
) -> std::io::Result<()>
```

**Responsibilities:**
- Convert internal call graph to D3 format
- Detect libsignal sources
- Count callers/callees
- Add metadata (timestamps, totals)

### TypeScript Components

#### 1. **Type System** (`src/types.ts`)
```typescript
interface D3Node {
  id: string;
  display_name: string;
  // ... matches Rust struct
  // Plus D3-specific properties:
  x?: number;           // Position (from simulation)
  y?: number;
  vx?: number;          // Velocity
  vy?: number;
  fx?: number | null;   // Fixed position (when dragging)
  fy?: number | null;
}

interface FilterOptions {
  showLibsignal: boolean;
  showNonLibsignal: boolean;
  maxDepth: number | null;
  searchQuery: string;
  minCallerCount: number;
  minCalleeCount: number;
  selectedNodes: Set<string>;
  expandedNodes: Set<string>;
}
```

#### 2. **Filter Engine** (`src/filters.ts`)

**Algorithm: Depth-Limited BFS**
```typescript
function computeDepthFromSelected(
  nodes: D3Node[],
  links: D3Link[],
  selectedNodes: Set<string>,
  maxDepth: number
): Set<string>
```

**Flow:**
1. Build adjacency list (bidirectional)
2. Initialize queue with selected nodes
3. BFS traversal respecting depth limit
4. Return set of reachable nodes

#### 3. **D3 Visualization** (`src/graph.ts`)

**Force Simulation Configuration:**
```typescript
d3.forceSimulation<D3Node>(nodes)
  .force('link', d3.forceLink<D3Node, D3Link>(links)
    .id(d => d.id)
    .distance(100))                // Link length
  .force('charge', d3.forceManyBody()
    .strength(-300))               // Repulsion
  .force('center', d3.forceCenter(width/2, height/2))
  .force('collision', d3.forceCollide()
    .radius(30))                   // Prevent overlap
```

**Update Pattern:**
```
Data Join → Enter Selection → Merge → Update Attributes → Simulation Tick
    ↓             ↓              ↓            ↓                  ↓
Filter Data   New Elements   Combined    Colors, Sizes   Position Updates
```

#### 4. **Main Application** (`src/main.ts`)

**State Management:**
```typescript
let state: GraphState = {
  fullGraph: D3Graph | null,      // Original data
  filteredGraph: D3Graph | null,  // After filters
  filters: FilterOptions,         // Current filter state
  selectedNode: D3Node | null,    // For details panel
  hoveredNode: D3Node | null,     // For highlighting
}
```

**Event Flow:**
```
UI Event → Update State → Apply Filters → Update Visualization → Update UI
   ↓           ↓              ↓                  ↓                    ↓
Click      Modify         Filter Nodes      Recompute Forces      Update
Slider     filters        Filter Links      Restart Simulation    Panels
Search
```

## Key Algorithms

### 1. Graph Filtering
**Complexity:** O(N + E) where N = nodes, E = edges

```typescript
1. Filter nodes by criteria (search, type, counts)
2. Build set of valid node IDs
3. Filter edges to only connect valid nodes
4. If depth filtering:
   a. Run BFS from selected nodes
   b. Keep only nodes within depth limit
```

### 2. Force Simulation
**Complexity:** O(N² + E) per iteration (using Barnes-Hut optimization)

```typescript
Each tick:
1. Apply link forces (edges pull nodes together)
2. Apply charge forces (nodes repel each other)
3. Apply centering force
4. Apply collision detection
5. Update positions
6. Update rendering
```

### 3. Interactive Highlighting
**Complexity:** O(E) for finding connected nodes

```typescript
On hover:
1. Find all links where source or target is hovered node
2. Extract connected node IDs
3. Fade non-connected nodes and links
4. Highlight connected ones
```

## Performance Considerations

### Rust Export
- **Fast:** O(N + E) single pass through graph
- **Memory:** Entire graph in memory (fine for <100K nodes)
- **Output:** Pretty-printed JSON (readable but larger)

### TypeScript Visualization
- **Loading:** O(N + E) JSON parse
- **Filtering:** O(N + E) per filter change (fast for <5K nodes)
- **Rendering:** Depends on D3 simulation
  - Manageable: <1000 nodes
  - Slow: 1000-5000 nodes
  - Challenging: >5000 nodes

**Optimizations for large graphs:**
- Start with filters applied
- Use depth limiting
- Increase thresholds
- Consider canvas rendering (instead of SVG)

## Technology Choices Rationale

### Why TypeScript over JavaScript?
- **Type safety** catches bugs at compile time
- **Better tooling** (autocomplete, refactoring)
- **Self-documenting** interfaces
- **D3.js** has excellent type definitions

### Why D3.js over other libraries?
- **Industry standard** for network graphs
- **Flexible** force simulation
- **Fine-grained control** over rendering
- **Large community** and ecosystem

### Why Vite over Webpack?
- **Fast HMR** (hot module replacement)
- **Simple config** (almost zero-config)
- **Modern** ES modules by default
- **Great TypeScript** support

### Why JSON over GraphML/GEXF?
- **Simple** to parse and generate
- **Human-readable** for debugging
- **JavaScript native** format
- **Flexible** schema

## Extension Points

### Adding New Filters
1. Add property to `FilterOptions` interface
2. Implement filter logic in `applyFilters()`
3. Add UI control in `index.html`
4. Add event handler in `main.ts`

### Adding New Node Metadata
1. Add field to Rust `D3Node` struct
2. Export data in `export_call_graph_d3()`
3. Add field to TypeScript `D3Node` interface
4. Use in visualization or filtering

### Custom Visualizations
Extend `graph.ts`:
- Modify force parameters
- Change node sizing/coloring logic
- Add new interaction modes
- Implement different layouts

### Data Sources
Currently: SCIP JSON
Potential: Language Server Protocol, ctags, custom parsers

---

**Architecture designed for:** Extensibility, Performance, Type Safety, Developer Experience

