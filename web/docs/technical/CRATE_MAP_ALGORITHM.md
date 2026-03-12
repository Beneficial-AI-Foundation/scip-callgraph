# Crate Map (Namespace Map) Algorithm

## Input

A directed graph $G = (V, E)$ where:

- $V$ is a set of function nodes, each with attributes:
  - $\text{name}(v) \in \Sigma^*$ — display name
  - $\text{crate}(v) \in \Sigma^*$ — crate or namespace name
  - $\text{file}(v) \in \Sigma^*$ — file path
  - $\text{isExternal}(v) \in \{\text{true}, \text{false}\}$

- $E \subseteq V \times V \times T$ where $T = \{\text{inner}, \text{pre}, \text{post}\}$ — typed directed edges

## Output

A multi-mode interactive visualization with three rendering modes:

1. **Collapsed mode** — an aggregated crate-level overview
2. **Expanded mode** — drill-down into a single cross-crate edge
3. **Boundary mode** — drill-down into all calls between two selected crates

## Core Algorithm

### Phase 1. Crate Aggregation

This is the key transformation that distinguishes the Crate Map from the File Map. It collapses the function-level graph into a crate-level graph.

**1.1. Partition by crate.**

Define the partition $\mathcal{C}$ of $V$ induced by the crate attribute:

$$
\mathcal{C} = \{ V_c \mid c \in \text{Im}(\text{crate}) \}, \quad V_c = \{ v \in V \mid \text{crate}(v) = c \}
$$

**1.2. Compute crate nodes.**

For each crate $c$, compute:

$$
\text{CrateNode}(c) = \left( c, \; |V_c|, \; |\{ \text{file}(v) \mid v \in V_c \}|, \; \text{isExternal}(c) \right)
$$

yielding a tuple of (name, function count, file count, external flag).

**1.3. Aggregate edges.**

For each pair of distinct crates $(c_i, c_j)$, count the number of function-level edges crossing the boundary:

$$
\text{weight}(c_i, c_j) = \left| \{ (u, v, t) \in E \mid \text{crate}(u) = c_i \land \text{crate}(v) = c_j \} \right|
$$

Intra-crate edges are discarded:

$$
E_\mathcal{C} = \{ (c_i, c_j, w) \mid c_i \neq c_j \land w = \text{weight}(c_i, c_j) > 0 \}
$$

Each aggregated edge also retains the underlying function-level calls for drill-down:

$$
\text{calls}(c_i, c_j) = \{ (u, v, t) \in E \mid \text{crate}(u) = c_i \land \text{crate}(v) = c_j \}
$$

**Result.** The crate graph $G_\mathcal{C} = (\mathcal{C}, E_\mathcal{C})$ is a weighted directed graph where each node is a crate and each edge carries a call count plus the underlying function-level calls.

### Phase 2. Color Assignment

Sort crate nodes by function count (descending) and assign palette indices:

$$
\gamma: \mathcal{C} \to \{0, 1, \ldots, 7\}, \quad \gamma(c_i) = \text{rank}(c_i) \bmod 8
$$

where $\text{rank}$ is the position after sorting by $|V_c|$ descending. This ensures the largest crates get consistent, distinguishable colors.

---

## Mode A: Collapsed Rendering

This is the default overview mode — one box per crate, weighted edges between them.

### A.1. Node sizing

$$
w(c) = \max\!\big(\text{nw}(\text{name}(c)),\; \text{nw}(\text{stats}(c))\big) + 30, \quad h(c) = 56
$$

where $\text{nw}(s) = \max(100, |s| \cdot 6.5 + 20)$ and $\text{stats}(c) = $ "$|V_c|$ fn, $|\text{files}(c)|$ files".

### A.2. Layout

Solve the DAG layout problem on $G_\mathcal{C}$:

$$
\text{pos}: \mathcal{C} \to \mathbb{R}^2
$$

with constraints:
- Direction: left-to-right ($\text{rankdir} = \text{LR}$)
- Node spacing: $60\text{px}$ vertical, $120\text{px}$ horizontal
- Margins: $50\text{px}$

(Solved by dagre/Sugiyama heuristic.)

### A.3. Edge rendering

Each crate-level edge $(c_i, c_j, w) \in E_\mathcal{C}$ is rendered as a cubic Bézier curve (same formula as File Map). The stroke width encodes the relative call count:

$$
\text{strokeWidth}(c_i, c_j) = 1.5 + \frac{w}{\max_{e \in E_\mathcal{C}} \text{weight}(e)} \cdot 4
$$

A text label at the midpoint of the curve shows "$w$ calls".

### A.4. Node rendering

