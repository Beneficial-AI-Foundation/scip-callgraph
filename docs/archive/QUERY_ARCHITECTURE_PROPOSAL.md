# Query Architecture: Composable Graph Views for scip-callgraph

## 1. Problem Statement

The scip-callgraph interactive viewer currently implements six distinct graph
query patterns (forward reachability, backward reachability, neighborhood,
path finding, crate boundary projection, and predicate-filtered subgraph)
inside a single 430-line `applyFilters()` function with deeply nested branching
logic. Each filter configuration implicitly selects a query mode, but the
mapping is encoded procedurally rather than declared structurally.

This makes it difficult to:
- Add new query modes without understanding the full branching cascade
- Test individual query behaviors in isolation
- Reason about which query is executing for a given filter configuration
- Compose queries (e.g., "callers of X, restricted to crate Y")
- Offer named view presets or a query inspector UI

This document describes the architectural refactoring toward a **composable
query pipeline** where filters compile into explicit graph queries, and views
are the deterministic output of those queries.

---

## 2. Position in the Landscape

### 2.1 Existing Tools and Their Query/View Models

| Tool | Domain | Query Mechanism | View Switching | Open Source |
|------|--------|----------------|----------------|-------------|
| **Gephi** | General graphs | Composable query tree (Filter -> Query -> View) | Workspaces; filter results exported to new workspace | Yes (GPLv3) |
| **Neo4j Bloom** | Property graphs | Perspectives (named Cypher queries + visual config) | Perspective switching (each defines visibility, styling, search phrases) | No |
| **Graphistry (GFQL)** | Large-scale analytics | Chain-based DSL: `n(filter) -> e_forward(hops) -> n(filter)` | Single GPU-rendered view; subgraph extraction via query | Partial (PyGraphistry is open) |
| **Cytoscape.js / Graphology + Sigma.js** | Bio / general networks | Node/edge attribute filtering; `nodeReducer`/`edgeReducer` | Hidden attributes; layout switching | Yes (MIT) |
| **Graph-loom** | Local graph notebook | Full OpenCypher console (MATCH, CREATE, etc.) | Physics layout + table view | Yes (Rust, 2026) |
| **DepEye** | Code dependencies | Filter by open files, directories, exclusions | Method-level vs class-level toggle | Yes (VS Code) |
| **marco** | Binary analysis | Neo4j Cypher + preset query templates | 3D force graph, table, clustering | Yes |
| **CodeGraph (optave)** | Code dependencies | MCP server with 30 tools; tree-sitter based | CLI-only | Yes |
| **Quantickle** | Threat intel / general | Cytoscape.js with 20+ layouts; dynamic filtering | Graph, table, JSON views | Yes (Apache 2.0) |

### 2.2 Where scip-callgraph Is Unique

scip-callgraph occupies a distinct niche that no existing tool covers:

**Language-agnostic call graph visualization with verification-aware semantics.**

The viewer accepts any JSON conforming to its schema -- whether generated from
Rust (via SCIP/rust-analyzer), Verus (via verus-analyzer or probe-verus), or
Lean (via probe-lean). The key invariant is structural, not linguistic: as long
as the input provides the required node and link fields, the viewer works.

This is achieved through a normalization layer (`parseAndNormalizeGraph`) that
accepts four input formats and converts them all to a unified `D3Graph`:

```
                    ┌──────────────────┐
                    │  D3Graph JSON    │  (Rust/SCIP pipeline)
                    │  nodes + links   │
                    ├──────────────────┤
                    │  SimplifiedNode[]│  (curve25519-dalek style)
Input formats ──►   ├──────────────────┤  ──► parseAndNormalizeGraph() ──► D3Graph
                    │  ProbeAtom dict  │  (probe-verus / probe-lean)
                    ├──────────────────┤
                    │  Schema 2.0      │  (envelope wrapping any above)
                    │  envelope        │
                    └──────────────────┘
```

