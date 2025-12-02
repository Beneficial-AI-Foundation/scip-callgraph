import * as d3 from 'd3';
import { D3Graph, D3Node, D3Link, GraphState } from './types';

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

    // Create force simulation
    this.simulation = d3
      .forceSimulation<D3Node>(nodes)
      .force(
        'link',
        d3.forceLink<D3Node, D3Link>(links)
          .id(d => d.id)
          .distance(100)
      )
      .force('charge', d3.forceManyBody().strength(-300))
      .force('center', d3.forceCenter(this.width / 2, this.height / 2))
      .force('collision', d3.forceCollide().radius(30));

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
      .attr('stroke', '#999')
      .attr('stroke-opacity', 0.6)
      .attr('stroke-width', 1.5)
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
      .attr('r', (d) => Math.max(5, Math.min(15, Math.sqrt(d.caller_count + d.callee_count) * 2)))
      .attr('fill', (d) => (d.is_libsignal ? '#4a90e2' : '#7ed321'))
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
   */
  private handleNodeClick(event: MouseEvent, node: D3Node): void {
    event.stopPropagation();
    
    const newState = { ...this.state };
    newState.selectedNode = node;
    
    // Toggle node selection for filtering
    if (newState.filters.selectedNodes.has(node.id)) {
      newState.filters.selectedNodes.delete(node.id);
    } else {
      newState.filters.selectedNodes.add(node.id);
    }
    
    this.onStateChange(newState, true);  // Selection changed
    this.highlightNode(node);
  }

  /**
   * Handle node hover
   */
  private handleNodeHover(_event: MouseEvent, node: D3Node): void {
    const newState = { ...this.state };
    newState.hoveredNode = node;
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
      this.simulation
        .force('center', d3.forceCenter(width / 2, height / 2))
        .alpha(0.3)
        .restart();
    }
  }
}