Each crate node is a rounded rectangle at $\text{pos}(c)$ containing:
- Line 1: crate name (bold, 12px)
- Line 2: "$|V_c|$ fn, $|\text{files}(c)|$ files" (light, 10px)

Fill and stroke colors are determined by $\gamma(c)$.

### A.5. Interactions

**Hover on crate $c$:**

Compute the 1-neighborhood in the crate graph:

$$
N_\mathcal{C}(c) = \{c\} \cup \{ c' \mid (c, c', \_) \in E_\mathcal{C} \lor (c', c, \_) \in E_\mathcal{C} \}
$$

$$
\text{opacity}(c') = \begin{cases} 1.0 & \text{if } c' \in N_\mathcal{C}(c) \\ 0.25 & \text{otherwise} \end{cases}
$$

**Click on crate $c$:**

Two-click boundary selection protocol:
1. First click sets $\text{source} \leftarrow c$
2. Second click (on different crate) sets $\text{target} \leftarrow c'$, triggers **Mode C** (boundary rendering)
3. Click on already-selected crate deselects it

Selected crates get a thicker border (3.5px) and a role label ("SOURCE" in blue, "TARGET" in orange).

**Click on edge $(c_i, c_j)$:**

Triggers **Mode B** (expanded edge rendering) — drill down into the function-level calls across that edge.

**Double-click on crate $c$:**

Navigates to the Call Graph view, filtered to show only files belonging to crate $c$:

$$
\text{filter} \leftarrow \{ \text{file}(v) \mid v \in V_c \}
$$

---

## Mode B: Expanded Edge Rendering

When the user clicks on a crate-level edge $(c_s, c_t)$, the view drills down to show the individual function-level calls that make up that edge.

### B.1. Extract function subgraph

$$
V_{\text{exp}} = \{ u \mid \exists (u, v, \_) \in \text{calls}(c_s, c_t) \} \cup \{ v \mid \exists (u, v, \_) \in \text{calls}(c_s, c_t) \}
$$

$$
E_{\text{exp}} = \text{calls}(c_s, c_t)
$$

These are only the functions directly involved in cross-crate calls.

### B.2. Build compound layout

Construct a compound graph with:
- Two cluster nodes: $c_s$ and $c_t$ (expanded, showing individual functions)
- All other crates rendered as collapsed summary boxes
- Function nodes $V_{\text{exp}}$ placed inside their respective crate clusters
- Invisible proxy nodes inside each cluster for routing external edges (workaround for a dagre bug with edges connecting to compound parent nodes)

$$
\text{pos}: V_{\text{exp}} \cup (\mathcal{C} \setminus \{c_s, c_t\}) \to \mathbb{R}^2
$$

$$
\text{bbox}: \{c_s, c_t\} \to \mathbb{R}^2 \times \mathbb{R}^2
$$

### B.3. Render

- Two expanded crate group backgrounds (pastel rectangles with bold stroke)
- Individual function nodes inside each group (rounded rectangles, white fill)
- Function-level edges as Bézier curves between individual nodes
- Remaining crates rendered as collapsed summary boxes at reduced opacity (0.6)
- Cross-crate edges involving non-expanded crates routed through proxy nodes

### B.4. Collapse

Pressing Escape or clicking the background returns to **Mode A**.

---

## Mode C: Boundary Rendering

When two crates are selected (source $c_s$ and target $c_t$), the view shows **all** function-level calls between them in both directions.

### C.1. Collect bidirectional calls

$$
E_{\text{into}} = \text{calls}(c_t, c_s) \quad \text{(calls from target into source)}
$$

$$
E_{\text{from}} = \text{calls}(c_s, c_t) \quad \text{(calls from source into target)}
$$

$$
E_{\text{boundary}} = E_{\text{into}} \cup E_{\text{from}}
$$

### C.2. Extract function subgraph

$$
V_{\text{boundary}} = \{ u \mid \exists (u, v, \_) \in E_{\text{boundary}} \} \cup \{ v \mid \exists (u, v, \_) \in E_{\text{boundary}} \}
$$

### C.3. Build compound layout

Construct a compound graph with exactly two clusters ($c_s$ and $c_t$), each containing the relevant function nodes. No other crates are shown — this is a focused view.

$$
\text{pos}: V_{\text{boundary}} \to \mathbb{R}^2, \quad \text{bbox}: \{c_s, c_t\} \to \mathbb{R}^2 \times \mathbb{R}^2
$$

### C.4. Render

- Source crate group: blue border, labeled "$c_s$ (source — called)"
- Target crate group: orange border, labeled "$c_t$ (target — caller)"
- Function-level edges between the two groups
- Summary text: "$|E_{\text{into}}|$ calls from $c_t \to c_s$, $|E_{\text{from}}|$ calls from $c_s \to c_t$"
- "View in Call Graph" navigation button — switches to Call Graph view with source/sink filters set to the two crates

### C.5. Collapse

Pressing Escape or clicking the background returns to **Mode A**.

---

## Summary: Pipeline Composition

$$
G \xrightarrow{\text{Aggregate}} G_\mathcal{C} \xrightarrow{\text{Layout}} \text{pos} \xrightarrow{\text{Render}} \mathcal{R}
$$

$$
\text{CrateMap} = \text{Render} \circ \text{Layout} \circ \text{Aggregate}
$$

| Stage | Function | Description | Complexity |
|-------|----------|-------------|------------|
| Aggregate | $(V, E) \to (\mathcal{C}, E_\mathcal{C})$ | Partition by crate, collapse edges, sum weights | $O(\|V\| + \|E\|)$ |
| Layout | $(\mathcal{C}, E_\mathcal{C}) \to \text{pos}$ | DAG placement (dagre/Sugiyama) | NP-hard, heuristic |
| Render | $\text{pos} \to \mathcal{R}$ | Visual encoding + 3-mode interaction model | Mapping functions + Bézier curves |

## Comparison with File Map

| Aspect | File Map | Crate Map |
|--------|-----------|-----------|
| **Partition key** | $\text{file}(v)$ | $\text{crate}(v)$ |
| **Unit of display** | Individual functions | Aggregated crates |
| **Edge granularity** | One curve per function-to-function edge | One weighted curve per crate-to-crate edge |
| **Aggregation** | None | Edge count aggregation: $\|E\| \to \|E_\mathcal{C}\|$ |
| **Drill-down** | None (single-level) | 3 modes: collapsed, expanded edge, boundary |
| **Visual encoding** | Shape = kind, border = status, fill = depth | Uniform boxes with summary stats |
| **Layout input** | $\|V\|$ nodes | $\|\mathcal{C}\|$ nodes (typically $\ll \|V\|$) |
| **Cross-view navigation** | None | Double-click → Call Graph; boundary → Call Graph |

A config can parameterize the partition key ($\text{file}$ vs. $\text{crate}$). It cannot express the difference between "render every function as a separate shape" (File Map) and "aggregate functions into weighted summary boxes with 3-mode drill-down" (Crate Map). These are different algorithms, not different parameters.

## Relationship to Known Algorithms and Techniques

### Phase 1 — Crate Aggregation: Quotient Graph

**References.**
- Harary, *Graph Theory*, Addison-Wesley, 1969, §14 "Quotient Graphs." — Defines the quotient of a graph with respect to a partition.
- Abello, van Ham & Krishnan, "ASK-GraphView: A Large Scale Graph Visualization System," *IEEE Trans. Visualization and Computer Graphics* 12(5), 2006. — Multi-level graph aggregation for interactive exploration.

**Relationship: instantiation with weighted extension.** Given the partition $\mathcal{C}$ of $V$ induced by $\text{crate}(v)$, the crate graph $G_\mathcal{C}$ is the **quotient graph** $G / \mathcal{C}$:

$$
G / \mathcal{C} = (\mathcal{C}, \; \{(c_i, c_j) \mid \exists (u,v) \in E, \; u \in V_{c_i}, \; v \in V_{c_j}, \; c_i \neq c_j \})
$$

The standard quotient graph has unweighted edges. Our construction extends it with:
- **Edge weights**: $\text{weight}(c_i, c_j) = |\{(u,v) \in E \mid u \in V_{c_i}, v \in V_{c_j}\}|$
- **Retained micro-edges**: each aggregated edge stores the underlying function-level calls $\text{calls}(c_i, c_j)$ for drill-down
- **Node metadata**: function count, file count, external flag

This is also an instance of **graph coarsening** as used in multi-level graph visualization (Abello et al.), where a large graph is progressively collapsed into smaller summaries for overview, with the ability to expand back to detail on demand.

### Phase 1.3 — Edge Aggregation

**Reference.** Holten, "Hierarchical Edge Bundling: Visualization of Adjacency Relations in Hierarchical Data," *IEEE Trans. Visualization and Computer Graphics* 12(5), 2006.

**Relationship: distinct but related.** Holten's hierarchical edge bundling routes individual edges along a hierarchy to reduce visual clutter, but preserves every edge as a separate curve. Our approach is more aggressive: we **collapse** all function-level edges between two crates into a single weighted edge. This is closer to **edge aggregation** than edge bundling — we lose individual edge identity in the overview (Mode A) but recover it on drill-down (Modes B and C).

### Mode A — Collapsed Rendering: Overview + Weighted Edges

**Reference.** Shneiderman, "The Eyes Have It: A Task by Data Type Taxonomy for Information Visualizations," *IEEE Symposium on Visual Languages*, 1996.

**Relationship: instantiation of the overview phase.** Mode A follows Shneiderman's mantra ("overview first, zoom and filter, then details-on-demand") by showing the crate-level summary as the entry point. The stroke-width encoding of call count follows the principle of using a **size channel for quantitative data** (Munzner, 2014, Ch. 5):

$$
\text{strokeWidth} = 1.5 + \frac{w}{w_{\max}} \cdot 4
$$

This is a linear interpolation in the range $[1.5, 5.5]$, mapping call count to a perceptually ordered visual variable.

### Modes B & C — Expanded/Boundary: Semantic Zoom

**References.**
- Bederson & Hollan, "Pad++: A Zooming Graphical Interface for Exploring Alternate Interface Physics," *UIST '94*, 1994. — Introduces semantic zooming: different representations at different zoom levels.
- Cockburn, Karlson & Bederson, "A Review of Overview+Detail, Zooming, and Focus+Context Interfaces," *ACM Computing Surveys* 41(1), 2009. — Survey of multi-scale visualization techniques.

**Relationship: instantiation of semantic zoom / details-on-demand.** The three modes implement a **semantic zoom** pattern where the same data is shown at different levels of abstraction:

| Mode | Abstraction level | Unit of display | Trigger |
|------|------------------|-----------------|---------|
| A (Collapsed) | Crate-level | Aggregated boxes | Default |
| B (Expanded edge) | Mixed: one edge expanded to functions | Functions + collapsed crates | Click edge |
| C (Boundary) | Function-level between two crates | Individual functions | Select two crates |

This is not geometric zoom (scaling the viewport), but **semantic zoom**: the visual representation changes structurally when the user requests more detail. The transition from Mode A to Mode B is analogous to clicking a node in a treemap to expand it — the surrounding context remains at the summary level while the focus area shows full detail.

### Modes B & C — Compound Graph with Proxy Nodes

**Reference.** Sander, "Layout of Compound Directed Graphs," Technical Report, Universität des Saarlandes, 1996.

**Relationship: extension with engineering workaround.** The expanded and boundary modes use dagre's compound graph mode (Sander's algorithm). However, dagre has a known bug where edges connecting directly to compound parent nodes crash the layout engine (dagre issue #236). Our implementation works around this by introducing **invisible proxy nodes** inside each cluster:

$$
V_{\text{proxy}} = \{ p_c \mid c \in \{c_s, c_t\} \}, \quad w(p_c) = h(p_c) = 0
$$

External edges are routed through these proxy nodes instead of connecting to the cluster node directly. This is a practical engineering adaptation not described in the original compound layout literature.

### Interaction: Two-Click Boundary Selection Protocol

**Reference.** Wattenberg, "Visual Exploration of Multivariate Graphs," *CHI '06*, 2006. — Interactive exploration of relationships between graph subsets.

**Relationship: domain-specific extension.** The two-click protocol (click source, click target, view boundary) is an interaction pattern for exploring **inter-module interfaces**. It is domain-specific to software architecture visualization: the "boundary" between two modules (the set of cross-module calls) is a well-defined concept in software engineering (Parnas, "On the Criteria to Be Used in Decomposing Systems into Modules," 1972). The visualization makes this concept interactive and visual.

### Cross-View Navigation

**Reference.** Roberts, "State of the Art: Coordinated & Multiple Views in Exploratory Visualization," *CMV '07*, 2007.

**Relationship: instantiation of coordinated multiple views.** The double-click and "View in Call Graph" button implement **coordinated navigation** between views: the Crate Map (overview) and Call Graph (detail) are linked views where a selection in one drives filtering in the other. This follows the **brushing and linking** paradigm from coordinated multiple views (CMV), where interaction in one view propagates to others.

### Overall Architecture — Multi-Level Graph Visualization

**Reference.** Elmqvist & Fekete, "Hierarchical Aggregation for Information Visualization: Overview, Techniques, and Design Guidelines," *IEEE Trans. Visualization and Computer Graphics* 16(3), 2010.

**Relationship: instantiation.** The full Crate Map architecture — quotient graph aggregation, overview rendering, semantic zoom into detail, and cross-view navigation — is an instance of the **hierarchical aggregation** framework described by Elmqvist & Fekete. Their framework identifies three components:

1. **Aggregation operator** — our quotient graph construction (Phase 1)
2. **Visual representation at each level** — our Mode A (aggregated) and Modes B/C (detailed)
3. **Navigation between levels** — our click-to-expand and escape-to-collapse interactions

Our specific contribution is applying this framework to **formal verification tracking**: the aggregated view shows crate-level structure, while the detailed views expose the individual function-level dependencies that cross crate boundaries — directly relevant to understanding verification scope and inter-module trust boundaries.