**The minimal schema that enables all viewer features:**

```typescript
// Required per node
interface MinimalNode {
  id: string;            // Unique identifier
  display_name: string;  // Human-readable function/theorem name
  dependencies: string[];// Outgoing edges (what this calls/uses)
}

// Optional per node (enables richer features)
interface OptionalNodeFields {
  relative_path: string; // Enables file filtering, blueprint grouping
  file_name: string;     // Enables include-files filter
  kind: string;          // Enables kind filtering (exec/proof/spec/theorem/def/axiom)
  crate_name: string;    // Enables crate map, crate boundary queries
  start_line: number;    // Enables source navigation (VS Code integration)
  end_line: number;
  verification_status: string;  // Enables verification coloring
}
```

**Capabilities that emerge from language agnosticism:**

| Feature | Rust/SCIP | Verus | Lean | What enables it |
|---------|-----------|-------|------|-----------------|
| Call graph traversal | Yes | Yes | Yes | `id` + `dependencies` |
| File-based views | Yes | Yes | Yes | `relative_path` + `file_name` |
| Kind filtering | exec only | exec/proof/spec | theorem/def/axiom/... | `kind` field |
| Verification status | No | Yes | Partial | `verification_status` |
| Crate/module map | Yes | Yes | Yes | `crate_name` (or derived from `id`/`relative_path`) |
| Blueprint grouping | Yes | Yes | Yes | `relative_path` |
| Source navigation | Yes | Yes | Yes | `start_line` + `end_line` |
| Pre/postcondition edges | Partial | Yes | Yes | `link.type` = inner/precondition/postcondition |

No other tool in the landscape combines:
1. Multiple visualization modes (force-directed, file-grouped, crate-level)
2. Rich traversal queries (BFS, DFS path-finding, crate boundary)
3. Verification-aware semantics (kind, status, pre/postcondition edges)
4. Language agnosticism via schema normalization
5. Zero infrastructure (static site, no database server)

### 2.3 Academic Foundations

The refactoring draws on established theory:

**Regular Datalog and Graph Views (Dumbrava et al., 2018).** Bonifati,
Dumbrava, and Gallego Arias developed a Coq-verified incremental graph query
engine based on Regular Datalog (RD) -- a fragment of non-recursive Datalog
with transitive closure as a primitive. Their key insight: *graph views are the
deterministic output of graph queries*, and when queries are expressed in a
logic-based language, their evaluation can be formally verified and
incrementally maintained.

Our filter system already implements the core RD operations without naming
them: `getCalleesRecursive` is transitive closure along the `calls` relation;
`findPathNodes` is a conjunctive reachability query; `applyFilters` is a
stratified evaluation pipeline. The refactoring makes this correspondence
explicit.

**GQL / SQL-PGQ (ISO/IEC 39075:2024).** The emerging graph query standard
defines graph pattern matching with node/edge predicates and variable-length
path patterns -- the same patterns our source/sink queries express. While we
don't implement GQL, our query AST uses the same conceptual vocabulary: node
matchers, edge traversals with direction and depth bounds, and path patterns
between matched endpoints.

**GFQL Chain Model (Graphistry).** Graphistry's GFQL demonstrates that all
graph queries can be expressed as chains of node matchers and edge traversals.
Our current source/sink/path model maps directly:

```
Source only:     chain([ n(source), e_forward(maxDepth) ])
Sink only:       chain([ n(sink), e_backward(maxDepth) ])
Neighborhood:    chain([ n(center), e_both(maxDepth) ])
Path finding:    chain([ n(source), e_forward(to_fixed_point), n(sink) ])
Crate boundary:  chain([ n(crate:A), e_forward(hops=1), n(crate:B) ])
```

The refactoring introduces this structure explicitly rather than encoding it
in branching logic.

---

## 3. Current Architecture

