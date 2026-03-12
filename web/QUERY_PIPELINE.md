# Query Pipeline Architecture

This document describes the composable query pipeline that powers the scip-callgraph interactive viewer's filtering and traversal system.

> **History.** The pipeline replaced a monolithic `applyFilters()` function (~983 lines in `filters.ts`). The pre-refactor design proposal and the original filter docs are archived in `docs/archive/QUERY_ARCHITECTURE_PROPOSAL.md` and `docs/archive/FILTERS_PRE_REFACTOR.md` respectively.

## 1. Overview

The pipeline follows a **compile → execute** pattern, separating *what* the user asked from *how* it is evaluated:

```
FilterOptions (UI state)
        │
        ▼
  ┌────────────┐
  │  Compiler  │  compileQuery(filters, lang) → CompiledQuery
  └──────┬─────┘
         │  CompiledQuery { query, traversalPredicates, displayPredicates, ... }
         ▼
  ┌────────────┐
  │  Executor  │  executeQuery(compiled, fullGraph, nodeOptions) → D3Graph
  └──────┬─────┘
         │  7 pipeline steps (see §4)
         ▼
     D3Graph (filtered result)
```

### Source files

| File | Role |
|------|------|
| `src/query.ts` | Query AST, 9 operators, resolver, compiler, executor |
| `src/filters.ts` | Public entry point (`applyFilters`), pattern utilities (`globToRegex`, `matchesQuery`) |
| `src/graph-loader.ts` | JSON format normalization (SCIP, SimplifiedNode, ProbeAtom, Schema 2.0) |

The public API is a single function:

```typescript
function applyFilters(
  fullGraph: D3Graph,
  filters: FilterOptions,
  nodeOptions?: SelectedNodeOptions,
  projectLanguage?: ProjectLanguage,
): D3Graph
```

It calls `compileQuery` then `executeQuery` internally.

---

## 2. Query AST

The compiler translates `FilterOptions` into a discriminated union called `GraphQuery`, which encodes the traversal mode without performing any graph access:

```typescript
type GraphQuery =
  | { type: 'callees';          from: NodeMatcher; maxDepth: number | null }
  | { type: 'callers';          to: NodeMatcher;   maxDepth: number | null }
  | { type: 'neighborhood';     center: NodeMatcher; maxDepth: number | null }
  | { type: 'paths';            from: NodeMatcher; to: NodeMatcher }
  | { type: 'crateBoundary';    sourceCrate: string; targetCrate: string }
  | { type: 'depthFromSelected'; selectedNodes: Set<string>; maxDepth: number }
  | { type: 'noTraversal' };
```

### Node matchers

A `NodeMatcher` identifies *which* nodes to start from:

```typescript
type NodeMatcher =
  | { kind: 'pattern'; query: string }   // substring or glob against display_name
  | { kind: 'crate';   pattern: string } // crate: prefix query
  | { kind: 'nodeIds'; ids: Set<string> } // explicit set (VS Code integration)
```

### Dispatch rules

| Source field | Sink field | Compiled query type |
|-------------|-----------|---------------------|
| non-empty | empty | `callees` |
| empty | non-empty | `callers` |
| same string | same string | `neighborhood` |
| `crate:A` | `crate:B` | `crateBoundary` |
| different | different | `paths` |
| empty | empty (+ clicked nodes + depth) | `depthFromSelected` |
| empty | empty | `noTraversal` |

---

## 3. Filter Types

### 3.1 Source Query (`sourceQuery`)

Shows what functions are **called by** the matched nodes (callee direction). Traverses forward up to `maxDepth`.

**Matching syntax** (handled by `matchesQuery()` in `filters.ts`):

| Syntax | Example | Meaning |
|--------|---------|---------|
| Substring | `decompress` | Matches any `display_name` containing "decompress" |
| Glob wildcards | `lemma_*` | Anchored match (only names starting with `lemma_`) |
| Path-qualified | `edwards::decompress` | Matches `decompress` in files named `edwards.rs` or `edwards.lean` |
| Lean dotted path | `Scalar52.add_spec` | Matches nodes whose full ID contains `Scalar52.add_spec` |
| Crate-qualified | `crate:curve25519-dalek` | All functions in the named crate |

