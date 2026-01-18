# Data for find_dalek_called_by_signal.py

This folder contains the input JSON files used by the `scripts/find_dalek_called_by_signal.py` script.

## Files

| File | Description |
|------|-------------|
| `graph.json` | SCIP call graph from rust-analyzer indexing libsignal + curve25519-dalek |
| `specs.json` | Verus specifications (requires/ensures clauses) from probe-verus |
| `atoms.json` | Atom data with dependencies and locations from probe-verus |

## Source Repositories

- **dalek-lite**: https://github.com/Beneficial-AI-Foundation/dalek-lite
- **libsignal_focus_dalek_lite**: https://github.com/Beneficial-AI-Foundation/libsignal_focus_dalek_lite

## Regenerating

To regenerate these files:

### 1. graph.json

Run the scip-callgraph pipeline on the libsignal workspace:

```bash
cargo run --release --bin pipeline ../libsignal_focus_dalek_lite --skip-verification --skip-similar-lemmas --use-rust-analyzer
```

Where `libsignal_focus_dalek_lite` is a local clone of https://github.com/Beneficial-AI-Foundation/libsignal_focus_dalek_lite

### 2. atoms.json

Run probe-verus atomize on dalek-lite:

```bash
probe-verus atomize ../dalek-lite --with-locations --regenerate-scip
```

Where `dalek-lite` is a local clone of https://github.com/Beneficial-AI-Foundation/dalek-lite

### 3. specs.json

Run probe-verus specify on dalek-lite (requires atoms.json to exist first):

```bash
probe-verus specify ../dalek-lite --with-scip-names atoms.json --with-spec-text
```

## Usage

The `find_dalek_called_by_signal.py` script automatically looks for these files in this directory.
