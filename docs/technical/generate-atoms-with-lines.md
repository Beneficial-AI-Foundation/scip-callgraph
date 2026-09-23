# Generate Atoms JSON with Line Numbers

This tool generates a JSON representation of function call graphs with line number information instead of full function bodies. It's similar to `write_atoms` but produces a different JSON format that includes:

- Line ranges (`lines-start` and `lines-end`) instead of function bodies
- Visibility flags for all functions and dependencies
- Simplified dependency structure

## Format

The output JSON follows this structure:

```json
{
  "display-name": "function_name",
  "visible": true,
  "dependencies": {
    "path/to/dependency": { "visible": true }
  },
  "code-path": "/relative/path/to/file.rs",
  "code-function": "module::path::function",
  "code-text": {
    "lines-start": 123,
    "lines-end": 456
  }
}
```

## Prerequisites

Before using this tool, ensure you have:

1. **rust-analyzer** installed:
   ```bash
   rustup component add rust-analyzer
   ```

2. **scip** CLI installed: download a release from
   [sourcegraph/scip](https://github.com/sourcegraph/scip/releases) and put
   it on `PATH`.

## Building

Build the tool in release mode for better performance:

```bash
cargo build --release --bin generate_atoms_with_lines
```

## Usage

```bash
./target/release/generate_atoms_with_lines <project_path> <output_json>
```

### Arguments

- `<project_path>`: Path to the Rust project directory (must contain `Cargo.toml`)
- `<output_json>`: Path where the output JSON will be written

### Example

Generate atoms JSON for curve25519-dalek:

```bash
./target/release/generate_atoms_with_lines \
  ./curve25519-dalek \
  curve_dalek_atoms_with_lines.json
```

## What the Tool Does

The tool performs four main steps:

1. **Generate SCIP Index**: Runs `rust-analyzer scip` on the project to create `index.scip`
2. **Convert to JSON**: Runs `scip print --json` to convert the binary SCIP format to JSON
3. **Build Call Graph**: Parses the SCIP JSON and constructs a function call graph
4. **Generate Output**: Converts the call graph to the atoms-with-lines format

## Output Format Details

### Differences from `write_atoms`

| Feature | `write_atoms` | `generate_atoms_with_lines` |
|---------|---------------|----------------------------|
| Function body | Full text included | Not included |
| Line numbers | Not included | Start and end lines |
| Visibility flags | Not included | Included for all items |
| Dependencies | Array of strings | Object with visibility |

### Field Descriptions

- **`display-name`**: The simple name of the function (e.g., `from`)
- **`visible`**: Always `true` for top-level functions
- **`dependencies`**: Map of dependency paths to visibility info
  - Keys are the full path to the dependency function
  - Values contain a `visible` flag (always `true`)
- **`code-path`**: Relative path to the source file from project root
- **`code-function`**: Full qualified path to the function
- **`code-text`**: Object containing line range information
  - `lines-start`: Line number where the function starts (1-based)
  - `lines-end`: Line number where the function ends (1-based)

## Use Cases

This format is particularly useful for:

- **Code visualization tools** that need to link to specific line ranges
- **Dependency analysis** where function bodies are not needed
- **Interactive explorers** that can load function bodies on-demand
- **Large codebases** where including all function bodies would create huge JSON files

## Comparison Example

### Traditional `write_atoms` format:
```json
{
  "identifier": "curve25519_dalek/scalar/Scalar/sub",
  "statement_type": "function",
  "deps": ["curve25519_dalek/scalar/UnpackedScalar/sub"],
  "body": "fn sub(self, _rhs: &'b Scalar) -> Scalar {\n    ...\n}",
  "display_name": "sub",
  "full_path": "/path/to/scalar.rs",
  "relative_path": "src/scalar.rs",
  "file_name": "scalar.rs",
  "parent_folder": "src"
}
```

### New `generate_atoms_with_lines` format:
```json
{
  "display-name": "sub",
  "visible": true,
  "dependencies": {
    "curve25519_dalek/scalar/UnpackedScalar/sub": { "visible": true }
  },
  "code-path": "src/scalar.rs",
  "code-function": "curve25519_dalek::scalar::Scalar::sub",
  "code-text": {
    "lines-start": 679,
    "lines-end": 734
  }
}
```

## Troubleshooting

### "rust-analyzer not found"
Install it with: `rustup component add rust-analyzer`

### "scip not found"
Download a release from https://github.com/sourcegraph/scip/releases and put it on `PATH`.

### "Not a valid Rust project"
Make sure you're pointing to a directory containing a `Cargo.toml` file.

### "rust-analyzer scip failed"
Ensure the project compiles successfully first:
```bash
cd <project_path>
cargo check
```

### Large projects take a long time
This is normal. The SCIP index generation can take several minutes for large projects. The tool will show progress messages.

Note the lines-format output is for viewers and dependency analysis only:
`compute_metrics` requires atoms with a `body` field (use `write_atoms` for
that), and `detect_unused_specs` takes a project path, not an atoms file.

## See Also

- `write_atoms.rs` - Original atoms format with function bodies
- [`scip-core-architecture.md`](scip-core-architecture.md) - Core call graph building logic
- [SCIP Format Documentation](https://github.com/sourcegraph/scip)

