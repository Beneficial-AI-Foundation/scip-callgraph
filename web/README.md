# probegraph web viewer

Interactive D3 visualization for probe graphs: Rust/Verus call graphs from
SCIP, Lean atom graphs from probe-lean, and merged cross-language graphs.
Usage is documented in [docs/guides/viewer.md](../docs/guides/viewer.md);
this file covers development.

## Quick start

```bash
npm install
npm run dev          # dev server on http://localhost:3000
```

Other scripts (see `package.json`):

```bash
npm run build         # production build (Pages base path /probegraph/)
npm run build:vscode  # VS Code webview build (relative base, dist-vscode/)
npm run type-check    # tsc --noEmit
npm test              # vitest, watch mode
npm run test:run      # vitest, single run
npx playwright test   # e2e suite in web/e2e/
```

CI runs `type-check` and `test:run` on every push to main (see
`.github/workflows/build.yml`); the Pages deploy also runs them before
building.

## Demo graph

`public/graph.json` is auto-loaded on startup and is the graph shown on the
GitHub Pages deploy (currently probe-lean output; check its `tool` field for
provenance). Files served from `public/` land at the site root, so any
`public/*.json` can be loaded via `?json=./name.json`. Don't commit ad-hoc
local graphs.

## Documentation

- [docs/guides/viewer.md](../docs/guides/viewer.md) — user guide: views,
  filters, URL parameters, sharing.
- [ARCHITECTURE.md](ARCHITECTURE.md) — module structure, input formats,
  data flow.
- [QUERY_PIPELINE.md](QUERY_PIPELINE.md) — the query/filter engine.
- [docs/technical/](docs/technical/) — layout and coloring algorithms per
  view.
- [../docs/guides/vscode-extension.md](../docs/guides/vscode-extension.md) —
  the VS Code webview integration.

## Troubleshooting

- **Nothing renders after loading a file** — check the browser console; the
  loader accepts probe atom dicts, schema envelopes, the native
  `{nodes, links, metadata}` format, and a legacy flat array. Anything else
  falls through with a warning.
- **"Large Graph Detected" prompt** — the file is over 10 MiB; set a Source,
  Sink, or Include Files filter and press Load & Search.
- **Large graph loads but shows a seeded subset** — expected for graphs over
  2 000 nodes / 10 000 links; see the seeded-view section of the viewer guide.