**Lean disambiguation:** When multiple Lean functions share the same `display_name` (e.g., several `add_spec` theorems), use a dotted module-path prefix from the node ID to narrow the match. For example, `Scalar52.add_spec` matches only `probe:...Scalar52.add_spec`, not the Edwards or Ristretto variants. The dotted-path match is a substring match against the full node ID and is only activated when the query contains a `.` character.

### 3.2 Sink Query (`sinkQuery`)

Shows what functions **call** the matched nodes (caller direction). Same matching syntax as source.

### 3.3 Source + Sink Combined

| Combination | Behavior |
|-------------|----------|
| Same query (source = sink) | Full neighborhood (callers + callees) |
| Both `crate:` queries | **Crate boundary mode** — only direct cross-crate calls |
| Different queries | DFS path finding from all sources to all sinks |

### 3.4 Include Files (`includeFiles`)

Comma-separated file patterns. Only functions defined in matching files pass the traversal predicate.

| Pattern | Matches |
|---------|---------|
| `edwards.rs` | All files named `edwards.rs` |
| `src/edwards.rs` | Only `*/src/edwards.rs` |
| `**/backend/**/edwards.rs` | Any `edwards.rs` under a `backend/` directory |
| `curve25519-dalek/**` | All files under `curve25519-dalek/` |

Filename patterns (no `/`) match against `file_name`; path patterns (with `/`) match against `relative_path`.

### 3.5 Exclude Name Patterns (`excludeNamePatterns`)

Comma-separated glob patterns matched against `display_name`. Example: `*_comm*, lemma_mul_*`.

### 3.6 Exclude Path Patterns (`excludePathPatterns`)

Comma-separated glob patterns matched against the node's full SCIP `id`. Example: `*/specs/*, */test/*`.

### 3.7 Function Kind Filters

| Toggle | Default | Description |
|--------|---------|-------------|
| `showExecFunctions` | true | Executable code |
| `showProofFunctions` | true | Proof functions / lemmas |
| `showSpecFunctions` | false | Specification functions |

Kind sets are language-aware (`getKindSetsForLanguage`), supporting Verus/Rust and Lean projects.

### 3.8 Call Type Filters (Link Types)

| Toggle | Default | Edge type |
|--------|---------|-----------|
| `showInnerCalls` | true | Body calls (`inner` / `calls`) |
| `showPreconditionCalls` | false | `requires` clause calls |
| `showPostconditionCalls` | false | `ensures` clause calls |

Requires/Ensures edges typically target spec functions. Enable **both** the call-type toggle and "Show Spec Functions" to see them.

### 3.9 Source Type Filters (Display Predicates)

| Toggle | Default |
|--------|---------|
| `showLibsignal` | true |
| `showNonLibsignal` | true |

Applied as **display predicates** — after traversal, so they don't affect reachability.

### 3.10 Max Depth (`maxDepth`)

Limits BFS traversal depth for `callees`, `callers`, and `neighborhood` queries. `null` or `0` means unlimited. Does **not** limit path finding.

### 3.11 Click-Based Selection (`selectedNodes`)

When no source/sink/include-files are active and `maxDepth` is set, clicking a node triggers `depthFromSelected` — bidirectional BFS from the clicked node.

### 3.12 Hidden Nodes (`hiddenNodes`)

Shift+click hides a node. Hidden nodes are excluded during the traversal-predicate phase (step 1), preventing them from appearing in any result.

---

## 4. Pipeline Steps

The `executeQuery` function runs 7 steps:

### Step 1 — Build traversable subgraph

```
selectNodes(fullGraph, traversalPredicates) → traversableGraph
```

Keeps only nodes that pass **all** traversal predicates: kind filter, exclude-name, exclude-path, include-file, hidden-nodes, build-artifact exclusion.

### Step 2 — Resolve matchers

```
resolveNodeMatcher(matcher, fullGraph, traversableIds, exactOverride) → Set<string>
```

