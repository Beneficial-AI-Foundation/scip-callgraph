# rust-analyzer-test

## Overview

This repository contains several tools and examples for analyzing Rust source code. It demonstrates how to:
- Parse Rust code using the syn crate.
- Generate call graphs that show function dependencies.
- Filter out standard library dependencies by leveraging rustdoc JSON.

## Key Components

### call_graph_with_syn_v1.rs

This binary parses a Rust source file and produces a JSON call graph. Its key features include:
- **Parsing using syn:** It uses a visitor pattern to extract function definitions and dependencies.
- **Dependency filtering:** It integrates with rustdoc JSON to automatically discard standard dependencies (e.g., from `std`, `core`, or `alloc`) and removes self-dependencies.
- **JSON Output:** The resulting call graph is output as a pretty printed JSON format which can be used for further visualization or analysis.

### Other Tools

- **call_graph_with_syn_v0.rs:** An earlier version of the call graph generator.
- **build_rustdoc_json.rs:** A tool to build rustdoc JSON output for the project.
- **parse_rustdoc_deps.rs:** Parses dependencies from rustdoc output.
- **rust_analyzer_ex1.rs, rust_analyzer_ex2.rs, rust_analyzer_ex3.rs:** Examples demonstrating integration with rust-analyzer for code analysis.

## Usage

1. **Generate rustdoc JSON:**
   ```bash
   cargo run --bin build_rustdoc_json path/to/Cargo.toml
   ```
   This generates a file named `rustdoc_output.json`.

2. **Generate a Call Graph:**
   ```bash
   cargo run --bin call_graph_with_syn_v1 path/to/file.rs
   ```
   The tool prints the JSON call graph to stdout.

3. **Explore Additional Examples:**
   Run other binaries (e.g., rust_analyzer_ex3) to see further analysis in action:
   ```bash
   cargo run --bin rust_analyzer_ex3
   ```

## Dependencies

- **syn** for parsing Rust source code.
- **serde / serde_json** for JSON serialization.
- **quote** for converting syntax trees back into tokens.
- **rustdoc-json** for generating the rustdoc JSON used for dependency filtering.
