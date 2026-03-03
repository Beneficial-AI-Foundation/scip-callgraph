import * as d3 from 'd3';
import dagreModule from '@dagrejs/dagre';
const dagre = dagreModule as any;
import { D3Graph, D3Node, D3Link, GraphState, BorderStatus, FillStatus, DeclKind, getKindSetsForLanguage } from './types';
import { transitiveReduction } from './graph-utils';

// --- Color palettes ---

const BORDER_COLORS: Record<BorderStatus, string> = {
  verified:  '#22c55e',
  ready:     '#3b82f6',
  blocked:   '#ef4444',
  not_ready: '#f59e0b',
  unknown:   '#9ca3af',
};

const FILL_COLORS: Record<FillStatus, string> = {
  fully_verified: '#1CAC78',
  verified:       '#9CEC8B',
  ready:          '#A3D6FF',
  none:           '#ffffff',
};

// --- File group background palette (soft pastels) ---

const FILE_GROUP_COLORS = [
  'rgba(66,133,244,0.10)',
  'rgba(234,67,53,0.10)',
  'rgba(52,168,83,0.10)',
  'rgba(251,188,4,0.10)',
  'rgba(171,71,188,0.10)',
  'rgba(0,172,193,0.10)',
  'rgba(255,112,67,0.10)',
  'rgba(124,179,66,0.10)',
];

const FILE_GROUP_STROKE_COLORS = [
  'rgba(66,133,244,0.30)',
  'rgba(234,67,53,0.30)',
  'rgba(52,168,83,0.30)',
  'rgba(251,188,4,0.30)',
  'rgba(171,71,188,0.30)',
  'rgba(0,172,193,0.30)',
  'rgba(255,112,67,0.30)',
  'rgba(124,179,66,0.30)',
];

// --- Node shape constants ---

const NODE_MIN_W = 120;
const NODE_H = 36;
const CHAR_WIDTH = 6.5;  // approx width per char at 10px system-ui
const NODE_PAD = 24;     // horizontal padding inside node shape

function nodeWidthFor(name: string): number {
  return Math.max(NODE_MIN_W, name.length * CHAR_WIDTH + NODE_PAD);
}

function borderColor(node: D3Node): string {
  return BORDER_COLORS[node.border_status || 'unknown'];
}

function fillColor(node: D3Node): string {
  return FILL_COLORS[node.fill_status || 'none'];
}

/**
 * Append the correct SVG shape for a node's declaration kind.
 *  - exec/definitions -> rounded rect
 *  - proof/theorems   -> ellipse
 *  - spec/axioms      -> diamond
 */
function appendShape(
  g: d3.Selection<SVGGElement, D3Node, SVGGElement, unknown>,
  proofKinds: Set<string>,
  specKinds: Set<string>,
  widths: Map<string, number>,
): void {
  const w = (d: D3Node) => widths.get(d.id) || NODE_MIN_W;
  const isExec = (kind: DeclKind) => !proofKinds.has(kind) && !specKinds.has(kind);

  g.filter(d => isExec(d.kind || 'exec'))
    .append('rect')
    .attr('class', 'bp-shape')
    .attr('x', d => -w(d) / 2).attr('y', -NODE_H / 2)
    .attr('width', d => w(d)).attr('height', NODE_H)
    .attr('rx', 6).attr('ry', 6);

  g.filter(d => proofKinds.has(d.kind || 'exec'))
    .append('ellipse')
    .attr('class', 'bp-shape')
    .attr('cx', 0).attr('cy', 0)
    .attr('rx', d => w(d) / 2).attr('ry', NODE_H / 2);

  const hh = NODE_H / 2;
  g.filter(d => specKinds.has(d.kind || 'exec'))
    .append('polygon')
    .attr('class', 'bp-shape')
    .attr('points', d => {
      const hw = w(d) / 2;
      return `0,${-hh} ${hw},0 0,${hh} ${-hw},0`;
    });
}

export class BlueprintVisualization {
  private svg: d3.Selection<SVGSVGElement, unknown, null, undefined>;
  private g: d3.Selection<SVGGElement, unknown, null, undefined>;
  private width: number;
  private height: number;
  private state: GraphState;
  private onStateChange: (state: GraphState, selectionChanged?: boolean) => void;
  private container: HTMLElement;
  private legendVisible = true;

  private linkSel: d3.Selection<SVGPathElement, D3Link, SVGGElement, unknown> | null = null;
  private nodeSel: d3.Selection<SVGGElement, D3Node, SVGGElement, unknown> | null = null;

