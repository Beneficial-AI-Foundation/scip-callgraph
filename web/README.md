# SCIP Call Graph Interactive Viewer

An interactive web-based visualization tool for exploring call graphs generated from SCIP (SCIP Code Intelligence Protocol) data. Designed for Verus verification projects with support for function modes, verification status, and source→sink path finding.

## Features

- **Three Visualization Views** - Call Graph, Blueprint, and Crate Map
- **Source→Sink Path Finding** - Find all paths between two functions
- **Crate Frontier** - See which functions in crate A are called by crate B
- **Verus Support** - Filter by function mode (exec/proof/spec) and call location (body/requires/ensures)
- **Verification Status** - Color-coded nodes showing verified/failed/unverified status
- **Similar Lemmas** - See semantically similar lemmas for each function
- **Node Hiding** - Shift+click to hide nodes, declutter complex graphs
- **Depth Control** - Explore from selected nodes with adjustable depth
- **Shareable URLs** - Copy link to share current filter state
- **VS Code Integration** - Can run as VS Code webview panel

## Quick Start

### 1. Install Dependencies

```bash
cd web
npm install
```

### 2. Start Development Server

```bash
npm run dev
```

This will start a Vite development server (usually at `http://localhost:3000`).

### 3. Generate Call Graph Data

**Option A: Use the automated script (easiest)**
```bash
# From Rust project
../QUICKSTART_INTERACTIVE.sh /path/to/rust/project

# Or from existing SCIP JSON
../QUICKSTART_INTERACTIVE.sh /path/to/index_scip.json
```

**Option B: Manual generation**
```bash
# Build the project first
cargo build --workspace --release

# Export call graph from SCIP JSON
cargo run -p metrics-cli --bin export_call_graph_d3 -- <input_scip.json> -o call_graph_d3.json
```

### 4. Load the Graph

**Option A: Automatic (via quickstart script)**

When using `../QUICKSTART_INTERACTIVE.sh`, the graph is automatically loaded! The script:
1. Copies the generated JSON to `public/graph.json`
2. The web app auto-fetches it on startup
3. You'll see a success message when loaded

**Option B: Manual file selection**

1. Open the web viewer in your browser
2. Click "📁 Load Graph JSON"
3. Select the generated `call_graph_d3.json` file
4. Explore!

**Option C: URL parameter**

Load from any URL:
```
http://localhost:3000/?json=https://example.com/graph.json
```

## Building for Production

```bash
npm run build
```

This creates an optimized production build in the `dist/` directory.

## Usage Guide

### Views

The viewer offers three visualization modes, switched via the header buttons:

#### Call Graph (default)
Force-directed D3.js layout with topological layering. Best for exploring function-level call paths, source-to-sink analysis, and depth-limited exploration.

#### Blueprint
Dagre hierarchical layout that groups functions by file. Each file is a compound box containing its functions as nodes, with cross-file edges drawn between them. Best for understanding module-level structure.

#### Crate Map
High-level view where each crate is a single box showing function and file counts. Edges between crates are weighted by the number of cross-crate function calls. Best for understanding inter-crate dependencies at a glance.

- **Click** a crate to select it as source (blue) or target (orange) for the crate frontier
- **Double-click** a crate to switch to Call Graph view filtered to that crate's files
- **Click** a cross-crate edge to expand it inline, showing the individual function calls
- Press **Escape** to collapse back to the crate overview

### Navigation

- **Zoom:** Mouse wheel or pinch
- **Pan:** Drag background
- **Move Node:** Drag node (Call Graph view)
- **Select:** Click node
- **Hide:** Shift+click node

### Filtering

#### Source → Sink
- **Source only**: Shows what the function calls (callees)
- **Sink only**: Shows who calls the function (callers)  
- **Both**: Shows all paths from source to sink

**Query Syntax:**
- `decompress` - exact match on function name
- `*decomp*` - contains "decomp" anywhere
- `lemma_*` - starts with "lemma_"
- `edwards::decompress` - function "decompress" in file "edwards.rs"
- `crate:curve25519-dalek` - all functions in the named crate

When both source and sink use `crate:` queries, a **direct boundary mode** activates that shows only the cross-crate function calls between the two crates.

#### Crate Frontier
Select a **Source Crate** and **Target Crate** from the sidebar dropdowns (or by clicking crates in the Crate Map) to see the frontier: which functions in the source crate are called by the target crate.

When a source crate is selected, the target dropdown is automatically filtered to only show crates that the source actually calls into.

In Crate Map view the frontier is rendered inline. A "View in Call Graph" button lets you open the same frontier in the full Call Graph for deeper exploration.

