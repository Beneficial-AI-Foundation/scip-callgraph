## Prerequirements

If you want to use your own json file., you need to install:

- [rust-analyzer](https://rust-analyzer.github.io/book/installation.html) it is used to generate a scip output file
- [scip](https://github.com/sourcegraph/scip) it is used to generate a JSON from the scip output file

If you just want to get info about `libsignal`, simply replace `scip_data.json` by `index_scip_libsignal_deps.json` in the commands below. (If you want to format it so you can see its structure, you can use the command `jq '.' index_scip_libsignal_deps.json > formatted_index_scip_libsignal_deps
.json`.)

## Building a Release

### Local Build

To build an optimized release version of the `generate_function_subgraph_dot` tool locally:

```bash
# Option 1: Use the build script
./build_release.sh

# Option 2: Build manually
cargo build --release --bin generate_function_subgraph_dot
```

The release binary will be located at `target/release/generate_function_subgraph_dot` and can be distributed as a standalone executable.

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

### 2. Generate file subgraph

Extract the subgraph of files based on a set of source files:

```bash
cargo run --bin generate_files_subgraph_dot <input-scip-json> <output-dot-file> <file-path1> [<file-path2> ...]
```

Example:

```bash
cargo run --bin generate_files_subgraph_dot scip_data.json files_subgraph.dot src/lib.rs src/main.rs
dot -Tsvg files_subgraph.dot -o files_subgraph.svg
```

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
cargo run --bin generate_function_subgraph_dot index_scip_libsignal_deps.json reduce3.dot "rust-analyzer cargo curve25519-dalek 4.1.3 backend/serial/u64/field/impl#[FieldElement51]reduce()" --include-callers --depth 3
```
