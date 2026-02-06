# Interactive Call Graphs for Every Verus Project

*One workflow. Zero setup. Instant visibility into your codebase.*

`Beneficial-AI-Foundation/scip-callgraph`

---

## The Problem

**Understanding (Verus/Rust) codebases is hard**

- Which functions call which? 
- Which lemmas are reused, which are dead?
- Verification is expensive — where should you focus effort?
- Onboarding new contributors means navigating unfamiliar call chains

---

## The Solution

**A reusable GitHub Actions workflow that generates an interactive call graph for any Verus (or Rust) project**

```
Your Repo  →  verus-analyzer  →  SCIP index  →  Call Graph Pipeline  →  Interactive Web Viewer
               (or rust-analyzer)                 (enrichment, metrics)    (deployed to GitHub Pages)
```

Everything runs in CI. Nothing to install locally.

---

## How Simple Is It?

**Minimal setup — just add one file to your repo:**

`.github/workflows/deploy-callgraph.yml`

```yaml
name: Deploy Call Graph

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
```

That's it. GitHub URL is auto-detected. Deploys to GitHub Pages automatically.

---

## What You Get

**An interactive, searchable call graph viewer at `https://YOUR_ORG.github.io/YOUR_REPO/`**

- **Force-directed graph visualization** (D3.js) — zoom, pan, drag
- **Search** by function name, symbol path, or file name
- **Depth-based exploration** — click a node, set max depth, see its neighborhood
- **Clickable source links** — jump directly to the function on GitHub
- **Caller/callee details** — click any node to see who calls it and what it calls

---

## Enrichments Beyond a Raw Call Graph

The pipeline doesn't just extract calls. It enriches the graph with:

| Enrichment | What it does |
|---|---|
| **Verus verification status** | Marks each function as verified / unverified / proof / exec / spec |
| **Similar lemmas detection** | Finds structurally similar lemmas (potential deduplication targets) |
| **GitHub deep links** | Every node links to its exact file and line range |

---

## Configurable for Your Project

The workflow accepts optional inputs for flexibility:

```yaml
jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
    with:
      # Pin a specific Verus version for reproducibility
      verus_version: '0.2025.11.23.41c5885'

      # For workspaces, specify the package
      package: my-crate

      # If your Verus project is in a subdirectory
      project_path: crypto/core

      # Skip slow steps if you just want the graph fast
      skip_verification: true
      skip_similar_lemmas: true
```

---

## Works for Non-Verus Rust Projects Too

Not using Verus? Switch to `rust-analyzer`:

```yaml
jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
    with:
      use_rust_analyzer: true
      skip_verification: true
```

Same interactive viewer. Same GitHub Pages deployment. Same zero-setup experience.

---

## Two Deployment Modes

**Standalone** (default) — the call graph viewer IS the entire site

```
https://your-org.github.io/your-repo/  →  Call Graph Viewer
```

**Subpath** — embed alongside an existing site (e.g., a verification dashboard)

```
https://your-org.github.io/your-repo/           →  Your Dashboard
https://your-org.github.io/your-repo/callgraph/  →  Call Graph Viewer
```

```yaml
with:
  deploy_mode: subpath
  subpath: callgraph
```

---

## Real-World Example: dalek-lite

**`curve25519-dalek` verification project uses this workflow today**

- Dashboard at root, call graph at `/callgraph`
- 200+ functions indexed with verification status
- Complexity metrics help prioritize which functions to verify next
- Similar lemmas detection surfaces deduplication opportunities

---

## What the Pipeline Does Under the Hood

```
1. Checkout your repo + scip-callgraph repo
2. Install toolchain (Rust, verus-analyzer/rust-analyzer, SCIP CLI)
3. Generate SCIP index from your source code
4. Run the enrichment pipeline:
    - Parse SCIP → build call graph
    - Run Verus verification (optional)
    - Detect similar lemmas (optional)
    - Compute complexity metrics
    - Embed function source code
    - Generate GitHub deep links
5. Build the web viewer (Vite + TypeScript + D3.js)
6. Deploy to GitHub Pages
```

All cached where possible. Runs on `ubuntu-latest`.

---

## Use Cases

| Who | How they use it |
|---|---|
| **Verification engineers** | Identify unverified functions, prioritize by complexity, find similar lemmas to reuse |
| **New contributors** | Explore the codebase visually, understand call chains before reading code |
| **Code reviewers** | Check what a changed function affects (callers), what it depends on (callees) |
| **Project leads** | Track verification progress, spot dead code, assess codebase health |
| **Security auditors** | Trace data flow through the call graph, identify critical paths |

---

## Getting Started

**Three steps. Five minutes.**

1. **Enable GitHub Pages** in your repo settings (Settings → Pages → Source: GitHub Actions)

2. **Add the workflow file:**

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

jobs:
  callgraph:
    uses: Beneficial-AI-Foundation/scip-callgraph/.github/workflows/generate-callgraph.yml@main
```

3. **Push to `main`.** Done.

---

## Summary

- **Any Verus project** can get an interactive call graph by adding a single workflow file
- **Zero local setup** — everything runs in GitHub Actions
- **Rich enrichments** — verification status, similar lemmas
- **Two deployment modes** — standalone or alongside existing sites
- **Works for plain Rust too** — just flip `use_rust_analyzer: true`

**Try it today:** [github.com/Beneficial-AI-Foundation/scip-callgraph](https://github.com/Beneficial-AI-Foundation/scip-callgraph)
