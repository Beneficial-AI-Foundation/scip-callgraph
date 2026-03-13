# Call Graph Algorithm

## Input

A directed graph $G = (V, E)$ where:

- $V$ is a set of function nodes, each with attributes:
  - $\text{name}(v) \in \Sigma^*$ — display name
  - $\text{status}(v) \in \{\text{verified}, \text{failed}, \text{unverified}, \text{unknown}\}$ — verification status
  - $\text{deps}(v) \subseteq V$ — functions this node calls (outgoing)
  - $\text{dependents}(v) \subseteq V$ — functions that call this node (incoming)

- $E \subseteq V \times V \times T$ where $T = \{\text{inner}, \text{pre}, \text{post}\}$ — typed directed edges

- Optionally, precomputed depth assignments $d_0: V \to \mathbb{N}$ from source/sink filtering.

## Output

A dynamic, force-stabilized visual embedding $\mathcal{R}$ where node positions evolve over time:

- A time-varying position function $\text{pos}: V \times \mathbb{R}^+ \to \mathbb{R}^2$ converging to equilibrium
- Rendered circles $S = \{s_v \mid v \in V\}$ with radius proportional to degree
- Rendered curves $C = \{c_e \mid e \in E\}$
- Interaction handlers $\mathcal{I}$ (click, hover, drag)

## Algorithm

### Phase 1. Layer Assignment (Topological Depth)

Assign each node a depth (horizontal rank) that respects the call direction.

**1.1. Find root nodes.**

$$
R = \{ v \in V \mid \text{in-degree}(v) = 0 \}
$$

If $R = \emptyset$ (graph has cycles), fall back to the node with minimum in-degree:

$$
R = \{ \arg\min_{v \in V} \text{in-degree}(v) \}
$$

**1.2. BFS depth assignment.**

Initialize $d(v) = 0$ for all $v \in R$. Then BFS, propagating depth along outgoing edges:

$$
d(v) = \max_{u : (u, v, \_) \in E} \big( d(u) + 1 \big)
$$

The $\max$ ensures that if a node is reachable via multiple paths, it gets the longest-path depth, keeping it to the right of all its callers.

For disconnected components, any unvisited node gets $d(v) = 0$.

If precomputed depths $d_0$ are provided (from source/sink path filtering), those are used instead.

**1.3. Group by layer.**

$$
L_k = \{ v \in V \mid d(v) = k \}, \quad k = 0, 1, \ldots, d_{\max}
$$

where $d_{\max} = \max_{v \in V} d(v)$.

### Phase 2. Crossing Minimization (Barycenter Heuristic)

Within each layer $L_k$, reorder nodes to minimize edge crossings with adjacent layers.

**2.1. Initial ordering.**

Sort each layer alphabetically by display name (deterministic seed):

$$
L_k = \text{sort}_{\text{name}}(L_k)
$$

**2.2. Barycenter sweeps.**

Repeat for $I = 6$ iterations:

**Forward sweep** (layer 1 to $d_{\max}$): for each node $v$ in layer $L_k$, compute its barycenter — the average position of its neighbors in the previous layer $L_{k-1}$:

$$
\text{bary}(v) = \frac{1}{|N^-(v)|} \sum_{u \in N^-(v)} \text{pos}_{k-1}(u)
$$

where $N^-(v) = \{ u \mid (u, v, \_) \in E, \; u \in L_{k-1} \}$ is the set of predecessors, and $\text{pos}_{k-1}(u)$ is the index of $u$ within layer $L_{k-1}$. Sort $L_k$ by barycenter.

If $N^-(v) = \emptyset$, retain the current position.

**Backward sweep** (layer $d_{\max} - 1$ down to 0): symmetric, using successors in the next layer:

$$
\text{bary}(v) = \frac{1}{|N^+(v)|} \sum_{w \in N^+(v)} \text{pos}_{k+1}(w)
$$

where $N^+(v) = \{ w \mid (v, w, \_) \in E, \; w \in L_{k+1} \}$.

After 6 forward+backward sweeps, the ordering within each layer is locally optimal for crossing minimization.

### Phase 3. Initial Coordinate Assignment

Convert layer assignments and within-layer orderings into $(x, y)$ coordinates.

**3.1. Compute effective canvas size.**

The canvas adapts to graph size:

$$
W_{\text{eff}} = \max(W_{\text{container}}, \; d_{\max} \cdot 200 + 200)
$$

$$
H_{\text{eff}} = \max(H_{\text{container}}, \; \max_k |L_k| \cdot 60 + 200)
$$

