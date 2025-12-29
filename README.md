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
│   └── metrics-cli/         # All command-line tools (38 binaries, including pipeline)
├── external/                # Git submodules
│   ├── scip-atoms/          # Verification analysis (github.com/Beneficial-AI-Foundation/scip-atoms)
│   └── verus_lemma_finder/  # Similar lemma search (github.com/Beneficial-AI-Foundation/verus_lemma_finder)
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
cargo run --bin <binary-name> -- <args>

# Or use the built binaries directly
./target/debug/<binary-name> <args>
```

---

## Call Graph Tools

### 1. Generate Full Call Graph

```bash
cargo run --bin generate_call_graph_dot -- <path_to_scip_json>
```

Outputs: `call_graph.json`, `call_graph.dot`, `call_graph.svg`, `call_graph.png`

### 2. Generate File Subgraph

```bash
cargo run --bin generate_files_subgraph_dot -- \
  <input-scip-json> <output-dot-file> <file-path1> [<file-path2> ...]
```

### 3. Generate Function Subgraph

```bash
cargo run --bin generate_function_subgraph_dot -- \
  <input-scip-json> <output-dot-file> <function-name> \
  [--include-callees] [--include-callers] [--depth <n>]
```

### 4. Interactive Call Graph Viewer

**Online:** Visit https://beneficial-ai-foundation.github.io/scip-callgraph/

**Local (Unified Pipeline - Recommended):**

The `pipeline` command generates a fully enriched call graph in one step:

```bash
# First time setup (clone with submodules)
git clone --recurse-submodules https://github.com/Beneficial-AI-Foundation/scip-callgraph.git
cd scip-callgraph

# Build the workspace
cargo build --release --workspace

# Optional: Setup Python for similar lemmas feature
uv sync --extra enrich
cd external/verus_lemma_finder && uv tool run maturin develop --release && cd ../..

# Run the full pipeline on your Verus project
cargo run --release --bin pipeline -- /path/to/verus-project

# Start the web viewer
cd web && npm install && npm run dev
```

Open http://localhost:3000 to explore your call graph interactively.

The pipeline automatically:
1. **Generates SCIP index** from your Verus project
2. **Exports call graph** in D3 format
3. **Runs verification** and enriches nodes with status (verified/failed/unverified)
4. **Adds similar lemmas** from vstd (if Python is set up)

#### Pipeline Options

```bash
# Skip verification (faster, no Verus needed)
cargo run --release --bin pipeline -- /path/to/project --skip-verification

# Skip similar lemmas (no Python needed)
cargo run --release --bin pipeline -- /path/to/project --skip-similar-lemmas

# Use cached SCIP JSON if available (default: regenerate fresh)
cargo run --release --bin pipeline -- /path/to/project --use-cached-scip

# Add GitHub URL for source code links in the web viewer
cargo run --release --bin pipeline -- /path/to/project --github-url https://github.com/user/repo

# For workspace projects
cargo run --release --bin pipeline -- /path/to/project -p my-crate
```

#### Verification Status Colors

- **Green** - Verified functions (passed verification, no assume/admit)
- **Red** - Failed functions (verification errors)
- **Grey** - Unverified functions (contains assume/admit)
- **Blue** - Unknown (no verification data)

#### Manual Steps (Alternative)

If you prefer manual control, you can still run each step separately:

```bash
# Step 1: Export call graph from SCIP JSON
cargo run --release --bin export_call_graph_d3 -- \
    path/to/index_scip.json \
    -o web/public/graph.json

# Step 2: Enrich with verification status (optional)
python3 scripts/add_verification_status.py \
    --graph web/public/graph.json \
    --verification data/verification_results.json

# Step 3: Enrich with similar lemmas (optional)
uv run python scripts/enrich_graph_with_similar_lemmas.py \
    --graph web/public/graph.json \
    --index external/verus_lemma_finder/data/vstd_lemma_index.json
```

See [docs/guides/INTERACTIVE_VIEWER.md](docs/guides/INTERACTIVE_VIEWER.md) and [docs/SIMILAR_LEMMAS.md](docs/SIMILAR_LEMMAS.md) for details

---

## 🆕 Verus Metrics Pipeline

Compute complexity metrics for Verus-verified Rust code, including:
- **Specification metrics** - Halstead metrics for `requires`, `ensures`, `decreases` clauses
- **Proof metrics** - Halstead metrics for `proof { }` blocks with transitive lemma analysis
- **Code metrics** - Integration with `rust-code-analysis` for cyclomatic/cognitive complexity

### Full Pipeline

```bash
# Step 1: Generate atoms from SCIP
cargo run --bin write_atoms -- \
  index_scip.json atoms.json

# Step 2: Compute spec metrics
cargo run --bin compute_metrics -- \
  atoms.json atoms_with_metrics.json

# Step 3: Compute proof metrics (with transitive lemma analysis)
cargo run --bin compute_proof_metrics -- \
  atoms_with_metrics.json atoms_complete.json

# Step 4: Enrich CSV with code metrics (from rust-code-analysis)
cargo run --bin enrich_csv_with_metrics -- \
  functions.csv rca_output_dir/ functions_enriched.csv

# Step 5: Add spec + proof metrics to CSV
cargo run --bin enrich_csv_complete -- \
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

38 command-line tools including:

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

## CI Integration for Verus Projects

To automatically generate call graphs for a Verus project, one can use our reusable GitHub Actions workflow:

```yaml
# .github/workflows/deploy-callgraph.yml
name: Deploy Call Graph

on:
  push:
    branches: [main]

permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
    with:
      github_url: https://github.com/YOUR_ORG/YOUR_REPO
```

The interactive call graph will be deployed to GitHub Pages.

See [docs/guides/CALLGRAPH_CI_INTEGRATION.md](docs/guides/CALLGRAPH_CI_INTEGRATION.md) for advanced options including:
- Subpath deployment for existing GitHub Pages sites
- Workspace support
- Customizing verification and similar lemmas

---

## Documentation

- [METRICS_PIPELINE.md](METRICS_PIPELINE.md) - Verus metrics pipeline guide
- [docs/guides/INTERACTIVE_VIEWER.md](docs/guides/INTERACTIVE_VIEWER.md) - Web viewer documentation
- [docs/guides/VSCODE_EXTENSION.md](docs/guides/VSCODE_EXTENSION.md) - Embedding in VS Code extensions
- [docs/guides/GITHUB_PAGES_GUIDE.md](docs/guides/GITHUB_PAGES_GUIDE.md) - Online viewer guide
- [docs/guides/CALLGRAPH_CI_INTEGRATION.md](docs/guides/CALLGRAPH_CI_INTEGRATION.md) - CI integration for Verus projects
- [docs/SIMILAR_LEMMAS.md](docs/SIMILAR_LEMMAS.md) - Similar lemmas feature

## Python Scripts

| Script | Description |
|--------|-------------|
| `scripts/add_verification_status.py` | Enrich graph.json with verification status from scip-atoms |
| `scripts/enrich_graph_with_similar_lemmas.py` | Enrich graph.json with similar lemmas from verus_lemma_finder |
| `scripts/visualize_metrics.py` | Generate visualization plots from metrics data |

## License

MIT OR Apache-2.0
