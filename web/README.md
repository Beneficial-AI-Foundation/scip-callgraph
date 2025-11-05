# SCIP Call Graph Interactive Viewer

An interactive web-based visualization tool for exploring call graphs generated from SCIP (SCIP Code Intelligence Protocol) data.

## Features

- 🎨 **Interactive D3.js Force-Directed Graph** - Visualize call relationships with physics-based layout
- 🔍 **Dynamic Filtering** - Filter by source type (libsignal vs external), search, caller/callee counts
- 📏 **Depth Control** - Explore call graphs from selected nodes with adjustable depth
- 🎯 **Node Selection** - Click nodes to see detailed information about functions
- 🔗 **Relationship Visualization** - See callers and callees for any selected function
- ⚡ **Real-time Updates** - All filters apply instantly without reloading
- 📱 **Responsive Design** - Works on different screen sizes

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
cargo build --release

# Export call graph from SCIP JSON
cargo run --bin export_call_graph_d3 -- <input_scip.json> -o call_graph_d3.json
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

- **Zoom:** Mouse wheel or pinch gesture
- **Pan:** Click and drag the background
- **Move Nodes:** Click and drag individual nodes
- **Select Node:** Click on a node to see details

### Filtering

#### Source Type
- **Libsignal** (blue nodes): Functions from the libsignal codebase
- **External** (green nodes): Functions from external dependencies

#### Search
Type function names, symbols, or file names to filter the graph.

#### Depth Limit
1. Click one or more nodes to select them as starting points
2. Adjust the "Max Depth" slider to show only nodes within N steps
3. Use "Clear Selection" to reset

#### Connection Thresholds
- **Min Callers:** Show only functions called by at least N other functions
- **Min Callees:** Show only functions that call at least N other functions

### Node Information Panel

The right sidebar shows detailed information about selected/hovered nodes:
- Display name and source type
- File location and full symbol path
- List of callers (functions that call this one)
- List of callees (functions called by this one)
- Function body (if available)

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

## Technology Stack

- **TypeScript** - Type-safe JavaScript for better development experience
- **D3.js v7** - Data visualization and force simulation
- **Vite** - Fast build tool and development server
- **Vanilla CSS** - No framework overhead, clean and maintainable

## Tips for Large Graphs

For very large call graphs (1000+ nodes):

1. Use filters to reduce the visible set before loading
2. Start with a search query to focus on specific areas
3. Use depth filtering from key entry points
4. Adjust caller/callee thresholds to show only important nodes

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

### Type Checking

```bash
npm run type-check
```

### Hot Module Replacement

The dev server supports HMR - changes to TypeScript files will update instantly without page reload.

## License

Same as parent project (MIT OR Apache-2.0)

