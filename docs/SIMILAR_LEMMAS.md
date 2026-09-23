# Similar Lemmas Feature

Enrich call graphs with semantically similar lemmas for each node, powered by
[verus_lemma_finder](https://github.com/Beneficial-AI-Foundation/verus_lemma_finder).

The `pipeline` binary runs this enrichment automatically (skip with
`--skip-similar-lemmas`). This page documents the standalone script.

## Setup (first time only)

```bash
uv sync --extra enrich
uv run maturin develop --release -m external/verus_lemma_finder/rust/Cargo.toml
```

`uv sync` installs the Python package but not its compiled Rust extension;
the `maturin develop` step is required.

## Quick Start

```bash
uv run python scripts/enrich_graph_with_similar_lemmas.py \
    --graph web/public/graph.json \
    --index /path/to/lemma_index.json
```

## Full Workflow

```bash
# 1. Generate call graph (from a SCIP index)
cargo run -p metrics-cli --bin export_call_graph_d3 -- project_scip.json -o graph.json

# 2. Build lemma index (in verus_lemma_finder)
cd /path/to/verus_lemma_finder
uv run python -m verus_lemma_finder index project_scip.json -o lemma_index.json

# 3. Enrich graph with similar lemmas
cd /path/to/probegraph
uv run python scripts/enrich_graph_with_similar_lemmas.py \
    --graph graph.json \
    --index /path/to/lemma_index.json \
    --top-k 3

# 4. View in web UI
cd web && npm run dev
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--graph`, `-g` | required | Input call graph JSON |
| `--index`, `-i` | required | Lemma index JSON |
| `--output`, `-o` | overwrites input | Output file path |
| `--top-k`, `-k` | 3 | Similar lemmas per node |
| `--quiet`, `-q` | false | Suppress progress output |

## Output Format

The script adds a `similar_lemmas` field to each node:

```json
{
  "display_name": "lemma_mod_bound",
  "similar_lemmas": [
    {
      "name": "lemma_mod_basics",
      "score": 0.92,
      "file_path": "src/lemmas/div_mod.rs",
      "line_number": 45,
      "signature": "pub proof fn lemma_mod_basics(x: int, m: int)",
      "source": "project"
    }
  ]
}
```

## Web Viewer

Similar lemmas appear in the Node Details panel with the lemma name,
similarity score (as a percentage), and file location.
