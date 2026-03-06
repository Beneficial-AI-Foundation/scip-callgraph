# Interactive Call Graph Viewer

The SCIP Call Graph project now includes an interactive web-based viewer for exploring call graphs dynamically!

## Overview

Instead of generating static SVG files for each filtering scenario, you can now:

1. **Export the full call graph once** as JSON
2. **Explore it interactively** in a web browser with real-time filtering
3. **Dynamically adjust** filters without re-running Rust scripts

## Architecture

```
┌─────────────────────────────────────────────┐
│ Rust Backend                                │
│ - Parse SCIP JSON                           │
│ - Build full call graph                     │
│ - Export to D3.js JSON format               │
│   (nodes + links structure)                 │
└──────────────────┬──────────────────────────┘
                   │
                   ↓ call_graph_d3.json
┌─────────────────────────────────────────────┐
│ TypeScript + D3.js Frontend                 │
│ - Load call graph JSON                      │
│ - Force-directed visualization              │
│ - Interactive filters                       │
│ - Search & node exploration                 │
└─────────────────────────────────────────────┘
```

## Quick Start

### Option 1: Automated (Recommended)

The script supports three modes:

**A. From Rust project (auto-generates SCIP):**
```bash
./QUICKSTART_INTERACTIVE.sh /path/to/your/rust/project
```

**B. From existing SCIP JSON:**
```bash
./QUICKSTART_INTERACTIVE.sh /path/to/index_scip.json
```

**C. From existing D3 graph JSON (fastest!):**
```bash
./QUICKSTART_INTERACTIVE.sh /path/to/call_graph_d3.json
```

**D. Auto-detect in current directory:**
```bash
./QUICKSTART_INTERACTIVE.sh
```

The script will:
1. Generate or use existing SCIP index
2. Export to D3 format
3. Start the web viewer
4. Show you exactly where the JSON file is

### Option 2: Manual

#### 1. Build the Rust Binary

```bash
cargo build --release
```

#### 2. Export Your Call Graph

```bash
# Example: Export libsignal call graph
cargo run --release --bin export_call_graph_d3 -- \
  path/to/your/index_scip.json \
  -o call_graph_d3.json
```

This creates a `call_graph_d3.json` file with the complete call graph in D3.js format.

#### 3. Start the Web Viewer

```bash
cd web
npm install
npm run dev
```

The viewer will open in your browser (usually at `http://localhost:3000`).

#### 4. Load and Explore

1. Click "📁 Load Graph JSON"
2. Select your `call_graph_d3.json` file
3. Start exploring!

## Key Features

### 🎨 Three Visualization Views

Switch between views using the header buttons:

#### Call Graph (default)
- **Force-directed layout** using D3.js physics simulation with topological layering
- **Zoom and pan** to navigate large graphs
- **Drag nodes** to rearrange the layout
- **Color coding** by verification status: green = verified, red = failed, grey = unverified
- Best for exploring function-level call paths and source-to-sink analysis

#### Blueprint
- **Dagre hierarchical layout** that groups functions by file
- Each file is a compound box containing its function nodes
- Cross-file edges are drawn between file groups
- Best for understanding module-level structure

#### Crate Map
- **Crate-level overview** where each crate is a single box showing function and file counts
- Edges between crates are weighted by the number of cross-crate function calls
- **Click** a cross-crate edge to expand it inline, showing the individual function calls
- **Click** crates to select them for the crate frontier (source = blue, target = orange)
- **Double-click** a crate to switch to Call Graph view filtered to that crate's files
- Best for understanding inter-crate dependencies at a glance

### 🔗 Crate Frontier

Select two crates to see their interface -- which functions in crate A are called by crate B:

- Use the **Source Crate** / **Target Crate** dropdowns in the sidebar
- Or **click** crates on the Crate Map (first click = source, second click = target)
- The target dropdown is automatically filtered to only show crates the source depends on
- In Crate Map view, the frontier is rendered inline with a "View in Call Graph" button
- In Call Graph / Blueprint views, it sets `crate:A` / `crate:B` source/sink queries

### 🔍 Powerful Filtering

#### Source → Sink
- **Source only**: Shows what the function calls (callees)
- **Sink only**: Shows who calls the function (callers)
- **Both**: Shows all paths from source to sink

#### Crate Queries
Use `crate:name` syntax in Source/Sink fields to match all functions in a crate. When both use `crate:` queries, a direct boundary mode shows only cross-crate calls.

#### Source Type Filter
Toggle visibility of libsignal vs external dependencies

#### Search
Find functions by:
- Display name
- Symbol path
- File name

#### Depth-Based Exploration
1. Click nodes to select them as starting points
2. Adjust "Max Depth" slider
3. See only functions within N steps of selected nodes
4. Perfect for understanding local call patterns

### ℹ️ Detailed Node Information

