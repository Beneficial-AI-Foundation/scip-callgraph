# Public Directory

This directory is used for serving static assets during development and production.

## Auto-Loading Feature

When you run `QUICKSTART_INTERACTIVE.sh`, it automatically copies the generated call graph to:

```
web/public/graph.json
```

The web application will automatically fetch and load this file on startup if it exists, providing a seamless experience without manual file selection.

## How It Works

1. **Script copies graph:** `QUICKSTART_INTERACTIVE.sh` → `cp call_graph_d3.json web/public/graph.json`
2. **Vite serves it:** Files in `public/` are accessible at the root URL (e.g., `http://localhost:3000/graph.json`)
3. **App auto-loads:** On startup, the app calls `fetch('/graph.json')` and loads the graph automatically

## Manual Usage

If you want to manually place a graph for auto-loading:

```bash
# Copy your graph JSON to this directory
cp /path/to/your/call_graph.json web/public/graph.json

# Start the dev server
npm run dev

# The graph will load automatically!
```

## Note

The `graph.json` file is gitignored to avoid committing large graph data files to the repository.