```
┌──────────────┐     ┌─────────────────────────────────────────────────────┐
│ FilterOptions│────►│              applyFilters()                         │
│ (flat struct)│     │                                                     │
└──────────────┘     │  1. Parse patterns, build modeAllowedNodeIds        │
                     │  2. Match source/sink queries                       │
                     │  3. Branch on 7 traversal modes (nested if/else)    │
                     │  4. Post-filter (libsignal, kind, hidden)           │
                     │  5. Filter links (type, depth-consistency)          │
                     │  6. Remove isolated nodes                           │
                     │                                                     │
                     └──────────────────────┬──────────────────────────────┘
                                            │
                                            ▼
                     ┌─────────────────────────────────────────────────────┐
                     │                  D3Graph                            │
                     │  (consumed by CallGraph / Blueprint / CrateMap)     │
                     └─────────────────────────────────────────────────────┘
```

**Problems with this design:**

- The 7 traversal modes are implicit in the branching logic of a single
  function. Adding an 8th (e.g., "transitive callers restricted to a crate")
  requires modifying deeply nested conditionals.
- Node predicate filtering is interleaved with traversal logic. The same
  kind/pattern/file predicates are applied in 3 different places.
- Traversal predicates (what BFS/DFS can walk through) and display predicates
  (what appears in the final view) are not distinguished. The libsignal filter
  is intentionally a display-only predicate (traversal walks through external
  code, then hides it from the result), but this design choice is implicit.
- Source/sink matching runs against the full graph, not the traversable
  subgraph, so that a spec function can still be named as a source even when
  specs are hidden. This is correct but undocumented in the code structure.
- Testing requires constructing full `FilterOptions` objects and asserting on
  the final `D3Graph`, making it impossible to test traversal logic
  independently from predicate logic.
- The function returns a `D3Graph` but internally tracks auxiliary state
  (`calleeDepths`, `callerDepths`, `boundaryLinkPairs`) that is discarded or
  partially exposed via `nodeDepths`.
- A 7th traversal mode (click-based depth expansion from `selectedNodes`) is
  embedded after the main traversal branch, using `computeDepthFromSelected` --
  it only fires when there is no source/sink query, no includeFiles, but the
  user has clicked nodes with a depth limit set.

---

## 4. Proposed Architecture: Composable Query Pipeline

```
┌──────────────┐     ┌───────────────┐     ┌────────────────┐     ┌──────────┐
│ FilterOptions│────►│ compileQuery()│────►│ executeQuery() │────►│ D3Graph  │
│ (flat struct)│     │               │     │                │     │          │
└──────────────┘     │ Returns:      │     │ Pipeline:      │     └──────────┘
                     │  GraphQuery   │     │  1. predicate  │
                     │  (AST)        │     │  2. traversal  │
                     │  +            │     │  3. post-filter│
                     │  predicates   │     │  4. link filter│
                     └───────────────┘     │  5. cleanup    │
                                           └────────────────┘
```

### 4.1 Query AST

A discriminated union that makes the traversal mode explicit:

```typescript
type NodeMatcher =
  | { kind: 'pattern'; query: string }
  | { kind: 'crate'; pattern: string }
  | { kind: 'nodeIds'; ids: Set<string> }

type GraphQuery =
  | { type: 'callees'; from: NodeMatcher; maxDepth: number | null }
  | { type: 'callers'; to: NodeMatcher; maxDepth: number | null }
  | { type: 'neighborhood'; center: NodeMatcher; maxDepth: number | null }
  | { type: 'paths'; from: NodeMatcher; to: NodeMatcher }
  | { type: 'crateBoundary'; sourceCrate: string; targetCrate: string }
  | { type: 'depthFromSelected'; selectedNodes: Set<string>; maxDepth: number }
  | { type: 'noTraversal' }
```

The `depthFromSelected` variant captures the click-based depth expansion mode
(bidirectional BFS from clicked nodes). It fires when: no source/sink query,
no includeFiles, but the user has selected nodes with a depth limit.