**3.2. Assign positions.**

With padding $p = 100$:

$$
x_v = p + d(v) \cdot \frac{W_{\text{eff}} - 2p}{d_{\max}}
$$

$$
y_v = p + (\text{rank}_k(v) + 1) \cdot \frac{H_{\text{eff}} - 2p}{|L_k| + 1}
$$

where $\text{rank}_k(v)$ is the zero-based position of $v$ within its layer $L_k$ (after crossing minimization).

### Phase 4. Force-Directed Simulation

The initial layered positions from Phase 3 are refined by a D3 force simulation. This is a numerical integration of a system of forces that iteratively adjusts positions toward equilibrium.

**4.1. Force system.**

The simulation evolves node positions under a superposition of five forces:

$$
F_{\text{total}}(v) = F_{\text{link}}(v) + F_{\text{charge}}(v) + F_{\text{x}}(v) + F_{\text{y}}(v) + F_{\text{collision}}(v)
$$

**Link force** — spring-like attraction along edges:

$$
F_{\text{link}}(v) = \sum_{(v, w, \_) \in E} k_{\text{link}} \cdot \left( \|\text{pos}(w) - \text{pos}(v)\| - d_{\text{link}} \right) \cdot \hat{r}_{vw}
$$

where $k_{\text{link}} = 0.5$ (spring constant) and $d_{\text{link}}$ is the rest length, adaptive to graph density:

$$
d_{\text{link}} = \text{clamp}\left(60 + \max_k |L_k| \cdot 8, \; 60, \; 200\right)
$$

**Charge force** — electrostatic repulsion between all node pairs (Barnes-Hut approximation):

$$
F_{\text{charge}}(v) = \sum_{u \neq v} \frac{q}{\|\text{pos}(u) - \text{pos}(v)\|^2} \cdot \hat{r}_{vu}
$$

where the charge strength is adaptive:

$$
q = \text{clamp}(-150 - |V| \cdot 3, \; -600, \; -150)
$$

**X-position force** — strong pull toward the target column (preserves layered structure):

$$
F_{\text{x}}(v) = 0.8 \cdot (x_{\text{target}}(v) - x_v)
$$

where $x_{\text{target}}(v)$ is the horizontal position from Phase 3. The high strength (0.8) ensures nodes stay near their assigned layer.

**Y-position force** — weak pull toward the target row (allows vertical rearrangement):

$$
F_{\text{y}}(v) = k_y \cdot (y_{\text{target}}(v) - y_v)
$$

where $k_y$ is adaptive:

$$
k_y = 0.03 + 0.02 \cdot \frac{\max_k |L_k|}{|V|}
$$

The weakness of this force (0.03–0.05) lets the other forces shift nodes vertically for better aesthetics.

**Collision force** — hard repulsion to prevent overlap:

$$
F_{\text{collision}}(v) = \begin{cases}
\text{repel} & \text{if } \|\text{pos}(u) - \text{pos}(v)\| < r_c \\
0 & \text{otherwise}
\end{cases}
$$

where the collision radius is adaptive:

$$
r_c = 25 + \min(|V| \cdot 0.3, \; 20)
$$

**4.2. Integration.**

D3's simulation uses velocity Verlet integration with cooling ($\alpha$ decay):

$$
v_i(t + \Delta t) = v_i(t) + F_{\text{total}}(v_i) \cdot \Delta t
$$

$$
\text{pos}(v_i, t + \Delta t) = \text{pos}(v_i, t) + v_i(t + \Delta t) \cdot \Delta t
$$

The simulation starts with $\alpha = 1$ (hot) and decays toward 0 (frozen). When $\alpha < \alpha_{\min}$, the simulation stops and positions stabilize.

### Phase 5. Visual Encoding

**5.1. Node rendering.**

Nodes are circles with radius proportional to degree:

$$
r(v) = \text{clamp}\left(\sqrt{|\text{deps}(v)| + |\text{dependents}(v)|} \cdot 2, \; 5, \; 15\right)
$$

**Node color** $\kappa: \text{VerificationStatus} \to \text{Color}$:

$$
\kappa(s) = \begin{cases}
\text{green} & \text{if } s = \text{verified} \\
\text{red} & \text{if } s = \text{failed} \\
\text{gray} & \text{if } s = \text{unverified} \\
\text{blue} & \text{if } s = \text{unknown}
\end{cases}
$$

Text labels are placed above each node ($dy = -20$).

**5.2. Edge rendering.**

