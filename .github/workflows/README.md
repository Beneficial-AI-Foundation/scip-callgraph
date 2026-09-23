# Workflows

CI/CD workflows for probegraph. External projects wanting a call graph should
start from the [CI integration guide](../../docs/guides/ci-integration.md)
instead of this file.

## Reusable workflows (called from other repos via `workflow_call`)

### `generate-callgraph.yml`

Generates an enriched call graph for a Rust/Verus project and deploys the web
viewer to GitHub Pages. Installs Verus (or rust-analyzer with
`use_rust_analyzer: true`), produces the SCIP index, runs verification and
similar-lemma enrichment (both skippable), builds the viewer, deploys
standalone or to a subpath. All inputs optional; `github_url` is auto-detected.
Full input table in the CI integration guide.

### `generate-lean-callgraph.yml`

Same idea for Lean 4 projects, via `probe-lean pipeline`: `lake build` (with
`lake exe cache get` first unless `use_mathlib_cache: false`), atom extraction,
sorry detection, viewer build, Pages deploy. Aligns probe-lean's toolchain to
the target project's `lean-toolchain`. `pre_built_atoms` skips build and
extraction when the atoms JSON is already available as an artifact.

### `detect-unused-specs-with-release.yml`

Runs unused specs/proofs analysis on the calling repo using a prebuilt
`detect_unused_specs` binary from this repo's releases. Inputs: `project_path`
(default `.`) and `scip_callgraph_version` (a release tag, default `latest`;
the input name predates the repo rename and is kept for compatibility).

## Internal workflows

### `build.yml` — CI

On push/PR to `main`. Two jobs:

- **web-tests**: Node 20, `npm ci`, `npm run type-check`, `npm run test:run`
  in `web/`.
- **build** (Linux, Windows, macOS x86_64): clippy, `cargo build --workspace`,
  `cargo test --workspace`, then release-builds six key binaries and smoke-tests
  them. `cargo fmt --check` runs with `continue-on-error`, so formatting
  failures do not fail CI. Uses cargo caching.

### `release.yml` — release distribution

On tags `v*.*.*` or manual dispatch (with a `version` input). For each of four
targets (Linux x86_64, macOS x86_64 and aarch64, Windows x86_64), builds 12
binaries — detect_unused_specs, generate_index_scip_json,
generate_call_graph_dot, generate_function_subgraph_dot,
generate_file_subgraph_dot, generate_files_subgraph_dot, write_atoms,
run_full_pipeline, compute_metrics, compute_proof_metrics,
enrich_csv_with_metrics, enrich_csv_complete — and packages them with README
and METRICS_PIPELINE.md as `probegraph-<version>-<target>.tar.gz` (`.zip` on
Windows), then creates a GitHub Release with the archives attached. No build
cache, for reproducibility.

### `deploy-pages.yml` — publish the demo viewer

On push to `main` touching `web/**` (or manual dispatch). Runs the web tests,
builds the viewer with `VITE_GRAPH_JSON_URL` / `VITE_GITHUB_URL` /
`VITE_GITHUB_PATH_PREFIX` (dispatch inputs override the defaults), and deploys
to this repo's GitHub Pages.

## Making a release

```bash
./scripts/release.sh v5.1.0
```

or manually: `git tag v5.1.0 && git push origin v5.1.0`, or trigger the Release
workflow from the Actions UI with the version tag. Release notes and archives
are generated automatically. Note the workspace `Cargo.toml` version is not
kept in sync with release tags; tags are the source of truth for versions.

## Adding a new binary to releases

Add the `[[bin]]` entry in `crates/metrics-cli/Cargo.toml`, then add the
`--bin` flag and the archive `cp` line in `release.yml` (and in `build.yml` if
it should be smoke-tested in CI).
