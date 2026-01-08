# Call Graph CI Integration Guide

This guide explains how to integrate automatic call graph generation into your Rust or Verus project using GitHub Actions.

## Quick Start

### Option 1: Standalone Deployment (No Existing GitHub Pages)

If your project doesn't have an existing GitHub Pages site, use this simple workflow:

```yaml
# .github/workflows/deploy-callgraph.yml
name: Deploy Call Graph

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
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
    with:
      github_url: https://github.com/YOUR_ORG/YOUR_REPO
      deploy_mode: standalone
```

Your call graph will be available at `https://YOUR_ORG.github.io/YOUR_REPO/`.

### Non-Verus Rust Projects

For regular Rust projects (not using Verus), use `use_rust_analyzer: true`:

```yaml
jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
    with:
      github_url: https://github.com/YOUR_ORG/YOUR_REPO
      use_rust_analyzer: true
      skip_verification: true
      skip_similar_lemmas: true
```

This uses rust-analyzer instead of verus-analyzer for SCIP generation, and skips Verus-specific features.

### Option 2: Subpath Deployment (Existing GitHub Pages)

If your project already has a GitHub Pages site and you want the call graph at a subpath (e.g., `/callgraph`), you'll need to merge the artifacts:

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
  # Generate the call graph
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
    with:
      github_url: https://github.com/YOUR_ORG/YOUR_REPO
      github_path_prefix: ''  # Set if your source is in a subdirectory
      deploy_mode: subpath
      subpath: callgraph

  # Build your existing site
  build-site:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      # Your existing site build steps here
      # Example for a static site:
      - name: Build site
        run: |
          # Your build commands
          mkdir -p _site
          cp -r docs/* _site/  # Or however you build your site
      
      - name: Upload site artifact
        uses: actions/upload-artifact@v4
        with:
          name: main-site
          path: _site

  # Merge and deploy
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

Your call graph will be available at `https://YOUR_ORG.github.io/YOUR_REPO/callgraph/`.

## Configuration Options

| Input | Description | Default |
|-------|-------------|---------|
| `project_path` | Path to project relative to repo root | `.` |
| `github_url` | GitHub repo URL for source code links | **Required** |
| `github_path_prefix` | Prefix for source file paths (e.g., `curve25519-dalek`) | `''` |
| `package` | Cargo package name for workspaces | `''` |
| `use_rust_analyzer` | Use rust-analyzer instead of verus-analyzer | `false` |
| `skip_verification` | Skip Verus verification step | `false` |
| `skip_similar_lemmas` | Skip similar lemmas enrichment | `false` |
| `deploy_mode` | `standalone` or `subpath` | `standalone` |
| `subpath` | URL subpath (when `deploy_mode: subpath`) | `callgraph` |

## Features Included

The generated call graph includes:

- ✅ **Interactive D3.js visualization** with zoom, pan, and filtering
- ✅ **Verification status** (verified/failed/unverified) for each function
- ✅ **Similar lemmas** from vstd for each function
- ✅ **Source code links** to GitHub (click to view source)
- ✅ **Function details** including caller/callee relationships
- ✅ **Search and filter** by function name, file, verification status

## Troubleshooting

### verus-analyzer fails

Make sure your project has a valid `Cargo.toml` and can be analyzed by verus-analyzer locally. If your project is not a Verus project, use `use_rust_analyzer: true` instead.

### rust-analyzer fails

Ensure your project compiles with `cargo check`. If you're using Verus-specific syntax, you need verus-analyzer (the default).

### Verification times out

Use `skip_verification: true` to skip the verification step and just generate the call graph structure.

### Similar lemmas missing

Use `skip_similar_lemmas: true` if the Python setup fails, or ensure your project is compatible with the verus_lemma_finder.

### CORS errors loading graph

If deploying to a custom domain, ensure the `github_url` matches your actual repository URL.

