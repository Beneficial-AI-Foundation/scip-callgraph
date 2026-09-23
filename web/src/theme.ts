/**
 * Design tokens shared between the stylesheet and the D3 views.
 *
 * The stylesheet (`style.css`) defines the same values as `--pg-*` custom
 * properties on `:root`; a host page embedding probegraph can override those
 * variables and the SVG views pick the overrides up at load time. The literals
 * here are fallbacks for environments without a computed stylesheet (tests).
 *
 * Palette follows the verilib.org family: Inter, off-white surfaces,
 * blue primary, green/red verification semantics.
 */

const FALLBACKS: Record<string, string> = {
  '--pg-accent': '#1e73da',
  '--pg-text': '#24292f',
  '--pg-text-muted': '#6b7280',
  '--pg-status-verified': '#3fbb14',
  '--pg-status-transitively-verified': '#217a0b',
  '--pg-status-trusted': '#9333ea',
  '--pg-status-failed': '#bb1416',
  '--pg-status-unverified': '#9ca3af',
  '--pg-status-unknown': '#217ef0',
  '--pg-edge-precondition': '#e65100',
  '--pg-edge-postcondition': '#c2185b',
  '--pg-edge-mapping': '#7c3aed',
  '--pg-edge-spec': '#0891b2',
  '--pg-selection': '#ff6b6b',
};

function cssVar(name: string): string {
  if (typeof document !== 'undefined') {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    if (v) return v;
  }
  return FALLBACKS[name] ?? '#999';
}

/** Node fill per verification status; `unknown` covers a missing status. */
export const STATUS_COLORS: Record<string, string> = {
  'verified': cssVar('--pg-status-verified'),
  'transitively-verified': cssVar('--pg-status-transitively-verified'),
  'trusted': cssVar('--pg-status-trusted'),
  'failed': cssVar('--pg-status-failed'),
  'unverified': cssVar('--pg-status-unverified'),
  'unknown': cssVar('--pg-status-unknown'),
};

export function statusColor(status: string | undefined | null): string {
  return STATUS_COLORS[status || 'unknown'] ?? STATUS_COLORS['unknown'];
}

/** Edge stroke per call type; plain calls use the neutral fallback. */
export const EDGE_TYPE_COLORS: Record<string, string> = {
  precondition: cssVar('--pg-edge-precondition'),
  postcondition: cssVar('--pg-edge-postcondition'),
  mapping: cssVar('--pg-edge-mapping'),
  spec: cssVar('--pg-edge-spec'),
};

export function edgeTypeColor(type: string | undefined, fallback = '#999'): string {
  return (type && EDGE_TYPE_COLORS[type]) || fallback;
}

export const ACCENT = cssVar('--pg-accent');
export const SELECTION_COLOR = cssVar('--pg-selection');
export const TEXT_COLOR = cssVar('--pg-text');
export const TEXT_MUTED = cssVar('--pg-text-muted');

/**
 * Categorical hues for file/crate grouping, shared by the File Map, Crate Map
 * and Hierarchy views at view-specific alphas.
 */
const GROUP_HUES: ReadonlyArray<[number, number, number]> = [
  [66, 133, 244],
  [234, 67, 53],
  [52, 168, 83],
  [251, 188, 4],
  [171, 71, 188],
  [0, 172, 193],
  [255, 112, 67],
  [124, 179, 66],
];

export const GROUP_HUE_COUNT = GROUP_HUES.length;

export function groupColors(alpha: number): string[] {
  return GROUP_HUES.map(([r, g, b]) => `rgba(${r},${g},${b},${alpha})`);
}
