# Generate SCIP Index JSON

A utility to generate SCIP (Source Code Indexing Protocol) JSON index files for Rust projects.

## Overview

This tool automates the process of running `verus-analyzer scip` and `scip print` to generate a JSON representation of the code index, which can be used for further analysis.

## Usage

### Command Line

```bash
cargo run --bin generate_index_scip_json -- /path/to/rust/project
```

### Example

```bash
# Generate SCIP index for current project
cargo run --bin generate_index_scip_json -- .

# Generate SCIP index for another project
cargo run --bin generate_index_scip_json -- /path/to/my/verus/project
```

## What It Does

1. Validates that the provided path exists and is a directory
2. Runs `verus-analyzer scip <path_to_folder>` to generate `index.scip`
3. Runs `scip print --json index.scip` to convert to JSON format
4. Saves the output as `<folder_name>_index_scip.json`

## Output

Creates a file named `<folder_name>_index_scip.json` in the current directory containing:
- Symbol definitions
- Symbol references
- Function signatures
- Call relationships
- Source file locations
- And more metadata about the codebase

## Using as a Library

The binary is a thin wrapper; the function lives in `scip-core`:

```rust
use scip_core::scip_utils::generate_scip_json_index;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_file = generate_scip_json_index("/path/to/project")?;
    println!("Generated: {}", output_file);
    Ok(())
}
```

## Requirements

- `verus-analyzer` must be installed and in PATH
- `scip` CLI tool must be installed and in PATH
- Target directory must be a valid Rust project

## Error Handling

The tool will exit with an error if:
- The provided path doesn't exist
- The path is not a directory
- `verus-analyzer scip` fails
- `scip print` fails
- Writing the output file fails

## Related Tools

- `detect_unused_specs`: Uses this tool internally to analyze specs
- `write_atoms`: Processes SCIP JSON into atoms format
- `generate_call_graph_dot`: Visualizes call graphs from SCIP data

## Caching

You can save time on repeated analyses by:
1. Generating the SCIP JSON once
2. Reusing it for multiple analysis passes
3. Only regenerating when the source code changes

Example workflow:
```bash
# Generate once
cargo run --bin generate_index_scip_json -- .

# Use multiple times
cargo run --bin detect_unused_specs -- . ./my_project_index_scip.json
cargo run --bin generate_call_graph_dot -- ./my_project_index_scip.json output.dot
```

## Troubleshooting

### "verus-analyzer not found"
Install verus-analyzer and add it to your PATH.

### "scip not found"
Download a release from https://github.com/sourcegraph/scip/releases and put it on `PATH`.

### "index.scip not found"
This usually means `verus-analyzer scip` failed. Try running it manually to see the error:
```bash
verus-analyzer scip /path/to/project
```

### Very large output files
For large projects, the JSON file can be quite large (100+ MB). This is normal but make sure you have sufficient disk space.