Click or hover over any node to see:
- Function name, mode, and file location
- Verification status badge
- GitHub link (if configured)
- **List of callers** (who calls this function, filtered items shown dimmed)
- **List of callees** (who this function calls)
- Similar lemmas with similarity scores (if available)

## Comparison with Static SVG Approach

### Before (Static SVG)
```bash
# Need to run a new command for each filter combination
cargo run --bin generate_function_subgraph_dot -- \
  index.scip \
  my_function \
  --include-callees \
  --depth 3 \
  --filter-non-libsignal-sources

# Generates: my_function.dot, my_function.svg, my_function.png
# To change filters? Run again with different flags
```

**Limitations:**
- Need to re-run for every filter change
- Can't dynamically explore
- Hard to navigate large graphs
- No search functionality

### Now (Interactive Viewer)
```bash
# Export once
cargo run --bin export_call_graph_d3 -- index.scip -o graph.json

# Open web viewer, load JSON
# All filtering happens in the browser!
```

**Benefits:**
- ✅ Export once, explore infinitely
- ✅ Real-time filtering (no waiting)
- ✅ Search functionality
- ✅ Click to explore callers/callees
- ✅ Adjustable depth without re-exporting
- ✅ Better for large graphs

## Use Cases

### 1. Understanding Function Dependencies
1. Search for your function
2. Click it to see all callers and callees
3. Explore the neighborhood

### 2. Finding Critical Functions
1. Adjust "Min Callers" threshold
2. Find functions called by many others (entry points)
3. Adjust "Min Callees" threshold
4. Find functions that call many others (orchestrators)

### 3. Exploring from Entry Points
1. Search for and click main entry points
2. Set depth to 2-3
3. See immediate dependencies without noise

### 4. Isolating Libsignal Code
1. Uncheck "External"
2. See only libsignal internal calls
3. Or vice versa to see external dependencies

### 5. Analyzing Call Chains
1. Click a starting function
2. Use depth filter to see how far calls propagate
3. Hover over nodes to highlight connections

## Data Format

The exported JSON has this structure:

```json
{
  "nodes": [
    {
      "id": "rust-analyzer cargo ...",
      "display_name": "my_function",
      "symbol": "...",
      "full_path": "/path/to/file.rs",
      "relative_path": "src/file.rs",
      "file_name": "file.rs",
      "parent_folder": "src",
      "start_line": 42,
      "end_line": 58,
      "is_libsignal": true,
      "dependencies": ["callee_id_1", "callee_id_2"],
      "dependents": ["caller_id_1"],
      "kind": "exec"
    }
  ],
  "links": [
    {
      "source": "node_id_1",
      "target": "node_id_2",
      "type": "inner"
    }
  ],
  "metadata": {
    "total_nodes": 150,
    "total_edges": 342,
    "project_root": "/path/to/project",
    "generated_at": "2025-11-05T...",
    "github_url": "https://github.com/user/repo"
  }
}
```

## Performance Tips

For graphs with 1000+ nodes:

1. **Start with filters** - Use search or source type filters before visualizing
2. **Use depth filtering** - Select key nodes and limit depth
3. **Increase thresholds** - Focus on high-connectivity nodes
4. **Wait for settlement** - Let the force simulation stabilize

## Technical Stack

- **Rust**: Parsing SCIP, building call graphs, exporting JSON
- **TypeScript**: Type-safe frontend code
- **D3.js v7**: Visualization and force simulation
- **Vite**: Fast build tool and dev server

## Development

See `web/README.md` for:
- Detailed development instructions
- Project structure
- Customization options
- TypeScript type definitions

## Future Enhancements

Potential improvements:

- [ ] Persistent layouts (save/load node positions)
- [ ] Export filtered views as images
- [ ] Time-based visualization (if SCIP data includes timestamps)
- [ ] Compare two versions side-by-side
- [x] Integration with code editor (jump to definition) -- See [VSCODE_EXTENSION.md](./VSCODE_EXTENSION.md)
- [x] Clustering/grouping by module -- Blueprint view (grouped by file) and Crate Map view (grouped by crate)
- [x] Call path finder (shortest path between two functions) -- Source + Sink combined mode
- [x] Crate-level dependency visualization -- Crate Map view with frontier selection

## Complementary Tools

The interactive viewer **complements** the existing static tools:

- **Static SVGs**: Great for documentation, reports, presentations
- **Interactive viewer**: Great for exploration, analysis, understanding

Use both as needed for your workflow!

## Troubleshooting

### "Cannot find module 'd3'"
Run `npm install` in the `web/` directory.

### Graph shows but no nodes appear
Check browser console for errors. Ensure JSON is in correct format.

### Performance is slow
Try filtering to reduce visible nodes. Large graphs (5000+ nodes) may be slow.

### Can't see my function
Use the search box - it searches names, symbols, and file paths.

## Feedback

Have ideas for improvements? Open an issue or PR on GitHub!

