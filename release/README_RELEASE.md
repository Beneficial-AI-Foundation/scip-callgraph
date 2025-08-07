# Function Subgraph Analyzer - Release Binary

This is the release version of the `generate_function_subgraph_dot` tool from the rust-analyzer-test project.

## What this tool does

This tool generates function subgraphs from SCIP (Source Code Intelligence Protocol) JSON files. It can analyze Rust codebases and create visual call graphs showing relationships between functions.

## Usage

```bash
./generate_function_subgraph_dot <input-scip-json> <output-dot-file> <function-name1> [<function-name2> ...] [--include-callees] [--include-callers] [--depth <n>] [--filter-non-libsignal-sources]
```

### Arguments

- `<input-scip-json>`: Path to the input SCIP JSON file containing code analysis data
- `<output-dot-file>`: Path where the output DOT file will be written
- `<function-name1>` [<function-name2> ...]: One or more function names to analyze

### Flags

- `--include-callees`: Include functions called by the specified functions
- `--include-callers`: Include functions that call the specified functions
- `--depth <n>`: Limit the caller/callee expansion to depth _n_
- `--filter-non-libsignal-sources`: Filter out functions that are not from libsignal sources

### Example

```bash
./generate_function_subgraph_dot index_scip_libsignal_deps.json reduce3.dot "rust-analyzer cargo curve25519-dalek 4.1.3 backend/serial/u64/field/impl#[FieldElement51]reduce()" --include-callers --depth 3
```

This will:

1. Read the SCIP data from `index_scip_libsignal_deps.json`
2. Find the specified `reduce()` function
3. Include all functions that call this function (up to 3 levels deep)
4. Output a DOT file to `reduce3.dot`
5. Also generate an SVG visualization

## Converting DOT to SVG

After generating a DOT file, you can create an SVG visualization using Graphviz:

```bash
dot -Tsvg output.dot -o output.svg
```

## System Requirements

- Linux x86-64 system
- No additional dependencies required (statically linked)

## Binary Information

- Size: ~3.1MB
- Architecture: x86-64 Linux
- Type: Dynamically linked executable
- Built with: Rust (release/optimized build)

For more information about the source code and other tools, see the full README.md file or visit the project repository.
