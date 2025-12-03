# SCIP Call Graph Interactive Viewer

An interactive web-based visualization tool for exploring call graphs generated from SCIP (SCIP Code Intelligence Protocol) data.

## Features

- **Force-Directed Graph** - D3.js physics-based layout for call relationships
- **Source→Sink Path Finding** - Find all paths between two functions
- **Similar Lemmas** - See semantically similar lemmas for each function
- **Node Hiding** - Shift+click to hide nodes, declutter complex graphs
- **Depth Control** - Explore from selected nodes with adjustable depth
- **Real-time Filtering** - All filters apply instantly

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

## Building for Production

```bash
npm run build
```

This creates an optimized production build in the `dist/` directory.

## Usage Guide

### Navigation

- **Zoom:** Mouse wheel or pinch
- **Pan:** Drag background
- **Move Node:** Drag node
- **Select:** Click node
- **Hide:** Shift+click node

### Filtering

#### Source → Sink
- **Source only**: Shows what the function calls (callees)
- **Sink only**: Shows who calls the function (callers)
- **Both**: Shows all paths from source to sink

Use `"quotes"` for exact match (e.g., `"as_bytes"` matches only `as_bytes`, not `lemma_as_bytes_foo`).

#### Depth Limit
Adjust "Max Depth" slider to limit traversal depth. Only applies to source-only or sink-only mode.

#### Node Type
- **Libsignal** (blue): Functions from the libsignal codebase
- **External** (green): Functions from external dependencies

#### Hiding Nodes
**Shift+click** any node to hide it. Hidden nodes appear in the sidebar list—click to unhide, or use "Show All".

### Node Details (Right Sidebar)

- Function name, type, and file location
- GitHub link (if configured)
- Callers and callees lists
- Similar lemmas with similarity scores (if available)
- Code preview

## File Structure

```
web/
├── src/
│   ├── main.ts         # Application entry point
│   ├── graph.ts        # D3.js visualization logic
│   ├── filters.ts      # Filtering and graph algorithms
│   └── types.ts        # TypeScript type definitions
├── index.html          # Main HTML file
├── style.css           # Styles
├── package.json        # Dependencies
├── tsconfig.json       # TypeScript configuration
└── vite.config.js      # Vite bundler configuration
```

## Stack

TypeScript, D3.js v7, Vite, vanilla CSS.

## Tips for Large Graphs

1. Use source/sink filtering to focus on specific paths
2. Shift+click to hide noisy nodes
3. Use depth limit to constrain traversal
4. Use exact match (`"quotes"`) when substring matching is too broad

## Troubleshooting

### Graph won't load
- Ensure the JSON file is in the correct D3 format (use `export_call_graph_d3` binary)
- Check browser console for error messages
- Verify the JSON is valid using a JSON validator

### Performance issues
- Try filtering down the graph size
- Close browser dev tools while visualizing
- Use Chrome/Edge for better performance with large graphs

### Nodes are overlapping
- Wait for the force simulation to settle (a few seconds)
- Manually drag nodes apart
- Increase the collision force in `graph.ts` if needed

## Development

```bash
npm run type-check  # Type checking
npm run dev         # Dev server with HMR
npm run build       # Production build
```

## License

MIT OR Apache-2.0

