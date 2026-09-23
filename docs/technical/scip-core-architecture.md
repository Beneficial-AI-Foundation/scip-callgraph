# scip-core Architecture

This document describes the modular architecture of the `scip-core` library.

## Module Structure

The library is organized into focused, single-responsibility modules:

```
scip-core/src/
├── lib.rs           # Re-exports commonly used items
├── types.rs         # All shared data structures
├── parser.rs        # SCIP JSON parsing
├── call_graph.rs    # Core graph building and analysis
├── export_d3.rs     # D3.js/web export
├── export_dot.rs    # DOT/Graphviz export
├── atoms_to_d3.rs   # Convert probe-verus output to D3.js format
├── call_graph_svg.rs # Legacy SVG visualization
├── scip_reader.rs   # Alternative SCIP reader
└── scip_utils.rs    # SCIP utility functions
```

## Module Descriptions

### `types.rs` - Shared Data Structures

All shared types used across the library:

**SCIP Index Types:**
- `ScipIndex` - Root structure of SCIP JSON
- `Metadata`, `ToolInfo` - Project metadata
- `Document` - A source file
- `Occurrence`, `Symbol`, `SignatureDocumentation`

These structs tolerate missing fields: `scip print --json` uses proto3 JSON
serialization, which omits any field holding its protobuf default (0, empty
string, empty list), so most fields carry `#[serde(default)]`.

**Call Graph Types:**
- `FunctionNode` - A node in the call graph
- `CallLocation` - Where a call occurs (Precondition/Postcondition/Inner)
- `CalleeOccurrence` - A call with location info
- `Atom` - Function with dependencies (for JSON export)

**Verus Types:**
- `DeclKind` - exec/proof/spec (serialized lowercase)
- `FunctionSections` - requires/ensures/body line ranges

**D3.js Types:**
- `D3Node`, `D3Link`, `D3Graph`, `D3GraphMetadata`

### `parser.rs` - SCIP Parsing

Functions for parsing SCIP JSON files:

- `parse_scip_json(path) -> Result<ScipIndex>` - Parse a SCIP JSON file
- `extract_display_name_from_symbol(symbol) -> String` - Extract human-readable name
- `extract_path_info_from_symbol(symbol) -> (full_path, file_name, parent_folder)`

### `call_graph.rs` - Core Graph Operations

Core call graph building and analysis:

- `build_call_graph(scip_data) -> HashMap<String, FunctionNode>` - Build call graph
- `is_function_like(kind) -> bool` - Check if symbol is function-like
- `detect_decl_kind(body) -> DeclKind` - Detect Verus declaration kind
- `parse_function_sections(body, start_line) -> FunctionSections` - Parse spec sections
- `classify_call_location(line, sections) -> CallLocation` - Classify call location
- `symbol_to_path(symbol, display_name) -> String` - Convert symbol to path
- `generate_filtered_call_graph(graph, entry_points, max_depth)` - Create subgraph
- `print_call_graph_summary(graph)` - Print summary statistics

### `export_d3.rs` - Web Export

D3.js/web visualization export:

- `export_call_graph_d3(graph, scip_data, output_path)` - Export to D3 JSON
- `write_call_graph_as_atoms_json(graph, output_path)` - Export as atoms JSON

### `export_dot.rs` - CLI Export

DOT/Graphviz export for command-line visualization:

- `generate_call_graph_dot_string(graph) -> String` - Generate DOT as string
- `generate_call_graph_dot(graph, output_path)` - Full graph as DOT file + SVG/PNG
- `generate_file_subgraph_dot(graph, file_path, output_path)` - File subgraph
- `generate_files_subgraph_dot(graph, file_paths, output_path)` - Multi-file subgraph
- `generate_function_subgraph_dot(graph, functions, output, callees, callers, depth, filter)` - Function subgraph
- `generate_call_graph_svg(graph, output_path)` - Simple SVG visualization
- `generate_svg_and_png_from_dot(dot_path)` - Convert DOT to SVG/PNG

## SCIP Indexer Pitfalls

Two properties of rust-analyzer's SCIP output that cost real debugging time:

**`doc.symbols[]` lists symbols *visible* in a file, not *defined* in it.**
Treating a symbol's presence in a document as "defined here" attributes
functions to the wrong files and extracts bodies from the wrong sources.
Definitions must be located via occurrences with `symbol_roles & 1 == 1`;
`build_call_graph` does this in a pre-pass (`call_graph.rs`, the
`symbol_to_def_file` map) and skips symbols with no definition occurrence.

**Operator-overload calls were missing before rust-analyzer PR #21187
(Dec 2025).** `scalar *= x` compiles to `MulAssign::mul_assign`, but older
rust-analyzer versions emitted no occurrence for the trait method, so call
graphs built from those indexes silently miss every `+=`/`*=`/`+`/`*`
operator edge — significant in arithmetic-heavy code like curve25519-dalek.
Fixed upstream in [rust-lang/rust-analyzer#21187](https://github.com/rust-lang/rust-analyzer/pull/21187);
regenerate indexes with a rust-analyzer from Dec 2025 or later. Details in
`docs/archive/operator_overload_scip_analysis.md`.

## Usage

### Quick Start

```rust
use scip_core::{parse_scip_json, build_call_graph, export_call_graph_d3};

// Parse SCIP JSON
let scip_data = parse_scip_json("index.scip.json")?;

// Build call graph
let call_graph = build_call_graph(&scip_data);

// Export for web visualization
export_call_graph_d3(&call_graph, &scip_data, "graph.json")?;
```

### Using Specific Modules

```rust
use scip_core::types::{FunctionNode, DeclKind};
use scip_core::parser::parse_scip_json;
use scip_core::call_graph::{build_call_graph, detect_decl_kind};
use scip_core::export_d3::export_call_graph_d3;
use scip_core::export_dot::generate_function_subgraph_dot;
```

### Convenience Re-exports

The most commonly used items are re-exported from `scip_core` directly:

```rust
use scip_core::{
    // Types
    ScipIndex, FunctionNode, Atom, D3Graph, DeclKind,

    // Parser
    parse_scip_json,

    // Call graph
    build_call_graph, generate_filtered_call_graph,

    // Export
    export_call_graph_d3, generate_call_graph_dot,
};
```

## Design Principles

1. **Single Responsibility** - Each module has one clear purpose
2. **No Duplication** - Types defined once in `types.rs`
3. **Clear Dependencies** - Modules import from each other explicitly
4. **Convenient Re-exports** - Common items accessible from `scip_core` root

## History

The library was refactored from two large monolithic modules (`scip_call_graph.rs`
and `scip_to_call_graph_json.rs`) into the current layered architecture. The old
modules have been removed - all functionality is now in the new modules described above.
