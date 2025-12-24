import * as d3 from 'd3';
import { D3Graph, D3Node, D3Link, GraphState } from './types';

/**
 * Compute topological depth for each node in the graph.
 * Nodes with no incoming edges (callers) are at depth 0.
 * Depth increases as we follow call edges.
 * Returns a Map from node id to depth.
 */
function computeTopologicalDepth(nodes: D3Node[], links: D3Link[]): Map<string, number> {
  const depths = new Map<string, number>();
  const nodeIds = new Set(nodes.map(n => n.id));
  
  // Build adjacency lists
  const incomingEdges = new Map<string, string[]>();  // node -> list of nodes that call it
  const outgoingEdges = new Map<string, string[]>();  // node -> list of nodes it calls
  
  for (const node of nodes) {
    incomingEdges.set(node.id, []);
    outgoingEdges.set(node.id, []);
  }
  
  for (const link of links) {
    const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
    const targetId = typeof link.target === 'string' ? link.target : link.target.id;
    
    // Only consider edges between nodes in our current graph
    if (nodeIds.has(sourceId) && nodeIds.has(targetId)) {
      outgoingEdges.get(sourceId)?.push(targetId);
      incomingEdges.get(targetId)?.push(sourceId);
    }
  }
  
  // Find root nodes (no incoming edges) - these are "entry points"
  const roots: string[] = [];
  for (const node of nodes) {
    if (incomingEdges.get(node.id)?.length === 0) {
      roots.push(node.id);
    }
  }
  
  // If no roots found (cycles?), use nodes with lowest in-degree
  if (roots.length === 0) {
    const nodesByInDegree = [...nodes].sort((a, b) => 
      (incomingEdges.get(a.id)?.length || 0) - (incomingEdges.get(b.id)?.length || 0)
    );
    if (nodesByInDegree.length > 0) {
      roots.push(nodesByInDegree[0].id);
    }
  }
  
  // BFS to compute depths
  const queue: Array<{ id: string; depth: number }> = roots.map(id => ({ id, depth: 0 }));
  
  while (queue.length > 0) {
    const { id, depth } = queue.shift()!;
    
    // Take max depth if already visited (handles DAG correctly)
    const currentDepth = depths.get(id);
    if (currentDepth !== undefined && currentDepth >= depth) {
      continue;
    }
    
    depths.set(id, depth);
    
    // Process callees (outgoing edges)
    for (const calleeId of outgoingEdges.get(id) || []) {
      queue.push({ id: calleeId, depth: depth + 1 });
    }
  }
  
  // Handle any unvisited nodes (disconnected components)
  for (const node of nodes) {
    if (!depths.has(node.id)) {
      depths.set(node.id, 0);
    }
  }
  
  return depths;
}

/**
 * Assign initial positions based on topological order.
 * X position is based on depth (layers from left to right).
 * Y position spreads nodes evenly within each layer.
 */
function assignTopologicalPositions(
  nodes: D3Node[],
  links: D3Link[],
  width: number,
  height: number
): void {
  const depths = computeTopologicalDepth(nodes, links);
  
  // Group nodes by depth
  const nodesByDepth = new Map<number, D3Node[]>();
  let maxDepth = 0;
  
  for (const node of nodes) {
    const depth = depths.get(node.id) || 0;
    maxDepth = Math.max(maxDepth, depth);
    
    if (!nodesByDepth.has(depth)) {
      nodesByDepth.set(depth, []);
    }
    nodesByDepth.get(depth)!.push(node);
  }
  
  // Sort nodes within each depth by their display name for consistency
  for (const [depth, nodesAtDepth] of nodesByDepth) {
    nodesAtDepth.sort((a, b) => a.display_name.localeCompare(b.display_name));
    nodesByDepth.set(depth, nodesAtDepth);
  }
  
  // Assign positions
  const padding = 100;
  const usableWidth = width - 2 * padding;
  const usableHeight = height - 2 * padding;
  
  // Calculate x spacing based on number of depths
  const xSpacing = maxDepth > 0 ? usableWidth / maxDepth : usableWidth / 2;
  
  for (const [depth, nodesAtDepth] of nodesByDepth) {
    // X position based on depth (left to right)
    const x = padding + depth * xSpacing;
    
    // Y positions spread evenly
    const ySpacing = usableHeight / (nodesAtDepth.length + 1);
    
    nodesAtDepth.forEach((node, index) => {
      node.x = x;
      node.y = padding + (index + 1) * ySpacing;
    });
  }
}