### 4.2 Predicate Separation: Traversal vs Display

A critical design choice: predicates are split into two groups with different
roles in the pipeline.

```typescript
/** Traversal predicates: determine which nodes BFS/DFS can walk through.
 *  Applied BEFORE traversal to build the traversable subgraph. */
interface TraversalPredicates {
  kindFilter: (node: D3Node) => boolean;
  excludeNamePatterns: RegExp[];
  excludePathPatterns: RegExp[];
  includeFilePatterns: IncludeFilePattern[];
  hiddenNodes: Set<string>;
  excludeBuildArtifacts: boolean;
}

/** Display predicates: determine which traversal results appear in the view.
 *  Applied AFTER traversal. Intentionally NOT in the traversal subgraph so
 *  that BFS/DFS can walk through nodes that will later be hidden.
 *
 *  Example: libsignal filter is display-only. You want to find paths
 *  THROUGH external code and then optionally hide the external nodes. */
interface DisplayPredicates {
  showLibsignal: boolean;
  showNonLibsignal: boolean;
}

/** Focus set: dual role.
 *  (1) In noTraversal mode: restricts which nodes are shown (pre-filter).
 *  (2) In removeIsolated: preserves focus nodes even if they have no edges. */
interface FocusConfig {
  focusNodeIds: Set<string>;
}
```

This separation makes explicit what the current code does implicitly: the
libsignal filter is deliberately excluded from `modeAllowedNodeIds` (line 244)
so that traversal can walk through external code.

### 4.3 Matcher Resolution

Source/sink matching runs against the **full graph**, not the traversable
subgraph. This is intentional: a spec function can be named as a source even
when specs are hidden from the traversal. The matched IDs are then filtered to
the traversable set before the actual traversal begins.

When the VS Code extension provides an exact `selectedNodeId`, it overrides
normal query matching -- the pattern-based `matchesQuery` is bypassed in favor
of an exact ID lookup. This is handled in the resolution step, not in the
query AST, keeping the AST clean of VS Code concerns.

```typescript
/** Resolve a NodeMatcher to concrete node IDs.
 *  @param fullGraph - resolve patterns against ALL nodes
 *  @param traversableIds - filter results to traversable set before traversal
 *  @param exactOverride - VS Code exact ID override (bypasses pattern matching) */
function resolveNodeMatcher(
  matcher: NodeMatcher,
  fullGraph: D3Graph,
  traversableIds: Set<string>,
  exactOverride?: string | null
): Set<string>
```

### 4.4 Graph Operators

Pure functions, each independently testable. Traversal operators return a
`TraversalResult` carrying side-channel metadata (depths, boundary links)
needed by downstream pipeline steps.

```typescript
/** Side-channel data produced during traversal, consumed by link filtering
 *  and layout. Not a D3Graph -- the executor assembles the final graph. */
interface TraversalResult {
  nodeIds: Set<string>;
  calleeDepths?: Map<string, number>;
  callerDepths?: Map<string, number>;
  boundaryLinkPairs?: Set<string>;
}
```

| Operator | Signature | Extracted from |
|----------|-----------|---------------|
| `selectNodes` | `(graph, predicate) -> graph` | Lines 244-253, 489-534 |
| `traverseForward` | `(graph, startIds, maxDepth) -> TraversalResult` | `getCalleesRecursive` (L804-842) |
| `traverseBackward` | `(graph, startIds, maxDepth) -> TraversalResult` | `getCallersRecursive` (L847-885) |
| `traverseBidirectional` | `(graph, centerIds, maxDepth) -> TraversalResult` | `computeDepthFromSelected` (L891-944) |
| `findPaths` | `(graph, sourceIds, sinkIds) -> TraversalResult` | `findPathNodes` (L720-791) |
| `crateBoundary` | `(graph, srcCrate, tgtCrate) -> TraversalResult` | Lines 375-401 |
| `filterLinks` | `(links, allowedTypes) -> links` | Lines 590-606 |
| `depthFilterLinks` | `(links, calleeDepths, callerDepths) -> links` | Lines 558-587 |
| `removeIsolated` | `(nodes, links, keepSet?) -> nodes` | Lines 608-624 |

