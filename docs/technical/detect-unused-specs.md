# Detect Unused Verus Specs and Proofs

Identifies specification and proof functions that no other code in the
project references, using a SCIP-derived call graph.

## How It Works

1. **Get a SCIP index**: runs `verus-analyzer scip` on the project, or uses
   a provided SCIP JSON.
2. **Build the call graph** and write it as atoms JSON
   (`<project_dir>_atoms.json`).
3. **Classify functions** as specs or proofs from their source text (see
   Detection Logic).
4. **Find unused ones**: a spec/proof is flagged when its identifier appears
   in no other atom's `deps` list.
5. **Report**: console summary plus a structured JSON file.

## Usage

```bash
# Auto-generate the SCIP index
cargo run -p metrics-cli --bin detect_unused_specs -- /path/to/rust/project

# Use an existing SCIP JSON
cargo run -p metrics-cli --bin detect_unused_specs -- /path/to/project /path/to/index.json
```

Requirements when auto-generating: `verus-analyzer` and the `scip` CLI on
`PATH`, and a valid Cargo project.

## Output

- Console: numbered lists of potentially unused specs and proofs, with file,
  path, and the declaration line.
- JSON: **`<project_dir>_unused_specs_proofs.json`** (the directory name of
  the project path, e.g. `curve25519-dalek_unused_specs_proofs.json`).

### JSON structure

```json
{
  "summary": {
    "total_unused_specs": 3,
    "total_unused_proofs": 2,
    "total_unused_combined": 5,
    "specs_by_visibility":  { "public": 1, "pub_crate": 0, "pub_open": 0, "pub_closed": 0, "private": 2 },
    "proofs_by_visibility": { "public": 0, "pub_crate": 0, "pub_open": 0, "pub_closed": 0, "private": 2 }
  },
  "unused_specs":  [ { "identifier": "...", "display_name": "...", "visibility": "private",
                       "file_name": "...", "relative_path": "...", "full_path": "...",
                       "declaration": "pub spec fn helper_spec(x: u32) -> bool" } ],
  "unused_proofs": [ ...same shape... ],
  "warnings": [ "...false-positive caveats..." ]
}
```

Visibility is extracted from the declaration prefix and is one of `pub`,
`pub(crate)`, `pub open`, `pub closed`, or `private`.

Quick queries:

```bash
jq '.summary.total_unused_combined' myproject_unused_specs_proofs.json
jq '.unused_specs[] | select(.visibility == "pub") | .display_name' myproject_unused_specs_proofs.json
```

## Detection Logic

Classification is text-based on the lowercased function body:

- **Spec** (`is_spec_function`): contains `spec fn` or `spec(`, starts with
  `spec `, or contains both `#[verifier` and `spec`.
- **Proof** (`is_proof_function`): contains `proof fn`, starts with
  `proof `, contains both `#[verifier` and `proof`, or contains
  `fn lemma_` / `fn proof_`.

**Unused** means: the identifier appears in no other function's `deps` in
the call graph.

## False Positives

The tool may flag specs/proofs that are in fact needed:

1. **Public API specs**: meant for library consumers, so no internal callers.
2. **Entry-point theorems**: top-level correctness properties nobody calls.
3. **Proof-context usage** that doesn't surface in the call graph.
4. **Macro-generated references**.
5. **Test-only specifications**.

## Recommended Workflow

1. Run the tool to get the candidate list.
2. Manually review each: public API? theorem entry point? referenced from
   docs or proofs?
3. Remove only after review, then run the test suite and verification.

## CI Integration

Use the reusable workflow
[`detect-unused-specs-with-release.yml`](../../.github/workflows/detect-unused-specs-with-release.yml)
rather than hand-rolling a job — it downloads a release binary and runs the
analysis for you.

> Known issue: that workflow's artifact-upload step globs
> `*_unused_specs.json`, but the tool now writes
> `*_unused_specs_proofs.json`, so the artifact upload matches nothing until
> the glob is fixed.

## Related Tools

- `generate_index_scip_json`: just generates the SCIP JSON index
- `write_atoms`: converts a call graph to atoms JSON
- `generate_call_graph_dot`: visualizes the full call graph
