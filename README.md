# scip-callgraph

A call graph generator, visualizer, and **complexity metrics analyzer** for Rust projects using rust-analyzer (or verus-analyzer) and SCIP.

## Overview

This workspace provides three main capabilities:

1. **Call Graph Generation** - Generate caller/callee graphs from SCIP indices
2. **Verus Metrics** - Compute Halstead complexity metrics for Verus specifications and proofs
3. **Interactive Visualization** - Explore call graphs dynamically in your browser

## Workspace Structure

```
scip-callgraph/
├── crates/
│   ├── scip-core/           # Core SCIP parsing library
│   ├── verus-metrics/       # Halstead metrics for Verus specs/proofs
│   └── metrics-cli/         # All command-line tools (36 binaries)
├── web/                     # Interactive web viewer
├── examples/                # Example data and test projects
├── docs/                    # Detailed documentation
└── METRICS_PIPELINE.md      # Verus metrics pipeline guide
```

## Prerequisites

If you want to use your own JSON file, you need to install:

- [rust-analyzer](https://rust-analyzer.github.io/book/installation.html) (or [verus-analyzer](github.com/verus-lang/verus-analyzer)) - generates SCIP output
- [scip](https://github.com/sourcegraph/scip) - converts SCIP to JSON

### Generating SCIP JSON

```bash
# In your Rust project directory:
rust-analyzer scip .
scip print --json index.scip > index_scip.json
```

You can also use [these scripts](https://github.com/Beneficial-AI-Foundation/installers_for_various_tools) for automated setup.

## Quick Start

### Build the Workspace

```bash
cargo build --workspace
```

### Running Tools

All binaries are in the `metrics-cli` package:

```bash
# General syntax
cargo run -p metrics-cli --bin <binary-name> -- <args>

# Or use the built binaries directly
./target/debug/<binary-name> <args>
```

---

## Call Graph Tools

### 1. Generate Full Call Graph

```bash
cargo run -p metrics-cli --bin generate_call_graph_dot -- <path_to_scip_json>
```

Outputs: `call_graph.json`, `call_graph.dot`, `call_graph.svg`, `call_graph.png`

### 2. Generate File Subgraph

```bash
cargo run -p metrics-cli --bin generate_files_subgraph_dot -- \
  <input-scip-json> <output-dot-file> <file-path1> [<file-path2> ...]
```

### 3. Generate Function Subgraph

```bash
cargo run -p metrics-cli --bin generate_function_subgraph_dot -- \
  <input-scip-json> <output-dot-file> <function-name> \
  [--include-callees] [--include-callers] [--depth <n>]
```

### 4. Interactive Call Graph Viewer

**Online:** Visit `https://beneficial-ai-foundation.github.io/scip-callgraph/`

**Local:**
```bash
# Export call graph data
cargo run -p metrics-cli --bin export_call_graph_d3 -- <input-scip-json> -o call_graph_d3.json

# Start the web viewer
cd web && npm install && npm run dev
```

See [docs/INTERACTIVE_VIEWER.md](docs/INTERACTIVE_VIEWER.md) for complete documentation.

---

## 🆕 Verus Metrics Pipeline

Compute complexity metrics for Verus-verified Rust code, including:
- **Specification metrics** - Halstead metrics for `requires`, `ensures`, `decreases` clauses
- **Proof metrics** - Halstead metrics for `proof { }` blocks with transitive lemma analysis
- **Code metrics** - Integration with `rust-code-analysis` for cyclomatic/cognitive complexity

### Full Pipeline

```bash
# Step 1: Generate atoms from SCIP
cargo run -p metrics-cli --bin write_atoms -- \
  index_scip.json atoms.json

# Step 2: Compute spec metrics
cargo run -p metrics-cli --bin compute_metrics -- \
  atoms.json atoms_with_metrics.json

# Step 3: Compute proof metrics (with transitive lemma analysis)
cargo run -p metrics-cli --bin compute_proof_metrics -- \
  atoms_with_metrics.json atoms_complete.json

# Step 4: Enrich CSV with code metrics (from rust-code-analysis)
cargo run -p metrics-cli --bin enrich_csv_with_metrics -- \
  functions.csv rca_output_dir/ functions_enriched.csv

# Step 5: Add spec + proof metrics to CSV
cargo run -p metrics-cli --bin enrich_csv_complete -- \
  atoms_complete.json functions_enriched.csv functions_COMPLETE.csv
```

See [METRICS_PIPELINE.md](METRICS_PIPELINE.md) for detailed documentation.

---

## Crates

### `scip-core`

Core library for SCIP parsing and call graph generation.

```rust
use scip_core::{parse_scip_json, build_call_graph, write_call_graph_as_atoms_json};

let scip_index = parse_scip_json("index.json")?;
let call_graph = build_call_graph(&scip_index);
write_call_graph_as_atoms_json(&call_graph, "atoms.json")?;
```

### `verus-metrics`

Halstead metrics computation for Verus specifications.

```rust
use verus_metrics::spec_halstead::analyze_spec;

let metrics = analyze_spec("x > 0 && y < 100");
println!("Halstead length: {:?}", metrics.halstead_length);
```

### `metrics-cli`

36 command-line tools including:

| Tool | Description |
|------|-------------|
| `write_atoms` | Extract functions from SCIP to atoms JSON |
| `compute_metrics` | Compute Verus spec Halstead metrics |
| `compute_proof_metrics` | Compute proof block Halstead metrics |
| `enrich_csv_with_metrics` | Add RCA metrics to CSV |
| `enrich_csv_complete` | Add all metrics to CSV |
| `generate_call_graph_dot` | Generate full call graph |
| `generate_function_subgraph_dot` | Generate function subgraph |
| `export_call_graph_d3` | Export for web viewer |

---

## Building Releases

### Local Build

```bash
cargo build --release --workspace
```

### GitHub Actions

- **CI**: `.github/workflows/build.yml` - runs on every push
- **Release**: `.github/workflows/release.yml` - triggered by version tags

```bash
# Create a release
git tag v1.0.0
git push origin v1.0.0
```

---

## Documentation

- [METRICS_PIPELINE.md](METRICS_PIPELINE.md) - Verus metrics pipeline guide
- [docs/INTERACTIVE_VIEWER.md](docs/INTERACTIVE_VIEWER.md) - Web viewer documentation
- [docs/GITHUB_PAGES_GUIDE.md](docs/GITHUB_PAGES_GUIDE.md) - Online viewer guide

## License

MIT OR Apache-2.0
