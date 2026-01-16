# Filter System Documentation

This document explains how the filtering system works in the call graph viewer.

## Overview

The filter system is implemented in `src/filters.ts` with the main entry point being the `applyFilters()` function. Filters are applied in a specific order, and different filters interact with each other in defined ways.

## Filter Types

### 1. Source Query (`sourceQuery`)
**UI Element:** "Source" input field  
**Function:** Shows what functions are **called by** the matched functions (callees direction)

**Behavior:**
- Matches function names using substring matching or glob patterns
- Traverses the call graph **forward** (following call edges) up to `maxDepth`
- Shows the source node and all functions it calls (directly or indirectly)

**Matching Logic:** `matchesQuery()` in `filters.ts`
- Simple substring: `decompress` matches any function containing "decompress"
- Glob patterns: `*compress*` for contains, `decompress*` for starts with
- Path-qualified: `edwards::decompress` matches `decompress` in files named `edwards.rs`

### 2. Sink Query (`sinkQuery`)
**UI Element:** "Sink" input field  
**Function:** Shows what functions **call** the matched functions (callers direction)

**Behavior:**
- Same matching logic as Source Query
- Traverses the call graph **backward** (reverse call edges) up to `maxDepth`
- Shows the sink node and all functions that call it

### 3. Source + Sink Combined
When both Source and Sink are set:

**Same query (Source = Sink):** Shows full neighborhood (both callers and callees)

**Different queries:** Shows all **paths** from source to sink nodes
- Ignores `maxDepth` - shows complete paths regardless of length
- Uses DFS with backtracking (`findPathNodes()` function)

### 4. Include Files (`includeFiles`)
**UI Element:** "Include Files" input field + File List panel  
**Function:** Only shows functions defined in the specified files

**Behavior:**
- Comma-separated list of file patterns: `hazmat.rs, scalar.rs`
- Supports two types of patterns:
  - **Filename patterns** (no `/`): Match against `file_name` only
  - **Path patterns** (contains `/`): Match against `relative_path` for disambiguation

**Pattern Syntax:**
- `*` matches any characters except `/`
- `**` matches any path segments (including across directories)
- `?` matches single character except `/`
- Case-insensitive matching

**Examples:**
| Pattern | Matches |
|---------|---------|
| `edwards.rs` | ALL files named `edwards.rs` |
| `src/edwards.rs` | Only `*/src/edwards.rs` |
| `ifma/edwards.rs` | Only `*/ifma/edwards.rs` |
| `**/backend/**/edwards.rs` | Any `edwards.rs` under a `backend/` directory |
| `curve25519-dalek/**` | All files under `curve25519-dalek/` |

**File List Panel:**
- Shows all unique files with function counts
- **Duplicate filenames** are shown with disambiguating path suffix in parentheses
- Example: `edwards.rs (src/)`, `edwards.rs (ifma/)`, `edwards.rs (avx2/)`
- Clicking a file toggles it in the filter (using the disambiguated path if needed)
- Hover over a file to see its full relative path

**Disambiguation Dropdown:**
- When you type an ambiguous filename (e.g., `edwards.rs`) and press **Enter**
- If multiple files match, a dropdown appears showing all matching files
- Select which specific file(s) you want to include using checkboxes
- Click **All** to include all matching files, or **Apply** to include only selected ones
- The ambiguous pattern is automatically replaced with the disambiguated path(s)
- Press **Escape** or click outside to cancel

**Implementation:** 
- `nodeMatchesIncludeFilePattern()` in `filters.ts`
- `pathPatternToRegex()` for path patterns
- `globToRegex()` for filename patterns
- `computeDisambiguatedPaths()` in `main.ts` for UI disambiguation

**Click behavior:** When Include Files is active, clicking nodes does NOT apply depth filtering (same as Source/Sink behavior). The filtered graph remains stable.

### 5. Exclude Name Patterns (`excludeNamePatterns`)
**UI Element:** "Exclude Names" input field  
**Function:** Hides functions matching the patterns

**Behavior:**
- Comma-separated glob patterns: `*_comm*, lemma_*`
- Matches against `node.display_name` (function name)
- Processed via `nodeMatchesExcludeNamePattern()`

### 6. Exclude Path Patterns (`excludePathPatterns`)
**UI Element:** "Exclude Paths" input field  
**Function:** Hides functions in matching paths

**Behavior:**
- Comma-separated glob patterns: `*/specs/*, */test/*`
- Matches against `node.id` (full SCIP path)
- Processed via `nodeMatchesExcludePathPattern()`

### 7. Function Mode Filters
**UI Elements:** Checkboxes for Exec, Proof, Spec functions  
**Function:** Filter by Verus function type

- `showExecFunctions`: Show executable functions (default: true)
- `showProofFunctions`: Show proof functions/lemmas (default: true)
- `showSpecFunctions`: Show spec functions (default: false)

**Implementation:** `nodePassesModeFilter()` in `filters.ts`

### 8. Call Type Filters
**UI Elements:** Checkboxes for Body, Requires, Ensures calls  
**Function:** Filter edges by where the call occurs

- `showInnerCalls`: Body calls (default: true)
- `showPreconditionCalls`: Calls in `requires` clauses (default: false)
- `showPostconditionCalls`: Calls in `ensures` clauses (default: false)