### 4.5 Compiler: FilterOptions -> GraphQuery

The `compileQuery` function is a pure mapping with no graph access:

```
sourceQuery  sinkQuery   isSame?  isCrate?  selectedNodes  -->  GraphQuery type
-----------  ---------   ------   -------   -------------       ---------------
non-empty    empty       -        -         -                   callees
empty        non-empty   -        -         -                   callers
non-empty    non-empty   yes      -         -                   neighborhood
non-empty    non-empty   no       both      -                   crateBoundary
non-empty    non-empty   no       no        -                   paths
empty        empty       -        -         non-empty+depth     depthFromSelected
empty        empty       -        -         -                   noTraversal
```

The `depthFromSelected` variant is only emitted when `selectedNodes` is
non-empty, `maxDepth` is set, AND no source/sink/includeFiles query is active.
This matches the condition at line 508 of the current code.

### 4.6 Executor: Standard Pipeline

The executor runs a fixed 7-step pipeline, dispatching on the query type for
step 3:

```
Step 1: Build traversable subgraph
        selectNodes(fullGraph, traversalPredicates) -> traversableGraph
        (kind, exclude, include, hidden, build artifacts -- NOT libsignal)

Step 2: Resolve matchers against fullGraph
        resolveNodeMatcher(matcher, fullGraph, traversableIds, exactOverride?)
        Returns concrete matched IDs, filtered to the traversable set.
        If a query was entered but nothing matched: return empty graph.

Step 3: Execute query (dispatch on GraphQuery.type)
        callees          -> traverseForward(traversableGraph, matchedSources, maxDepth)
        callers          -> traverseBackward(traversableGraph, matchedSinks, maxDepth)
        neighborhood     -> traverseForward + traverseBackward, merge results
        paths            -> findPaths(traversableGraph, matchedSources, matchedSinks)
        crateBoundary    -> crateBoundary(traversableGraph, srcCrate, tgtCrate)
        depthFromSelected-> traverseBidirectional(traversableGraph, selectedNodes, maxDepth)
        noTraversal      -> use traversableGraph as-is (+ focusNodeIds restriction)

Step 4: Assemble result nodes
        Filter fullGraph.nodes to TraversalResult.nodeIds

Step 5: Apply display predicates (post-filters)
        libsignal/external filter on result nodes

Step 6: Filter links
        Keep links between valid nodes
        Apply boundaryLinkPairs restriction (crate boundary mode)
        Apply depthFilterLinks (path-based modes with maxDepth)
        Apply link type filter (inner/pre/post)

Step 7: Cleanup
        removeIsolated(result, focusNodeIds)
        Build nodeDepths from TraversalResult depths
        Return D3Graph with fresh node/link copies
```

### 4.6 Backward Compatibility

The public `applyFilters()` function retains its exact signature:

```typescript
export function applyFilters(
  fullGraph: D3Graph,
  filters: FilterOptions,
  nodeOptions?: SelectedNodeOptions,
  projectLanguage?: ProjectLanguage
): D3Graph {
  const predicates = buildPredicates(filters, projectLanguage);
  const query = compileQuery(filters, nodeOptions);
  return executeQuery(query, fullGraph, predicates, filters);
}
```

All 37 call sites in `main.ts` continue working unchanged. The existing test
suite in `filters.test.ts` passes without modification.

---

## 5. File Layout

