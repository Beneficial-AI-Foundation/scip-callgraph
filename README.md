# scip-callgraph

A call graph generator and visualizer for Rust projects using rust-analyzer (or verus-analyzer) and SCIP.

This tool can be used to generate a graph of callers/callees for a given rust project from the json obtained after running `rust-analyzer` (or `verus-analyzer`) and `scip`. The tool automatically generates output in multiple formats: DOT (for Graphviz), SVG (vector graphics), and PNG (raster graphics). 

## Prerequirements

If you want to use your own json file, you need to install:

- [rust-analyzer](https://rust-analyzer.github.io/book/installation.html) (or [verus-analyzer](github.com/verus-lang/verus-analyzer)) it is used to generate a scip output file: one needs to run `rust-analyzer scip .` from the command line within a rust project; this will generate `index.scip` within the rust project; 
- [scip](https://github.com/sourcegraph/scip) it is used to generate a JSON from the scip output file.

### Note
You can use [these scripts](https://github.com/Beneficial-AI-Foundation/installers_for_various_tools) to install the tools and to [generate the json](https://github.com/Beneficial-AI-Foundation/installers_for_various_tools?tab=readme-ov-file#generate-scip-index). If you prefer to use `rust-analyzer` and `scip` on your own, you need to run:
- `rust-analyzer scip .` from the command line within a rust project; this will generate `index.scip` within the rust project;
- `scip print --json index.scip > index_scip.json` to obtain `index_scip.json` which is then given as input to obtain the call graphs.

If you just want to get info about `libsignal`, simply replace `index_scip.json` by `examples/example_data/index_scip_libsignal_deps.json` in the commands below. Example SCIP data files are provided in the `examples/example_data/` directory.

## How to use

Below are the main utilities provided by this repository:

### 1. Generate full call graph

Run the analyzer on a Rust project to produce a call graph (JSON and DOT):

```bash
cargo run --bin generate_call_graph_dot <path_to_rust_repo>
```

This outputs:

- `call_graph.json`: the call graph data in JSON format
- `call_graph.dot`: the call graph in Graphviz DOT format
- `call_graph.svg`: the call graph as an SVG image
- `call_graph.png`: the call graph as a PNG image

### 2. Generate file subgraph

Extract the subgraph of files based on a set of source files:

```bash
cargo run --bin generate_files_subgraph_dot <input-scip-json> <output-dot-file> <file-path1> [<file-path2> ...]
```

Example:

```bash
cargo run --bin generate_files_subgraph_dot index_scip.json files_subgraph.dot src/lib.rs src/main.rs
```

This automatically generates:
- `files_subgraph.dot`: the subgraph in DOT format
- `files_subgraph.svg`: the subgraph as an SVG image
- `files_subgraph.png`: the subgraph as a PNG image

### 3. Generate function subgraph

Extract the subgraph of functions based on a set of function names, with optional caller/callee expansion:

```bash
cargo run --bin generate_function_subgraph_dot <input-scip-json> <output-dot-file> <function-name1> [<function-name2> ...] [--include-callees] [--include-callers] [--depth <n>] [--filter-non-libsignal-sources]
```

Flags:

- `--include-callees`: include functions called by the specified functions
- `--include-callers`: include functions that call the specified functions
- `--depth <n>`: limit the caller/callee expansion to depth _n_
- `--filter-non-libsignal-sources`: filter out functions that are not from libsignal sources

Example:

```bash
cargo run --bin generate_function_subgraph_dot examples/example_data/index_scip_libsignal_deps.json reduce3.dot "rust-analyzer cargo curve25519-dalek 4.1.3 backend/serial/u64/field/impl#[FieldElement51]reduce()" --include-callers --depth 3
```

This automatically generates:
- `reduce3.dot`: the function subgraph in DOT format
- `reduce3.svg`: the function subgraph as an SVG image
- `reduce3.png`: the function subgraph as a PNG image

### 4. 🆕 Interactive Call Graph Viewer

Explore call graphs dynamically in your browser with real-time filtering!

#### 🌐 Online Viewer (No Installation!)

**Visit:** `https://beneficial-ai-foundation.github.io/scip-callgraph/`

Just upload your JSON file or share via URL:
```
https://beneficial-ai-foundation.github.io/scip-callgraph/?json=https://gist.github.com/.../graph.json
```

See [GitHub Pages Guide](docs/GITHUB_PAGES_GUIDE.md) for detailed instructions.

#### 💻 Local Setup

**Quick start:**
```bash
# Option 1: Provide a Rust project (auto-generates SCIP)
./QUICKSTART_INTERACTIVE.sh /path/to/your/rust/project

# Option 2: Use existing SCIP JSON file
./QUICKSTART_INTERACTIVE.sh /path/to/index_scip.json

# Option 3: Use existing D3 JSON file (fastest!)
./QUICKSTART_INTERACTIVE.sh /path/to/call_graph_d3.json

# Option 4: Use SCIP JSON in current directory
./QUICKSTART_INTERACTIVE.sh
```

**Or manually:**
```bash
# Export call graph data
cargo run --bin export_call_graph_d3 -- <input-scip-json> -o call_graph_d3.json

# Start the web viewer
cd web
npm install
npm run dev
```

#### ✨ Features

Explore with:
- 🔍 Real-time search and filtering
- 📏 Dynamic depth control from selected nodes
- 🎨 Interactive force-directed visualization
- ℹ️ Click nodes to see callers/callees
- 🎯 Filter by source type (libsignal vs external)

**Key advantages over static SVGs:**
- No need to regenerate for different filters
- Explore large graphs interactively
- Search functionality
- See caller/callee relationships on demand

See [docs/INTERACTIVE_VIEWER.md](docs/INTERACTIVE_VIEWER.md) for complete documentation.

## Project Structure

```
scip-callgraph/
├── src/              # Core library and binaries
│   ├── bin/          # Command-line tools
│   │   ├── export_call_graph_d3.rs  # NEW: Export for web viewer
│   │   └── ...       # Other binaries
│   └── ...
├── web/              # NEW: Interactive web viewer
│   ├── src/          # TypeScript source code
│   ├── index.html    # Main HTML page
│   └── package.json  # Node dependencies
├── examples/         # Example code and test projects
├── scripts/          # Build and utility scripts
├── docs/             # Detailed documentation
│   └── INTERACTIVE_VIEWER.md  # NEW: Web viewer docs
└── test_outputs/     # Generated graphs and outputs (gitignored)
```

## Building a Release

### Local Build

To build an optimized release version of the `generate_function_subgraph_dot` tool locally:

```bash
# Option 1: Use the build script
./scripts/build_release.sh

# Option 2: Build manually
cargo build --release --bin generate_function_subgraph_dot
```

The release binary will be located at `target/release/generate_function_subgraph_dot` and can be distributed as a standalone executable.

### Using the Release Binary from Other Projects

Once built, you can use the binary from any directory in several ways:

#### Option 1: Full Path
```bash
/path/to/scip-callgraph/target/release/generate_function_subgraph_dot input.json output.dot function_name --depth 10
```

#### Option 2: Add to PATH (Recommended)
Add the release directory to your PATH for convenient access from anywhere:

```bash
# Add to your ~/.bashrc or ~/.zshrc:
export PATH="$PATH:/path/to/scip-callgraph/target/release"

# Reload your shell:
source ~/.bashrc

# Now you can run from any directory:
generate_function_subgraph_dot input.json output.dot function_name --depth 10
```

### GitHub Actions Automated Builds

This repository includes GitHub Actions workflows for automated building and releasing:

#### Continuous Integration

- **Workflow**: `.github/workflows/build.yml`
- **Triggers**: Every push to `master`/`main` branch and pull requests
- **Platforms**: Linux, Windows, macOS
- **Actions**: Format check, linting, debug and release builds, basic functionality testing

#### Release Builds

- **Workflow**: `.github/workflows/release.yml`
- **Triggers**:
  - Git tags matching `v*` (e.g., `v1.0.0`, `v1.2.3`)
  - Manual workflow dispatch
- **Platforms**: Linux x86_64, Windows x86_64, macOS x86_64, macOS ARM64
- **Output**: Creates GitHub releases with downloadable binaries for each platform

#### Creating a Release

To create a new release with automated builds:

```bash
# Tag your release
git tag v1.0.0
git push origin v1.0.0
```

This will automatically:

1. Build binaries for all supported platforms
2. Create archives with documentation
3. Create a GitHub release with all binaries attached