export class CallGraphVisualization {
  private svg: d3.Selection<SVGSVGElement, unknown, null, undefined>;
  private g: d3.Selection<SVGGElement, unknown, null, undefined>;
  private width: number;
  private height: number;
  private simulation: d3.Simulation<D3Node, D3Link> | null = null;
  private state: GraphState;
  private onStateChange: (state: GraphState, selectionChanged?: boolean) => void;

  private linkElements: d3.Selection<SVGLineElement, D3Link, SVGGElement, unknown> | null = null;
  private nodeElements: d3.Selection<SVGCircleElement, D3Node, SVGGElement, unknown> | null = null;
  private textElements: d3.Selection<SVGTextElement, D3Node, SVGGElement, unknown> | null = null;

  constructor(
    container: HTMLElement,
    state: GraphState,
    onStateChange: (state: GraphState, selectionChanged?: boolean) => void
  ) {
    this.state = state;
    this.onStateChange = onStateChange;

    // Get container dimensions
    const rect = container.getBoundingClientRect();
    this.width = rect.width;
    this.height = rect.height;

    // Create SVG
    this.svg = d3
      .select(container)
      .append('svg')
      .attr('width', '100%')
      .attr('height', '100%')
      .attr('viewBox', `0 0 ${this.width} ${this.height}`);

    // Add zoom behavior
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 10])
      .on('zoom', (event) => {
        this.g.attr('transform', event.transform);
      });

    this.svg.call(zoom);

    // Create main group for zooming/panning
    this.g = this.svg.append('g');

    // Add arrow markers for directed edges
    this.svg
      .append('defs')
      .append('marker')
      .attr('id', 'arrowhead')
      .attr('viewBox', '0 -5 10 10')
      .attr('refX', 20)
      .attr('refY', 0)
      .attr('markerWidth', 6)
      .attr('markerHeight', 6)
      .attr('orient', 'auto')
      .append('path')
      .attr('d', 'M0,-5L10,0L0,5')
      .attr('fill', '#999');
  }

  /**
   * Update the visualization with new filtered graph
   */
  public update(filteredGraph: D3Graph): void {
    if (!filteredGraph || filteredGraph.nodes.length === 0) {
      this.clear();
      return;
    }

    // Stop existing simulation
    if (this.simulation) {
      this.simulation.stop();
    }

    // Create a deep copy of nodes and links for D3
    const nodes = filteredGraph.nodes.map(node => ({ ...node }));
    const links = filteredGraph.links.map(link => ({ ...link }));

    // Assign initial positions based on topological order
    // This ensures consistent, deterministic layout
    assignTopologicalPositions(nodes, links, this.width, this.height);

    // Compute target X positions for each node based on depth (for x-force)
    const depths = computeTopologicalDepth(nodes, links);
    const maxDepth = Math.max(...depths.values(), 1);
    const padding = 100;
    const usableWidth = this.width - 2 * padding;
    const xSpacing = maxDepth > 0 ? usableWidth / maxDepth : usableWidth / 2;
    
    // Store target x position for each node
    const targetX = new Map<string, number>();
    for (const node of nodes) {
      const depth = depths.get(node.id) || 0;
      targetX.set(node.id, padding + depth * xSpacing);
    }

    // Create force simulation with layered layout
    this.simulation = d3
      .forceSimulation<D3Node>(nodes)
      .force(
        'link',
        d3.forceLink<D3Node, D3Link>(links)
          .id(d => d.id)
          .distance(80)
          .strength(0.5)
      )
      .force('charge', d3.forceManyBody().strength(-200))
      // Use forceX to keep nodes at their topological layer
      .force('x', d3.forceX<D3Node>(d => targetX.get(d.id) || this.width / 2).strength(0.8))
      // Use forceY to center vertically with some spread
      .force('y', d3.forceY(this.height / 2).strength(0.05))
      .force('collision', d3.forceCollide().radius(35));

    // Update links
    this.linkElements = this.g
      .selectAll<SVGLineElement, D3Link>('line')
      .data(links, (d: D3Link) => {
        const source = typeof d.source === 'string' ? d.source : d.source.id;
        const target = typeof d.target === 'string' ? d.target : d.target.id;
        return `${source}-${target}`;
      });

    this.linkElements.exit().remove();

    const linkEnter = this.linkElements
      .enter()
      .append('line')
      .attr('class', 'link')
      .attr('stroke', (d) => {
        // Color links based on type
        const linkType = d.type || 'inner';
        if (linkType === 'precondition') return '#e65100';  // Orange for requires
        if (linkType === 'postcondition') return '#c2185b'; // Pink for ensures
        return '#999';  // Default gray for inner/body calls
      })
      .attr('stroke-opacity', 0.6)
      .attr('stroke-width', 1.5)
      .attr('stroke-dasharray', (d) => {
        // Dash pattern for spec links
        const linkType = d.type || 'inner';
        if (linkType === 'precondition' || linkType === 'postcondition') return '5,3';
        return 'none';
      })
      .attr('marker-end', 'url(#arrowhead)');

    this.linkElements = linkEnter.merge(this.linkElements);

    // Update nodes
    this.nodeElements = this.g
      .selectAll<SVGCircleElement, D3Node>('circle')
      .data(nodes, (d: D3Node) => d.id);

    this.nodeElements.exit().remove();

    const nodeEnter = this.nodeElements
      .enter()
      .append('circle')
      .attr('class', 'node')
      .attr('r', (d) => Math.max(5, Math.min(15, Math.sqrt(d.dependents.length + d.dependencies.length) * 2)))
      .attr('fill', (d) => this.getNodeColor(d))
      .attr('stroke', '#fff')
      .attr('stroke-width', 2)
      .style('cursor', 'pointer')
      .call(this.dragBehavior() as any);

    // Add event handlers
    nodeEnter
      .on('click', (event, d) => this.handleNodeClick(event, d))
      .on('mouseenter', (event, d) => this.handleNodeHover(event, d))
      .on('mouseleave', () => this.handleNodeLeave());

    this.nodeElements = nodeEnter.merge(this.nodeElements);

    // Update text labels
    this.textElements = this.g
      .selectAll<SVGTextElement, D3Node>('text')
      .data(nodes, (d: D3Node) => d.id);

    this.textElements.exit().remove();

    const textEnter = this.textElements
      .enter()
      .append('text')
      .attr('class', 'label')
      .attr('text-anchor', 'middle')
      .attr('dy', -20)
      .attr('font-size', '10px')
      .attr('font-family', 'sans-serif')
      .attr('fill', '#333')
      .attr('pointer-events', 'none')
      .text((d) => d.display_name);

    this.textElements = textEnter.merge(this.textElements);

    // Update positions on each tick
    this.simulation.on('tick', () => {
      this.linkElements
        ?.attr('x1', (d) => (d.source as D3Node).x!)
        .attr('y1', (d) => (d.source as D3Node).y!)
        .attr('x2', (d) => (d.target as D3Node).x!)
        .attr('y2', (d) => (d.target as D3Node).y!);

      this.nodeElements
        ?.attr('cx', (d) => d.x!)
        .attr('cy', (d) => d.y!);

      this.textElements
        ?.attr('x', (d) => d.x!)
        .attr('y', (d) => d.y!);
    });

    // Reheat simulation
    this.simulation.alpha(1).restart();
  }

  /**
   * Get node color based on verification status
   * - verified: green (#22c55e)
   * - failed: red (#ef4444)
   * - unverified: grey (#9ca3af)
   * - unknown (no status): blue (#3b82f6)
   */
  private getNodeColor(node: D3Node): string {
    switch (node.verification_status) {
      case 'verified':
        return '#22c55e';  // Green
      case 'failed':
        return '#ef4444';  // Red
      case 'unverified':
        return '#9ca3af';  // Grey
      default:
        return '#3b82f6';  // Blue (unknown/no verification status)
    }
  }

  /**
   * Drag behavior for nodes
   */
  private dragBehavior(): d3.DragBehavior<Element, D3Node, D3Node | d3.SubjectPosition> {
    return d3
      .drag<Element, D3Node>()
      .on('start', (event, d) => {
        if (!event.active && this.simulation) {
          this.simulation.alphaTarget(0.3).restart();
        }
        d.fx = d.x;
        d.fy = d.y;
      })
      .on('drag', (event, d) => {
        d.fx = event.x;
        d.fy = event.y;
      })
      .on('end', (event, d) => {
        if (!event.active && this.simulation) {
          this.simulation.alphaTarget(0);
        }
        d.fx = null;
        d.fy = null;
      });
  }

  /**
   * Handle node click
   * - Normal click: select/deselect node
   * - Shift+click: hide node
   */
  private handleNodeClick(event: MouseEvent, node: D3Node): void {
    event.stopPropagation();
    
    const newState = { ...this.state };
    
    // Shift+click: hide the node
    if (event.shiftKey) {
      newState.filters.hiddenNodes.add(node.id);
      // Clear selection if hiding the selected node
      if (newState.selectedNode?.id === node.id) {
        newState.selectedNode = null;
      }
      newState.filters.selectedNodes.delete(node.id);
      this.state = newState;
      this.onStateChange(newState, true);  // Triggers re-filter
      return;
    }
    
    // Normal click: toggle selection
    newState.selectedNode = node;
    
    // Toggle node selection for filtering
    if (newState.filters.selectedNodes.has(node.id)) {
      newState.filters.selectedNodes.delete(node.id);
    } else {
      newState.filters.selectedNodes.add(node.id);
    }
    
    this.state = newState;  // Keep local state in sync
    this.onStateChange(newState, true);  // Selection changed
    this.highlightNode(node);
  }

  /**
   * Handle node hover
   */
  private handleNodeHover(_event: MouseEvent, node: D3Node): void {
    const newState = { ...this.state };
    newState.hoveredNode = node;
    this.state = newState;  // Keep local state in sync
    this.onStateChange(newState, false);  // Hover only, no selection change
    
    // Highlight connected nodes
    this.nodeElements?.style('opacity', (d) => {
      if (d.id === node.id) return 1;
      // Check if connected
      const isConnected = this.state.filteredGraph?.links.some(link => {
        const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
        const targetId = typeof link.target === 'string' ? link.target : link.target.id;
        return (sourceId === node.id && targetId === d.id) ||
               (targetId === node.id && sourceId === d.id);
      });
      return isConnected ? 1 : 0.2;
    });

    this.linkElements?.style('opacity', (link) => {
      const sourceId = typeof link.source === 'string' ? link.source : link.source.id;
      const targetId = typeof link.target === 'string' ? link.target : link.target.id;
      return (sourceId === node.id || targetId === node.id) ? 1 : 0.1;
    });
  }

  /**
   * Handle node leave
   */
  private handleNodeLeave(): void {
    const newState = { ...this.state };
    newState.hoveredNode = null;
    this.state = newState;  // Keep local state in sync
    this.onStateChange(newState, false);  // Hover only, no selection change
    
    this.nodeElements?.style('opacity', 1);
    this.linkElements?.style('opacity', 0.6);
  }

  /**
   * Highlight a specific node
   */
  private highlightNode(node: D3Node): void {
    this.nodeElements
      ?.attr('stroke', (d) => (d.id === node.id ? '#ff6b6b' : '#fff'))
      .attr('stroke-width', (d) => (d.id === node.id ? 4 : 2));
  }

  /**
   * Clear the visualization
   */
  public clear(): void {
    this.g.selectAll('*').remove();
    if (this.simulation) {
      this.simulation.stop();
    }
  }

  /**
   * Resize the visualization
   */
  public resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
    this.svg.attr('viewBox', `0 0 ${width} ${height}`);
    
    if (this.simulation) {
      // Update the y-centering force for new height
      this.simulation
        .force('y', d3.forceY(height / 2).strength(0.05))
        .alpha(0.3)
        .restart();
    }
  }
}