Patterns are matched against the **full** graph (so a user can find a node even if it shares a name with a filtered-out node), then the result is intersected with the traversable set.

The `exactOverride` parameter handles VS Code integration: when the extension sends a precise SCIP symbol ID, it bypasses pattern matching entirely.

### Step 3 — Dispatch traversal

Based on the `GraphQuery.type`, one of 6 traversal paths is taken:

| Query type | Operator(s) used |
|-----------|-----------------|
| `callees` | `traverseForward` (BFS, per start node, merged) |
| `callers` | `traverseBackward` (BFS, per start node, merged) |
| `neighborhood` | `traverseForward` + `traverseBackward`, union |
| `paths` | `findPaths` (DFS with backtracking) |
| `crateBoundary` | `crateBoundary` (edge scan) |
| `depthFromSelected` | `traverseBidirectional` (undirected BFS) |
| `noTraversal` | Focus set or full traversable set |

Each traversal returns a `TraversalResult` carrying `nodeIds` plus optional side-channel data (`calleeDepths`, `callerDepths`, `boundaryLinkPairs`).

### Step 4 — Assemble result nodes

Filter `fullGraph.nodes` to only those whose `id` is in the traversal result's `nodeIds`.

### Step 5 — Apply display predicates

Post-traversal filtering: libsignal/non-libsignal toggle, plus re-application of kind filter and hidden-node exclusion (since step 4 pulls from the full graph).

### Step 6 — Filter links

Three passes over links:

1. **Endpoint filter** — keep only links where both source and target are in the result node set.
2. **Depth filter** — when a depth limit is active, keep only BFS-tree edges (no shortcut edges). Uses `calleeDepths` / `callerDepths` from the traversal result.
3. **Link type filter** — apply `showInnerCalls`, `showPreconditionCalls`, `showPostconditionCalls`.

For `crateBoundary` queries, only links whose `(source, target)` pair is in `boundaryLinkPairs` survive step 1.

### Step 7 — Cleanup and build metadata

1. **Remove isolated nodes** — nodes with no remaining edges (unless in the focus set).
2. **Build `nodeDepths`** — a `Map<string, number>` attached to the result `D3Graph` for depth-based layout coloring. Merged from forward/backward traversal depths, taking the minimum when a node appears in both.
3. **Deep copy** — nodes and links are shallow-cloned to prevent D3's force simulation from mutating the original graph.

---

## 5. Operators

Nine pure functions in `query.ts`, each taking a graph (or its parts) and returning a new structure:

| Operator | Signature | Algorithm |
|----------|-----------|-----------|
| `selectNodes` | `(graph, predicates) → D3Graph` | Linear scan, predicate conjunction |
| `traverseForward` | `(graph, startIds, maxDepth) → TraversalResult` | BFS on forward adjacency |
| `traverseBackward` | `(graph, startIds, maxDepth) → TraversalResult` | BFS on reverse adjacency |
| `traverseBidirectional` | `(graph, centerIds, maxDepth) → TraversalResult` | BFS on undirected adjacency |
| `findPaths` | `(graph, sourceIds, sinkIds) → TraversalResult` | DFS with backtracking |
| `crateBoundary` | `(graph, srcCrate, tgtCrate) → TraversalResult` | Edge scan matching crate pairs |
| `filterLinksByType` | `(links, filter) → D3Link[]` | Type predicate |
| `depthFilterLinks` | `(links, calleeDepths?, callerDepths?) → D3Link[]` | BFS-tree edge predicate |
| `removeIsolated` | `(nodes, links, keepSet?) → D3Node[]` | Connected-component filter |

All operators are individually testable; none depend on global state.

---

## 6. Key Design Decisions

### 6.1 Dual-role focus nodes

`focusNodeIds` is only used in the `noTraversal` case (no source/sink query). When a traversal is active, focus nodes have no effect — the traversal result fully determines what is shown.

### 6.2 Traversal predicates vs. display predicates

**Traversal predicates** (kind, exclude-name, exclude-path, include-file, hidden, build-artifact) are applied *before* traversal in step 1. They determine the reachable subgraph.