Each edge $(u, v, t) \in E$ is rendered as a cubic Bézier curve (same formula as File Map/Crate Map, but connecting node centers rather than node edges):

$$
\mathbf{B}(\tau) = (1-\tau)^3 P_0 + 3(1-\tau)^2 \tau \, P_1 + 3(1-\tau) \tau^2 \, P_2 + \tau^3 P_3
$$

where:

$$
P_0 = (x_u, y_u), \quad P_1 = (x_u + 0.4\delta, \; y_u)
$$

$$
P_2 = (x_v - 0.4\delta, \; y_v), \quad P_3 = (x_v, y_v)
$$

with $\delta = x_v - x_u$. Control points use 40% horizontal offsets for smooth S-curves.

Edge style by type:

$$
\text{style}(t) = \begin{cases}
\text{solid gray} & \text{if } t = \text{inner} \\
\text{dashed orange} & \text{if } t = \text{pre} \\
\text{dashed pink} & \text{if } t = \text{post}
\end{cases}
$$

Edge positions update continuously on each simulation tick (dynamic, not static).

### Phase 6. Interaction Model

**6.1. State.**

$$
\mathcal{S} = (\text{selected} \subseteq V, \; \text{hovered} \in V \cup \{\bot\}, \; \text{dragged} \in V \cup \{\bot\})
$$

**6.2. Click on node $v$:**

$$
\text{selected}' = \begin{cases}
\text{selected} \setminus \{v\} & \text{if } v \in \text{selected} \\
\text{selected} \cup \{v\} & \text{otherwise}
\end{cases}
$$

Selected node gets a red highlight stroke (4px).

**6.3. Shift+click on node $v$:**

$$
V' = V \setminus \{v\}, \quad E' = E \setminus \{(u,w,t) \mid u = v \lor w = v\}
$$

Remove node and re-render.

**6.4. Hover on node $v$:**

$$
N(v) = \{v\} \cup \{u \mid (u,v,\_) \in E \lor (v,u,\_) \in E\}
$$

$$
\text{opacity}(u) = \begin{cases}
1.0 & \text{if } u \in N(v) \\
0.2 & \text{otherwise}
\end{cases}
$$

**6.5. Drag node $v$:**

During drag, pin the node to the cursor position:

$$
v.fx = x_{\text{cursor}}, \quad v.fy = y_{\text{cursor}}
$$

This constrains $v$'s position while the simulation reheats ($\alpha \leftarrow 0.3$), letting other nodes readjust around the dragged node. On drag end, release the pin:

$$
v.fx = \bot, \quad v.fy = \bot, \quad \alpha_{\text{target}} \leftarrow 0
$$

## Summary: Pipeline Composition

$$
G \xrightarrow{\text{Depth}} (V, L_0, \ldots, L_{d_{\max}}) \xrightarrow{\text{Bary}} \text{ordering} \xrightarrow{\text{Init}} \text{pos}_0 \xrightarrow{\text{Force}} \text{pos}_\infty \xrightarrow{\text{Render}} \mathcal{R}
$$

$$
\text{CallGraph} = \text{Render} \circ \text{ForceSimulation} \circ \text{InitPosition} \circ \text{CrossingMin} \circ \text{LayerAssign}
$$

