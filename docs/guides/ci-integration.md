# CI Integration Guide

How to generate and deploy an interactive call graph for your Rust, Verus, or
Lean 4 project using the reusable GitHub Actions workflows in this repo.

## One-time setup: enable GitHub Pages

In your repository: **Settings → Pages → Build and deployment → Source:
GitHub Actions**. That is all; the workflows below handle the rest. The
workflow needs `pages: write` and `id-token: write` permissions, which the
examples include.

## Rust / Verus projects

### Minimal setup

`github_url` is auto-detected from the calling repository, so the minimal
workflow needs no inputs:

```yaml
# .github/workflows/callgraph.yml
name: Call Graph

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/probegraph/.github/workflows/generate-callgraph.yml@main
```

Your call graph deploys to `https://YOUR_ORG.github.io/YOUR_REPO/`.

For reproducible runs, pin the Verus release:

```yaml
    with:
      verus_version: '0.2025.11.23.41c5885'
```

### Non-Verus Rust projects

Use rust-analyzer instead of verus-analyzer and skip the Verus-specific steps:

```yaml
jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/probegraph/.github/workflows/generate-callgraph.yml@main
    with:
      use_rust_analyzer: true
      skip_verification: true
      skip_similar_lemmas: true
```

### Rust/Verus workflow inputs

All inputs are optional.

| Input | Description | Default |
|-------|-------------|---------|
| `project_path` | Path to the project relative to repo root | `.` |
| `github_url` | Repo URL for source links | auto-detected |
| `github_branch` | Branch for source links | `main` |
| `github_path_prefix` | Prefix for source file paths (e.g. `curve25519-dalek` when the crate lives in a subdirectory) | `''` |
| `package` | Cargo package name, for workspaces | `''` |
| `use_rust_analyzer` | Use rust-analyzer instead of verus-analyzer | `false` |
| `skip_verification` | Skip the Verus verification step | `false` |
| `skip_similar_lemmas` | Skip similar-lemma enrichment | `false` |
| `verus_version` | Verus release to install (e.g. `0.2025.11.23.41c5885`) | latest release |
| `rust_version` | Rust toolchain (must match the Verus version) | `1.91.0` |
| `deploy_mode` | `standalone` or `subpath` | `standalone` |
| `subpath` | URL subpath when `deploy_mode: subpath` | `callgraph` |

## Lean 4 projects

Uses [probe-lean](https://github.com/Beneficial-AI-Foundation/probe-lean) to
extract declarations, dependencies, and verification status (sorry detection):

```yaml
# .github/workflows/callgraph.yml
name: Call Graph

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/probegraph/.github/workflows/generate-lean-callgraph.yml@main
```

The workflow runs `probe-lean pipeline`, which builds the project with
`lake build`, extracts atoms, computes specification status, and maps `sorry`
warnings to declarations. Each declaration gets a `verification-status`
(`verified`, `unverified`, or `failed`) rendered with the same color coding as
Verus projects.

### Lean workflow inputs

All inputs are optional.

| Input | Description | Default |
|-------|-------------|---------|
| `project_path` | Path to the Lean project relative to repo root | `.` |
| `github_url` | Repo URL for source links | auto-detected |
| `github_branch` | Branch for source links | `main` |
| `github_path_prefix` | Prefix for source file paths | `''` |
| `probe_lean_version` | probe-lean git ref (branch, tag, SHA) | `main` |
| `use_mathlib_cache` | Run `lake exe cache get` before building. Required for Mathlib-dependent projects | `true` |
| `skip_verification` | Skip sorry detection | `false` |
| `pre_built_atoms` | Artifact name with pre-built atoms JSON; skips `lake build` and extraction entirely | `''` |
| `deploy_mode` | `standalone` or `subpath` | `standalone` |
| `subpath` | URL subpath when `deploy_mode: subpath` | `callgraph` |

The workflow aligns probe-lean's Lean toolchain to your project's
`lean-toolchain` file. Older Lean versions may need probe-lean source changes.

## Deploying to a subpath of an existing Pages site

If your repo already publishes a Pages site, generate the graph as an artifact
and merge it into your site before deploying:

```yaml
# .github/workflows/deploy-with-callgraph.yml
name: Deploy Site with Call Graph

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/probegraph/.github/workflows/generate-callgraph.yml@main
    with:
      deploy_mode: subpath
      subpath: callgraph

  build-site:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # Your existing site build steps here
      - name: Build site
        run: |
          mkdir -p _site
          cp -r docs/* _site/
      - name: Upload site artifact
        uses: actions/upload-artifact@v4
        with:
          name: main-site
          path: _site

  deploy:
    needs: [callgraph, build-site]
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - name: Download main site
        uses: actions/download-artifact@v4
        with:
          name: main-site
          path: site
      - name: Download callgraph viewer
        uses: actions/download-artifact@v4
        with:
          name: callgraph-viewer
          path: site/callgraph
      - name: Upload combined artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: site
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

The graph appears at `https://YOUR_ORG.github.io/YOUR_REPO/callgraph/`.

## Sharing graphs without a deployment

Any deployed viewer instance can load a graph from a URL with the `?json=`
parameter, so you can host just the JSON (in a repo, a gist, or any static
host with CORS) and share a link:

```
https://YOUR_ORG.github.io/YOUR_REPO/?json=https://raw.githubusercontent.com/you/repo/main/graph.json
```

## What the deployed viewer includes

- Three views: force-directed Call Graph, File Map, and Crate Map (shown as
  Namespace Map for Lean graphs)
- Verification status per node (six states, including transitively verified
  and trusted), with status filters
- Spec-clause edge types (body call / precondition / postcondition) and, for
  merged graphs, cross-language Rust-to-Lean mapping links
- Per-kind declaration filters (exec/proof/spec for Verus; axioms, types,
  projections, instances for Lean)
- Similar-lemma suggestions (Verus), source links to GitHub, search, path
  queries, and shareable filter URLs
- A Guide tab with a generated graph overview and suggested queries

See the [viewer guide](viewer.md) for usage, including how source links are
configured (`?github=` parameter, graph metadata, or `VITE_GITHUB_URL` at
build time).

## Custom domain

To serve the viewer at, e.g., `callgraph.yourdomain.com`: create
`web/public/CNAME` containing the domain, add a `CNAME` DNS record pointing to
`YOUR_USERNAME.github.io`, then set the custom domain under Settings → Pages
and enable Enforce HTTPS.

## Testing viewer changes locally

Before pushing changes that touch `web/`:

```bash
cd web
npm install
npm run type-check
npm run test:run   # the deploy workflow runs these tests too
npm run build
npm run preview
```

## Troubleshooting

**verus-analyzer fails.** Check the project analyzes locally with
verus-analyzer. If it is not a Verus project, set `use_rust_analyzer: true`.

**rust-analyzer fails.** Make sure the project compiles with `cargo check`.
Verus-specific syntax needs verus-analyzer (the default).

**Verification times out.** Set `skip_verification: true` to generate the
graph structure without verification.

**Similar lemmas missing.** Set `skip_similar_lemmas: true` if the Python
setup fails.

**Lean build fails on a Mathlib project.** Leave `use_mathlib_cache` at its
default (`true`); building Mathlib from source usually exceeds runner limits.

**Pages deploy fails or 404s.** Check the Actions log for the failed job.
Confirm Pages source is set to "GitHub Actions". First deployments can take
5-10 minutes; later ones are faster. TypeScript or build errors reproduce
locally with `npm run type-check` / `npm run build` in `web/`.