**Important:** Requires/Ensures edges typically connect to **Spec functions** (since `requires` and `ensures` clauses call specification predicates). If "Show Spec Functions" is disabled, these edges won't be visible even when their respective toggles are enabled, because:
1. Spec nodes are excluded by the mode filter (step 1 in filter order)
2. Links to excluded nodes are removed (step 4: "Only links between included nodes")
3. The call type filter runs after node filtering, so it never sees edges to filtered-out Spec nodes

To see Requires/Ensures edges, enable **both** the call type toggle AND "Show Spec Functions".

### 9. Source Type Filters
**UI Elements:** Checkboxes for Libsignal/External  
**Function:** Filter by code source

- `showLibsignal`: Show libsignal code (default: true)
- `showNonLibsignal`: Show external dependencies (default: true)

### 10. Max Depth (`maxDepth`)
**UI Element:** Depth limit slider  
**Function:** Limits traversal depth for Source/Sink queries

- `null` or `0` means unlimited depth
- Only applies to Source/Sink query traversal
- Does NOT limit Source→Sink path finding

### 11. Click-Based Selection (`selectedNodes`)
**UI Element:** Clicking nodes in the graph  
**Function:** Explore neighborhood of clicked nodes

**Behavior:**
- When NO Source/Sink/Include Files is set: clicking a node shows its neighborhood up to `maxDepth`
- When Source/Sink/Include Files IS set: clicking only highlights the node (no filter change)
- Shift+click: Hides the node

### 12. Hidden Nodes (`hiddenNodes`)
**UI Element:** Shift+click nodes or "Hidden Nodes" panel  
**Function:** Manually hide specific nodes

## Filter Application Order

The `applyFilters()` function applies filters in this order:

1. **Pre-filtering** (creates `modeAllowedNodeIds`):
   - Function mode filter (exec/proof/spec)
   - Exclude name patterns
   - Exclude path patterns
   - Include files filter
   - Build artifact exclusion (`target/`, `build/`)
   - Hidden nodes exclusion

2. **Query-based filtering**:
   - If Source/Sink queries → traverse and collect matching nodes
   - If only Include Files → use pre-filtered nodes directly
   - If click selection (and no queries/include files) → depth from selected

3. **Additional filters**:
   - Libsignal/external filter
   - Mode filter (redundant but ensures consistency)
   - Hidden nodes filter

4. **Link filtering**:
   - Only links between included nodes
   - Path-based link filter (when depth limit active)
   - Call type filter (inner/precondition/postcondition)

5. **Isolated node removal**:
   - Removes nodes with no edges in filtered graph

## Large Graph Handling

For large graphs (>5000 links), the system defers loading until filters are applied:

**What counts as a valid filter for loading:**
- Source query (non-empty)
- Sink query (non-empty)
- Include Files (non-empty)

**Implementation:** `hasSearchFilters()` in `main.ts`

## Key Functions

| Function | File | Purpose |
|----------|------|---------|
| `applyFilters()` | filters.ts | Main filter orchestration |
| `matchesQuery()` | filters.ts | Query matching (substring, glob, path-qualified) |
| `globToRegex()` | filters.ts | Convert glob pattern to regex (for filenames) |
| `pathPatternToRegex()` | filters.ts | Convert path pattern to regex (for relative paths) |
| `nodeMatchesIncludeFilePattern()` | filters.ts | Check if node's file matches include patterns |
| `nodeMatchesExcludeNamePattern()` | filters.ts | Check if node name matches exclude patterns |
| `nodePassesModeFilter()` | filters.ts | Check function mode (exec/proof/spec) |
| `getCalleesRecursive()` | filters.ts | Traverse forward (calls) with depth tracking |
| `getCallersRecursive()` | filters.ts | Traverse backward (callers) with depth tracking |
| `findPathNodes()` | filters.ts | Find all nodes on paths from source to sink |
| `computeDepthFromSelected()` | filters.ts | BFS for click-based neighborhood |
| `computeDisambiguatedPaths()` | main.ts | Compute shortest unique path suffixes for duplicate filenames |
| `populateFileList()` | main.ts | Build file list UI with disambiguation |
| `findAmbiguousMatches()` | main.ts | Find files matching an ambiguous pattern |
| `showDisambiguationDropdown()` | main.ts | Show dropdown to select from ambiguous matches |
| `checkAndShowDisambiguation()` | main.ts | Check patterns and show dropdown if ambiguous |
| `hasSearchFilters()` | main.ts | Check if meaningful filters are set (for large graph handling) |

## URL Parameters

Filters can be set via URL parameters:

| Parameter | Filter |
|-----------|--------|
| `source` | Source query |
| `sink` | Sink query |
| `files` | Include files |
| `depth` | Max depth |
| `exec` | Show exec functions (0/1) |
| `proof` | Show proof functions (0/1) |
| `spec` | Show spec functions (0/1) |
| `inner` | Show body calls (0/1) |
| `pre` | Show requires calls (0/1) |
| `post` | Show ensures calls (0/1) |
| `libsignal` | Show libsignal code (0/1) |
| `external` | Show external code (0/1) |
| `excludeName` | Exclude name patterns |
| `excludePath` | Exclude path patterns |
| `hidden` | Hidden node names (comma-separated) |