**Display predicates** (libsignal/non-libsignal) are applied *after* traversal in step 5. They don't affect reachability: a path through a libsignal node is still found, but the libsignal node itself may be hidden from the result.

### 6.3 Full-graph matcher resolution

`resolveNodeMatcher` matches patterns against the **full** graph, not the traversable subgraph. This prevents confusing situations where a function "exists" but can't be found because some predicate filtered it out. The matched IDs are then intersected with the traversable set to ensure only valid start nodes are used.

### 6.4 VS Code exact override

When VS Code sends a `selectedNodeId` (a precise SCIP symbol), it bypasses pattern matching entirely. If the exact ID doesn't exist in the graph (e.g., stale index), it falls back to normal pattern matching.

### 6.5 TraversalResult side-channel

Rather than encoding depth information in the node objects, traversal operators return a `TraversalResult` with optional `calleeDepths`, `callerDepths`, and `boundaryLinkPairs` maps. The executor uses these for depth-based link filtering (step 6) and for the `nodeDepths` metadata attached to the final graph (step 7).

### 6.6 Compiler purity

`compileQuery` is a pure function — it takes `FilterOptions` and returns a `CompiledQuery` with no graph access. This makes it trivially testable and allows potential future caching of compiled queries.

---

## 7. URL Parameters

Filter state is encoded in shareable URLs:

| Parameter | Filter | Example |
|-----------|--------|---------|
| `source` | Source query | `?source=decompress` |
| `sink` | Sink query | `?sink=validate` |
| `files` | Include files | `?files=edwards.rs,scalar.rs` |
| `depth` | Max depth | `?depth=3` |
| `exec` | Show exec functions (0/1) | `?exec=0` |
| `proof` | Show proof functions (0/1) | `?proof=1` |
| `spec` | Show spec functions (0/1) | `?spec=1` |
| `inner` | Show body calls (0/1) | `?inner=1` |
| `pre` | Show requires calls (0/1) | `?pre=1` |
| `post` | Show ensures calls (0/1) | `?post=1` |
| `libsignal` | Show libsignal code (0/1) | `?libsignal=0` |
| `external` | Show external code (0/1) | `?external=0` |
| `excludeName` | Exclude name patterns | `?excludeName=*_comm*` |
| `excludePath` | Exclude path patterns | `?excludePath=*/specs/*` |
| `hidden` | Hidden node names (comma-separated) | `?hidden=foo,bar` |
| `view` | Active view | `?view=crate-map` |
| `source-crate` | Source crate for frontier | `?source-crate=libsignal-core` |
| `target-crate` | Target crate for frontier | `?target-crate=curve25519-dalek` |

---

## 8. Testing

Three tiers of tests ensure correctness and prevent regressions:

### 8.1 Unit tests (`query.test.ts`) — 45 tests

Test individual operators, the compiler, and the resolver in isolation with small hand-crafted graphs. Examples:

- `selectNodes` respects kind filter, exclude patterns, hidden nodes
- `traverseForward` / `traverseBackward` respect maxDepth
- `findPaths` finds all nodes on any source→sink path
- `compileQuery` produces correct `GraphQuery` variants for all dispatch rules
- `resolveNodeMatcher` handles exact override, falls back on miss

### 8.2 Golden / snapshot tests (`query.integration.test.ts`) — 20 tests

Load real graph data (Verus SCIP graph, Verus atoms, Lean atoms) through `applyFilters` and assert exact output counts and specific node presence. These lock down the current behavior as a regression baseline. Examples:

- Full graph (no filters) produces exactly N nodes and M links
- Forward traversal from a known function at depth 1 returns expected callees
- Path finding between two functions returns expected intermediate nodes
- Crate boundary between two crates returns expected frontier

### 8.3 Backward-compatibility tests (`filters.test.ts`) — 53 tests

The original test suite from before the refactoring. These tests exercise `applyFilters`, `matchesQuery`, `globToRegex`, `pathPatternToRegex`, and other utilities. They continue to pass unchanged, confirming that the refactoring preserved all external behavior.

---

**Last updated:** March 2026