```
web/src/
├── query.ts          NEW   Query AST, operators, compiler, executor
├── filters.ts        SLIM  applyFilters wrapper + pattern utilities (matchesQuery, glob, etc.)
├── filters.test.ts   SAME  Existing tests (backward compat)
├── query.test.ts     NEW   Operator-level + compiler tests
├── types.ts          SAME  No changes needed
├── main.ts           SAME  All 37 call sites unchanged
├── graph.ts          SAME  Call Graph view
├── blueprint.ts      SAME  Blueprint view
├── crate-map.ts      SAME  Crate Map view
├── graph-utils.ts    SAME  transitiveReduction
└── status.ts         SAME  computeDerivedStatuses
```

---

## 6. What This Enables (Future Work)

### 6.1 Named View Presets

A preset is a serializable `{ query: GraphQuery, predicates: Partial<NodePredicates> }`:

```typescript
const presets = {
  "Verification Frontier": {
    query: { type: 'callers', to: { kind: 'pattern', query: 'crate:vstd' }, maxDepth: 1 },
    predicates: { kindFilter: onlyUnverified }
  },
  "Cross-Crate API": {
    query: { type: 'crateBoundary', sourceCrate: 'curve25519-dalek', targetCrate: 'libsignal-protocol' }
  }
};
```

### 6.2 Query Inspector

Display the compiled query in the UI:
"Showing: callees of `decompress` at depth 2, excluding specs, in files matching `edwards.rs`"

### 6.3 Query Composition

Because operators are pure functions, they compose naturally:

```typescript
// "Callers of X within crate Y" -- currently impossible without modifying applyFilters
const inCrate = selectNodes(graph, n => n.crate_name === 'curve25519-dalek');
const callers = traverseBackward(inCrate, matchedSinks, 3);
```

### 6.4 Incremental View Maintenance

Following Dumbrava's approach, operators with delta semantics could update views
when only one predicate changes (e.g., toggling "show spec functions") without
recomputing the full traversal. At current graph sizes (hundreds to low
thousands of nodes) this is a performance luxury, not a necessity -- but the
architecture makes it possible.

### 6.5 New Language Support

Adding support for a new language (e.g., Haskell, Agda, Coq) requires only:
1. A tool that emits JSON conforming to the minimal schema (id, display_name, dependencies)
2. Optional: a `kind` mapping for the language's declaration types
3. Optional: a `detectProjectLanguage` case for kind-filter labels

No changes to the query pipeline, operators, or visualization layer.

---

## 7. References

- A. Bonifati, S. Dumbrava, E.J. Gallego Arias. *Certified Graph View
  Maintenance with Regular Datalog.* TPLP 18(3-4):372-389, 2018.
  [HAL](https://hal.science/hal-01932818v1)
  | [GitHub (VerDILog)](https://github.com/VerDILog/)

- D. Deutsch, N. Francis, A. Green, K. Hare, B. Li, L. Libkin, T. Lindaaker,
  V. Marsault, W. Martens, J. Michels, F. Murlak, S. Plantikow, P. Selmer,
  O. van Rest, H. Voigt, D. Vrgoc, M. Wu, F. Zemke. *GQL and SQL/PGQ:
  Theoretical Models and Expressive Power.* SIGMOD 2024.
  [arXiv:2409.01102](https://arxiv.org/abs/2409.01102)

- Graphistry. *GFQL: The Dataframe-Native Graph Query Language.*
  [Docs](https://pygraphistry.readthedocs.io/en/latest/gfql/index.html)
  | [Thinking in Chain](https://hub.graphistry.com/docs/GFQL/gfql-chaining/)

- Gephi Filter Architecture.
  [Docs](https://docs.gephi.org/desktop/Plugins/Filter/)
  | [API: Query](https://gephi.org/javadoc/0.9.3/org/gephi/filters/api/class-use/Query.html)

---

*This document describes the target architecture for the query pipeline
refactoring. See `web/ARCHITECTURE.md` for the current system architecture,
`web/FILTERS.md` for detailed filter documentation, and
`docs/technical/SCIP_CORE_ARCHITECTURE.md` for the Rust backend.*