| Stage | Function | Description | Complexity |
|-------|----------|-------------|------------|
| LayerAssign | $G \to (L_0, \ldots, L_{d_{\max}})$ | BFS longest-path depth | $O(\|V\| + \|E\|)$ |
| CrossingMin | $L_k \to L_k'$ | Barycenter heuristic, 6 iterations | $O(I \cdot (\|V\| + \|E\|))$, $I = 6$ |
| InitPosition | $(L_k') \to \text{pos}_0$ | Layered coordinate assignment | $O(\|V\|)$ |
| ForceSimulation | $\text{pos}_0 \to \text{pos}_\infty$ | Velocity Verlet with 5-force system | $O(T \cdot \|V\| \log \|V\|)$ per tick (Barnes-Hut) |
| Render | $\text{pos}_t \to \mathcal{R}$ | SVG circles + Bézier curves (updated per tick) | $O(\|V\| + \|E\|)$ per tick |

## Comparison with File Map and Crate Map

| Aspect | Call Graph | File Map | Crate Map |
|--------|-----------|---------------------|-----------|
| **Layout engine** | Custom: layered init + D3 force simulation | dagre (Sugiyama, static) | dagre (Sugiyama, static) |
| **Position stability** | Dynamic (positions evolve over time) | Static (computed once) | Static (computed once) |
| **Node shape** | Circles (radius = degree) | Shapes by kind (rect/ellipse/diamond) | Uniform rectangles |
| **Grouping** | None | By file | By crate |
| **Edge treatment** | Individual, dynamic position | Individual, static position | Aggregated with weights |
| **Draggable** | Yes (force simulation re-equilibrates) | No | No |
| **Color encodes** | Verification status (1 channel) | Border = readiness, fill = depth (2 channels) | Palette by crate identity |

The key architectural difference: the Call Graph uses a **hybrid layout** — Sugiyama-style layer assignment and crossing minimization for initialization, followed by force-directed simulation for refinement. This gives it the layered structure of a DAG drawing but with the dynamic, interactive feel of a force layout (nodes can be dragged, the graph "breathes" as it stabilizes). File Map and Crate Map use purely static dagre layouts.

## Relationship to Known Algorithms and Techniques

### Phase 1 — Layer Assignment: Longest-Path Layering

**References.**
- Sugiyama, Tagawa & Toda, "Methods for Visual Understanding of Hierarchical System Structures," *IEEE Trans. Systems, Man, and Cybernetics* 11(2), 1981. — Phase 2 of the Sugiyama framework: layer assignment.
- di Battista, Eades, Tamassia & Tollis, *Graph Drawing: Algorithms for the Visualization of Graphs*, Prentice Hall, 1999, §9.2. — Describes longest-path, shortest-path, and minimum-width layering strategies.

**Relationship: instantiation (longest-path variant).** The Sugiyama framework offers several layer assignment strategies. Our implementation uses the **longest-path** heuristic: BFS from roots, taking $d(v) = \max(d(u) + 1)$ over predecessors. This is the simplest Sugiyama layering, which tends to produce wide, shallow layouts. The alternative (network simplex, used by dagre and Graphviz's `dot`) minimizes total edge span but is more complex. The longest-path approach is chosen here because the force simulation in Phase 4 will refine the layout anyway — the initial layering only needs to be approximately correct.

The fallback for cyclic graphs (use minimum in-degree node as root) is a practical heuristic not present in the standard Sugiyama framework, which assumes a DAG (after cycle-breaking).

### Phase 2 — Crossing Minimization: Barycenter Heuristic

**References.**
- Sugiyama, Tagawa & Toda (1981), Phase 3. — Introduces the barycenter heuristic for crossing minimization.
- Eades & Wormald, "Edge Crossings in Drawings of Bipartite Graphs," *Algorithmica* 11(4), 1994. — Proves that 2-layer crossing minimization is NP-hard; barycenter is an effective heuristic.
- Jünger & Mutzel, "2-Layer Straightline Crossing Minimization: Performance of Exact and Heuristic Algorithms," *JGAA* 1(1), 1997. — Experimental comparison showing barycenter performs well in practice.

**Relationship: direct instantiation.** Our implementation is a textbook barycenter heuristic with alternating forward/backward sweeps, directly from Sugiyama's Phase 3. The 6-iteration count is a common practical choice (the original paper suggests iterating until convergence; in practice 4–8 sweeps suffice). The alphabetical initial ordering provides a deterministic seed, which is a minor practical variation — the original framework uses arbitrary initial ordering.

### Phase 3 — Coordinate Assignment: Uniform Spacing

**References.**
- Brandes & Köpf, "Fast and Simple Horizontal Coordinate Assignment," *GD 2001*, LNCS 2265. — The state-of-the-art coordinate assignment used in dagre and Graphviz.

**Relationship: simplification.** Our coordinate assignment is simpler than Brandes-Köpf: uniform horizontal spacing per layer, uniform vertical spacing within each layer. This produces a regular grid-like initial layout. The force simulation in Phase 4 is responsible for fine-tuning — so the initial assignment doesn't need the sophistication of Brandes-Köpf (which optimizes for edge straightness and node alignment). This is a deliberate design choice: the force simulation will override the exact coordinates anyway.

### Phase 4 — Force-Directed Simulation: Constrained Force Layout

**References.**
- Fruchterman & Reingold, "Graph Drawing by Force-Directed Placement," *Software: Practice and Experience* 21(11), 1991. — The classical spring-electric model: attractive springs on edges, repulsive charges on all pairs.
- Eades, "A Heuristic for Graph Drawing," *Congressus Numerantium* 42, 1984. — The original spring-embedder idea.
- Barnes & Hut, "A Hierarchical $O(N \log N)$ Force-Calculation Algorithm," *Nature* 324, 1986. — The quadtree approximation used by D3 for the charge force.
- Bostock, Ogievetsky & Heer, "D3: Data-Driven Documents," *IEEE Trans. Visualization and Computer Graphics* 17(12), 2011. — The D3.js framework; `d3-force` implements the simulation engine.

**Relationship: extension of the spring-electric model with layered constraints.** The classical Fruchterman-Reingold model has two forces: attractive springs on edges and repulsive charges on all pairs. Our system extends this with:

1. **X-position force** ($k = 0.8$, strong) — constrains nodes to their Sugiyama-assigned layer column. This is not present in standard force-directed layouts and is what makes this a **hybrid** Sugiyama + force approach. Without it, the force simulation would lose the layered DAG structure.

2. **Y-position force** ($k \approx 0.03$, weak) — provides a gentle pull toward the initial vertical ordering from the barycenter phase, but weak enough that the other forces can override it.

3. **Collision force** — prevents node overlap; not present in the original Fruchterman-Reingold but standard in modern force layouts.

4. **Adaptive parameters** — charge, link distance, collision radius, and y-strength all adapt to graph size. This is a practical engineering extension: a fixed parameterization works poorly across graphs ranging from 10 to 1000+ nodes.

The Barnes-Hut algorithm ($O(|V| \log |V|)$ per tick) is used by D3's charge force for scalability. This is an instantiation of the N-body approximation from computational physics.

The velocity Verlet integrator with $\alpha$-cooling is D3's standard simulation loop. $\alpha$ controls the "temperature" of the system: high $\alpha$ means large position adjustments (exploration), decaying $\alpha$ means settling into equilibrium (exploitation). This is analogous to **simulated annealing**.

### Phase 5 — Visual Encoding: Size by Degree

**References.**
- Bertin, *Sémiologie graphique*, 1967. — Size is an ordered visual variable, effective for quantitative data.
- Munzner, *Visualization Analysis and Design*, CRC Press, 2014, Ch. 5. — Area/size channel for quantitative attributes; recommends sqrt-scaling for area perception.

**Relationship: instantiation with sqrt-scaling.** The node radius $r(v) = \sqrt{\deg(v)} \cdot 2$ follows the standard practice of mapping quantitative data to the **square root** of the visual size, because human perception of circle area is proportional to $r^2$. This ensures that a node with 4x the degree appears 2x the radius (4x the area), maintaining perceptual proportionality. The clamp to $[5, 15]$ prevents extremes (invisible dots or oversized blobs).

### Phase 6 — Drag Interaction: Constrained Node Pinning

**References.**
- Dwyer, Marriott & Stuckey, "Fast Node Overlap Removal," *GD 2005*, LNCS 3843. — Constraint-based approaches to interactive graph layout.
- D3.js drag module documentation. — The fx/fy pinning mechanism.

**Relationship: instantiation of D3's pinning model.** During drag, setting $v.fx, v.fy$ converts the node into a fixed constraint in the force simulation. The simulation reheats ($\alpha_{\text{target}} = 0.3$) so other nodes can readjust around the pinned node. On release, the pin is removed and the system cools back to equilibrium. This is a direct use of D3's built-in constraint mechanism, which is itself an instance of the general approach of mixing free and fixed nodes in force-directed layouts.

### Overall Architecture — Hybrid Sugiyama + Force Layout

**References.**
- Gansner, Koren & North, "Graph Drawing by Stress Majorization," *GD 2004*, LNCS 3383. — Stress-based layout as an alternative to force-directed.
- Dwyer, "Scalable, Versatile and Simple Constrained Graph Layout," *EuroVis 2009*. — Constrained force-directed layout combining Sugiyama-style constraints with force simulation.

**Relationship: instance of constrained force-directed layout.** The Call Graph algorithm is a hybrid that uses Sugiyama for initialization (phases 1–3) and force simulation for refinement (phase 4). This is an instance of the broader pattern described by Dwyer: **constrained force-directed layout**, where some positional constraints (here: the strong x-force pulling nodes to their assigned layer) are maintained while allowing the force simulation to optimize other aesthetic criteria (minimizing edge lengths, avoiding overlap, distributing nodes evenly).

The advantage over pure Sugiyama (used by File Map and Crate Map via dagre): interactivity. Nodes can be dragged and the graph re-equilibrates in real time. The advantage over pure force-directed: the layered structure is preserved, giving the graph a readable left-to-right flow that pure force layouts lack.
