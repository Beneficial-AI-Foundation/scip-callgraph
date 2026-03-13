/**
 * Context building for the AI chat assistant.
 *
 * Converts graph state into compact text that fits within an LLM's context
 * window.  The output is used both as the system prompt and to keep the AI
 * informed of the user's current view after manual filter changes.
 */

import type { ViewContext, ViewerStateAccessor } from './types';

/**
 * Capture a snapshot of the current viewer state for injection into the
 * conversation.  Called at send-time so the AI always sees fresh context.
 */
export function buildViewContext(accessor: ViewerStateAccessor): ViewContext {
  const filtered = accessor.getFilteredGraph();
  const selected = accessor.getSelectedNode();
  const filters = accessor.getFilters();

  return {
    activeView: accessor.getActiveView(),
    filters: { ...filters },
    displayedNodeCount: filtered?.nodes.length ?? 0,
    displayedEdgeCount: filtered?.links.length ?? 0,
    selectedNode: selected ? {
      id: selected.id,
      displayName: selected.display_name,
      kind: selected.kind || 'unknown',
      verificationStatus: selected.verification_status,
      callerCount: selected.dependents?.length ?? 0,
      calleeCount: selected.dependencies?.length ?? 0,
      crateName: selected.crate_name,
      filePath: selected.relative_path,
    } : null,
  };
}

/**
 * Format the current view context as text for the AI.
 */
export function formatViewContext(ctx: ViewContext): string {
  const lines: string[] = ['Current View State:'];
  lines.push(`- View: ${ctx.activeView}`);
  lines.push(`- Showing: ${ctx.displayedNodeCount} nodes, ${ctx.displayedEdgeCount} edges`);

  const activeFilters: string[] = [];
  if (ctx.filters.sourceQuery) activeFilters.push(`source="${ctx.filters.sourceQuery}"`);
  if (ctx.filters.sinkQuery) activeFilters.push(`sink="${ctx.filters.sinkQuery}"`);
  if (ctx.filters.maxDepth !== null) activeFilters.push(`depth=${ctx.filters.maxDepth}`);
  if (ctx.filters.includeFiles) activeFilters.push(`files="${ctx.filters.includeFiles}"`);
  if (ctx.filters.excludeNamePatterns) activeFilters.push(`exclude="${ctx.filters.excludeNamePatterns}"`);
  if (!ctx.filters.showExecFunctions) activeFilters.push('exec=hidden');
  if (!ctx.filters.showProofFunctions) activeFilters.push('proof=hidden');
  if (ctx.filters.showSpecFunctions) activeFilters.push('spec=shown');
  if (!ctx.filters.showVerifiedNodes) activeFilters.push('verified=hidden');
  if (!ctx.filters.showFailedNodes) activeFilters.push('failed=hidden');
  if (!ctx.filters.showUnverifiedNodes) activeFilters.push('unverified=hidden');

  if (activeFilters.length > 0) {
    lines.push(`- Filters: ${activeFilters.join(', ')}`);
  } else {
    lines.push('- Filters: none (full graph)');
  }

  if (ctx.selectedNode) {
    const s = ctx.selectedNode;
    lines.push(`- Selected: ${s.displayName} (${s.kind}, ${s.verificationStatus ?? 'unknown'}, ${s.callerCount} callers, ${s.calleeCount} callees, ${s.crateName}, ${s.filePath})`);
  }

  return lines.join('\n');
}

/**
 * Build the full system prompt incorporating graph summary, view context,
 * domain-aware explanations, and few-shot examples.
 */
export { buildFullSystemPrompt as buildSystemPrompt } from './prompts';