#### Function Mode (Verus)
- **Exec** (blue): Executable code (default Rust functions)
- **Proof** (purple): Proof functions and lemmas
- **Spec** (orange): Specification functions (hidden by default)

#### Call Types
- **Body Calls**: Dependencies from function body (shown by default)
- **Requires**: Dependencies from `requires` clauses
- **Ensures**: Dependencies from `ensures` clauses

#### Exclude/Include Patterns
- **Exclude by Name**: Hide functions matching glob patterns (e.g., `*_comm*, lemma_mul_*`)
- **Exclude by Path**: Hide functions in certain paths (e.g., `*/specs/*`)
- **Include Files**: Only show functions from specific files

#### Depth Limit
Adjust "Max Depth" slider to limit traversal depth. Only applies to source-only or sink-only mode.

#### Hiding Nodes
**Shift+click** any node to hide it. Hidden nodes appear in the sidebar list—click to unhide, or use "Show All".

### Node Details (Right Sidebar)

- Function name, mode, and file location
- Verification status badge
- GitHub link (if configured)
- Callers and callees lists (filtered items shown dimmed)
- Similar lemmas with similarity scores (if available)

### Verification Status (Node Colors)

- **Green** (✓): Verified by Verus
- **Red** (✗): Verification failed
- **Grey** (○): Not yet verified
- **Blue** (?): Unknown/no verification data

### URL Parameters

Share specific views with URL parameters:

| Parameter | Description | Example |
|-----------|-------------|---------|
| `json` | Load graph from URL | `?json=https://...` |
| `github` | GitHub repo for source links | `?github=https://github.com/user/repo` |
| `prefix` | Path prefix for GitHub links | `?prefix=crate-name` |
| `view` | Active view | `?view=crate-map` or `?view=blueprint` |
| `source` | Source query | `?source=main` |
| `sink` | Sink query | `?sink=validate` |
| `source-crate` | Source crate for frontier | `?source-crate=libsignal-core` |
| `target-crate` | Target crate for frontier | `?target-crate=curve25519-dalek` |
| `depth` | Max depth | `?depth=3` |
| `exec`, `proof`, `spec` | Toggle modes | `?spec=1&exec=0` |
| `inner`, `pre`, `post` | Toggle call types | `?pre=1&post=1` |
| `hidden` | Hidden node names | `?hidden=foo,bar` |

## File Structure

```
web/
├── src/
│   ├── main.ts         # Application entry, state management, VS Code integration
│   ├── graph.ts        # Call Graph view - D3.js force-directed visualization
│   ├── blueprint.ts    # Blueprint view - Dagre hierarchical layout grouped by file
│   ├── crate-map.ts    # Crate Map view - crate-level overview with frontier rendering
│   ├── filters.ts      # Filtering algorithms, path finding, crate boundary mode
│   ├── status.ts       # Verification status computation
│   └── types.ts        # TypeScript type definitions (D3Node, CrateGraph, etc.)
├── index.html          # Main HTML file
├── style.css           # Styles
├── public/             # Static assets, auto-loaded graph.json
├── package.json        # Dependencies
├── tsconfig.json       # TypeScript configuration
├── vite.config.js      # Vite bundler configuration (web)
└── vite.config.vscode.js  # Vite config for VS Code webview build
```

## Stack

TypeScript, D3.js v7, Vite, vanilla CSS.

## Tips for Large Graphs

1. **Use source/sink filtering** to focus on specific paths
2. **Shift+click** to hide noisy nodes
3. **Use depth limit** to constrain traversal
4. **Exclude patterns** to hide common lemmas (e.g., `*_comm*`)
5. **Include files** to focus on specific modules
6. For very large files (>5MB), the viewer will prompt before loading

## Troubleshooting

### Graph won't load
- Ensure the JSON file is in the correct D3 format (use `export_call_graph_d3` binary)
- Check browser console for error messages
- Verify the JSON is valid using a JSON validator

### Performance issues
- Try filtering down the graph size with source/sink queries
- Close browser dev tools while visualizing
- Use Chrome/Edge for better performance with large graphs
- Results are auto-limited to 200 nodes for performance

### Nodes are overlapping
- Wait for the force simulation to settle (a few seconds)
- Manually drag nodes apart
- The topological layout should create horizontal layers

### "Large Graph Detected" message
- Enter a source or sink query before loading
- Press Enter or click "Load & Search" to load with filters applied

## Development

```bash
npm run type-check  # Type checking
npm run dev         # Dev server with HMR
npm run build       # Production build (web)
npm run build:vscode  # Build for VS Code webview
```

## License

MIT OR Apache-2.0