  constructor(
    container: HTMLElement,
    state: GraphState,
    onStateChange: (state: GraphState, selectionChanged?: boolean) => void,
  ) {
    this.container = container;
    this.state = state;
    this.onStateChange = onStateChange;

    const rect = container.getBoundingClientRect();
    this.width = rect.width;
    this.height = rect.height;

    this.svg = d3.select(container)
      .append('svg')
      .attr('width', '100%')
      .attr('height', '100%')
      .attr('viewBox', `0 0 ${this.width} ${this.height}`)
      .attr('class', 'blueprint-svg');

    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.05, 8])
      .on('zoom', (event) => { this.g.attr('transform', event.transform); });
    this.svg.call(zoom);

    this.g = this.svg.append('g');

    // Arrow marker
    const defs = this.svg.append('defs');
    defs.append('marker')
      .attr('id', 'bp-arrow')
      .attr('viewBox', '0 -5 10 10')
      .attr('refX', 10).attr('refY', 0)
      .attr('markerWidth', 7).attr('markerHeight', 7)
      .attr('orient', 'auto')
      .append('path')
      .attr('d', 'M0,-4L10,0L0,4')
      .attr('fill', '#888');

    this.renderLegend();
  }

  // ----- public interface (same shape as CallGraphVisualization) -----

  public update(filteredGraph: D3Graph): void {
    if (!filteredGraph || filteredGraph.nodes.length === 0) {
      this.clear();
      return;
    }

    // Re-render legend in case projectLanguage changed after graph load
    this.renderLegend();

    const nodes = filteredGraph.nodes.map(n => ({ ...n }));
    let links = filteredGraph.links.map(l => ({ ...l }));

    // Transitive reduction for cleaner DAG
    links = transitiveReduction(nodes, links);

    // Group nodes by file for compound graph clustering
    const fileGroups = new Map<string, D3Node[]>();
    for (const node of nodes) {
      const key = node.relative_path || 'unknown';
      if (!fileGroups.has(key)) fileGroups.set(key, []);
      fileGroups.get(key)!.push(node);
    }
    const fileKeys = [...fileGroups.keys()];

    // Run dagre layout with compound graph mode
    const gGraph = new dagre.graphlib.Graph({ compound: true });
    gGraph.setGraph({
      rankdir: 'LR',
      nodesep: 30,
      ranksep: 160,
      marginx: 40,
      marginy: 40,
    });
    gGraph.setDefaultEdgeLabel(() => ({}));

    for (const key of fileKeys) {
      const groupId = `file:${key}`;
      gGraph.setNode(groupId, { label: key, clusterLabelPos: 'top' });
    }

    const nodeWidths = new Map<string, number>();
    for (const node of nodes) {
      const nw = nodeWidthFor(node.display_name);
      nodeWidths.set(node.id, nw);
      gGraph.setNode(node.id, { width: nw + 10, height: NODE_H + 10 });
      const fileKey = node.relative_path || 'unknown';
      gGraph.setParent(node.id, `file:${fileKey}`);
    }
    for (const link of links) {
      const s = typeof link.source === 'string' ? link.source : link.source.id;
      const t = typeof link.target === 'string' ? link.target : link.target.id;
      gGraph.setEdge(s, t);
    }

    dagre.layout(gGraph);

    // Apply dagre positions back to nodes
    const posMap = new Map<string, { x: number; y: number }>();
    for (const node of nodes) {
      const info = gGraph.node(node.id);
      if (info) {
        node.x = info.x;
        node.y = info.y;
        posMap.set(node.id, { x: info.x, y: info.y });
      }
    }

    // Clear previous render
    this.g.selectAll('*').remove();

    // Render file group backgrounds (behind everything else)
    for (let i = 0; i < fileKeys.length; i++) {
      const groupId = `file:${fileKeys[i]}`;
      const info = gGraph.node(groupId);
      if (!info || !info.width || !info.height) continue;
      const color = FILE_GROUP_COLORS[i % FILE_GROUP_COLORS.length];
      const stroke = FILE_GROUP_STROKE_COLORS[i % FILE_GROUP_STROKE_COLORS.length];

      this.g.append('rect')
        .attr('class', 'bp-file-group')
        .attr('x', info.x - info.width / 2)
        .attr('y', info.y - info.height / 2)
        .attr('width', info.width)
        .attr('height', info.height)
        .attr('rx', 8)
        .attr('fill', color)
        .attr('stroke', stroke)
        .attr('stroke-width', 1);

      this.g.append('text')
        .attr('class', 'bp-file-label')
        .attr('x', info.x - info.width / 2 + 8)
        .attr('y', info.y - info.height / 2 + 14)
        .text(extractFileName(fileKeys[i]));
    }

    // Render links
    this.linkSel = this.g.selectAll<SVGPathElement, D3Link>('path.bp-link')
      .data(links)
      .enter()
      .append('path')
      .attr('class', 'bp-link')
      .attr('fill', 'none')
      .attr('stroke', d => {
        const t = d.type || 'inner';
        if (t === 'precondition') return '#e65100';
        if (t === 'postcondition') return '#c2185b';
        return '#888';
      })
      .attr('stroke-opacity', 0.55)
      .attr('stroke-width', 1.5)
      .attr('stroke-dasharray', d => {
        const t = d.type || 'inner';
        return (t === 'precondition' || t === 'postcondition') ? '6,3' : 'none';
      })
      .attr('marker-end', 'url(#bp-arrow)')
      .attr('d', d => {
        const sId = typeof d.source === 'string' ? d.source : d.source.id;
        const tId = typeof d.target === 'string' ? d.target : d.target.id;
        const sp = posMap.get(sId);
        const tp = posMap.get(tId);
        if (!sp || !tp) return '';
        const sw = (nodeWidths.get(sId) || NODE_MIN_W) / 2;
        const tw = (nodeWidths.get(tId) || NODE_MIN_W) / 2;
        const dx = tp.x - sp.x;
        return `M${sp.x + sw},${sp.y} C${sp.x + sw + dx * 0.4},${sp.y} ${tp.x - tw - dx * 0.4},${tp.y} ${tp.x - tw},${tp.y}`;
      });

    // Render node groups
    this.nodeSel = this.g.selectAll<SVGGElement, D3Node>('g.bp-node')
      .data(nodes, d => d.id)
      .enter()
      .append('g')
      .attr('class', 'bp-node')
      .attr('transform', d => `translate(${d.x},${d.y})`)
      .style('cursor', 'pointer');

    // Append shape based on language-aware kind categories
    const { proofKinds, specKinds } = getKindSetsForLanguage(this.state.projectLanguage);
    appendShape(this.nodeSel, proofKinds, specKinds, nodeWidths);

    // Style shapes with dual-channel colors
    this.nodeSel.selectAll<SVGElement, D3Node>('.bp-shape')
      .attr('fill', d => fillColor(d))
      .attr('stroke', d => borderColor(d))
      .attr('stroke-width', 2.5);

    // Labels
    this.nodeSel.append('text')
      .attr('text-anchor', 'middle')
      .attr('dominant-baseline', 'central')
      .attr('font-size', '10px')
      .attr('font-family', 'system-ui, sans-serif')
      .attr('fill', d => {
        const fs = d.fill_status || 'none';
        return (fs === 'fully_verified' || fs === 'verified') ? '#1a3a1a' : '#333';
      })
      .attr('pointer-events', 'none')
      .text(d => d.display_name);

    // Interactions
    this.nodeSel
      .on('click', (event, d) => this.handleClick(event, d))
      .on('mouseenter', (_event, d) => this.handleHover(d))
      .on('mouseleave', () => this.handleLeave());

    // Auto-fit to content
    const graphInfo = gGraph.graph();
    if (graphInfo && graphInfo.width && graphInfo.height) {
      this.svg.attr('viewBox',
        `0 0 ${graphInfo.width + 80} ${graphInfo.height + 80}`);
    }
  }

  public destroy(): void {
    this.svg.remove();
    const legend = this.container.querySelector('.bp-legend');
    if (legend) legend.remove();
  }

  public resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }

  public clear(): void {
    this.g.selectAll('*').remove();
  }

  // ----- Legend -----

  private renderLegend(): void {
    const existing = this.container.querySelector('.bp-legend');
    if (existing) existing.remove();

    const lang = this.state.projectLanguage;
    const rectLabel = lang === 'lean' ? 'Definition' : lang === 'verus' ? 'Exec function' : 'Exec / Definition';
    const ellipseLabel = lang === 'lean' ? 'Theorem' : lang === 'verus' ? 'Proof / lemma' : 'Proof / Theorem';
    const diamondLabel = lang === 'lean' ? 'Axiom' : lang === 'verus' ? 'Spec function' : 'Spec / Axiom';

    const legend = document.createElement('div');
    legend.className = 'bp-legend';
    legend.innerHTML = `
      <div class="bp-legend-header" id="bp-legend-toggle">
        <strong>Legend</strong> <span class="bp-legend-arrow">${this.legendVisible ? '&#9660;' : '&#9654;'}</span>
      </div>
      <div class="bp-legend-body" style="display:${this.legendVisible ? 'block' : 'none'}">
        <div class="bp-legend-section"><strong>Shapes</strong></div>
        <div class="bp-legend-item">
          <svg width="28" height="18"><rect x="2" y="2" width="24" height="14" rx="3" fill="#eee" stroke="#888" stroke-width="1.5"/></svg>
          <span>${rectLabel}</span>
        </div>
        <div class="bp-legend-item">
          <svg width="28" height="18"><ellipse cx="14" cy="9" rx="12" ry="7" fill="#eee" stroke="#888" stroke-width="1.5"/></svg>
          <span>${ellipseLabel}</span>
        </div>
        <div class="bp-legend-item">
          <svg width="28" height="18"><polygon points="14,1 27,9 14,17 1,9" fill="#eee" stroke="#888" stroke-width="1.5"/></svg>
          <span>${diamondLabel}</span>
        </div>
        <div class="bp-legend-section"><strong>Border&nbsp;color</strong></div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${BORDER_COLORS.verified}"></span>Verified</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${BORDER_COLORS.ready}"></span>Ready (deps done)</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${BORDER_COLORS.not_ready}"></span>Not ready</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${BORDER_COLORS.blocked}"></span>Failed</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${BORDER_COLORS.unknown}"></span>Unknown</div>
        <div class="bp-legend-section"><strong>Fill&nbsp;color</strong></div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${FILL_COLORS.fully_verified}"></span>Fully verified (+ ancestors)</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${FILL_COLORS.verified}"></span>Verified</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${FILL_COLORS.ready}"></span>Ready</div>
        <div class="bp-legend-item"><span class="bp-swatch" style="background:${FILL_COLORS.none};border:1px solid #ccc"></span>Not done</div>
        <div class="bp-legend-section"><strong>Edges</strong></div>
        <div class="bp-legend-item">
          <svg width="28" height="10"><line x1="0" y1="5" x2="28" y2="5" stroke="#888" stroke-width="1.5"/></svg>
          <span>Body call</span>
        </div>
        <div class="bp-legend-item">
          <svg width="28" height="10"><line x1="0" y1="5" x2="28" y2="5" stroke="#e65100" stroke-width="1.5" stroke-dasharray="4,2"/></svg>
          <span>Requires</span>
        </div>
        <div class="bp-legend-item">
          <svg width="28" height="10"><line x1="0" y1="5" x2="28" y2="5" stroke="#c2185b" stroke-width="1.5" stroke-dasharray="4,2"/></svg>
          <span>Ensures</span>
        </div>
      </div>
    `;
    this.container.appendChild(legend);

    legend.querySelector('#bp-legend-toggle')?.addEventListener('click', () => {
      this.legendVisible = !this.legendVisible;
      const body = legend.querySelector('.bp-legend-body') as HTMLElement;
      const arrow = legend.querySelector('.bp-legend-arrow') as HTMLElement;
      if (body) body.style.display = this.legendVisible ? 'block' : 'none';
      if (arrow) arrow.innerHTML = this.legendVisible ? '&#9660;' : '&#9654;';
    });
  }

  // ----- Interactions -----

  private handleClick(event: MouseEvent, node: D3Node): void {
    event.stopPropagation();
    const newState = { ...this.state };

    if (event.shiftKey) {
      newState.filters.hiddenNodes.add(node.id);
      if (newState.selectedNode?.id === node.id) newState.selectedNode = null;
      newState.filters.selectedNodes.delete(node.id);
      this.state = newState;
      this.onStateChange(newState, true);
      return;
    }

    newState.selectedNode = node;
    if (newState.filters.selectedNodes.has(node.id)) {
      newState.filters.selectedNodes.delete(node.id);
    } else {
      newState.filters.selectedNodes.add(node.id);
    }
    this.state = newState;
    this.onStateChange(newState, true);

    // Highlight selected
    this.nodeSel?.selectAll<SVGElement, D3Node>('.bp-shape')
      .attr('stroke-width', d => d.id === node.id ? 4 : 2.5)
      .attr('stroke', d => d.id === node.id ? '#ff6b6b' : borderColor(d));
  }

  private handleHover(node: D3Node): void {
    const newState = { ...this.state };
    newState.hoveredNode = node;
    this.state = newState;
    this.onStateChange(newState, false);

    const connectedIds = new Set<string>();
    connectedIds.add(node.id);
    this.state.filteredGraph?.links.forEach(link => {
      const s = typeof link.source === 'string' ? link.source : link.source.id;
      const t = typeof link.target === 'string' ? link.target : link.target.id;
      if (s === node.id) connectedIds.add(t);
      if (t === node.id) connectedIds.add(s);
    });

    this.nodeSel?.style('opacity', d => connectedIds.has(d.id) ? 1 : 0.2);
    this.linkSel?.style('opacity', d => {
      const s = typeof d.source === 'string' ? d.source : d.source.id;
      const t = typeof d.target === 'string' ? d.target : d.target.id;
      return (s === node.id || t === node.id) ? 1 : 0.08;
    });
  }

  private handleLeave(): void {
    const newState = { ...this.state };
    newState.hoveredNode = null;
    this.state = newState;
    this.onStateChange(newState, false);

    this.nodeSel?.style('opacity', 1);
    this.linkSel?.style('opacity', 0.55);
  }
}

function extractFileName(relativePath: string): string {
  const parts = relativePath.split('/').filter(Boolean);
  if (parts.length <= 2) return relativePath;
  return parts.slice(-2).join('/');
}
