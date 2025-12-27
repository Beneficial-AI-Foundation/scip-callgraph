# VS Code Extension Integration

The call graph web viewer can be embedded in a VS Code extension as a webview panel, enabling interactive call graph exploration directly within the editor.

## Overview

When running inside a VS Code webview, the viewer:
- Receives graph data from the extension (no file picker needed)
- Opens source files directly in the editor instead of GitHub links
- Communicates bidirectionally with the extension via message passing

## Building for VS Code

Use the dedicated build command that produces assets optimized for webview embedding:

```bash
cd web
npm run build:vscode
```

This outputs to `web/dist-vscode/` with:
- **Relative paths** (`./` base) instead of absolute paths
- **Inlined assets** for easier loading
- **Predictable filenames** (`assets/main.js`, `assets/main.css`)
- **No public folder** (graph.json not bundled - sent via messages)

### Build Configuration

The `vite.config.vscode.js` configures:

```javascript
{
  base: './',              // Relative paths for webview
  publicDir: false,        // Don't copy public folder
  build: {
    outDir: 'dist-vscode',
    assetsInlineLimit: 100000,  // Inline most assets
  }
}
```

## Message Protocol

### Extension → Webview

#### `loadGraph`

Send graph data to the webview:

```typescript
panel.webview.postMessage({
  type: 'loadGraph',
  graph: {
    nodes: [...],
    links: [...],
    metadata: {...}
  },
  initialQuery: {           // Optional
    source: 'my_function',
    sink: '',
    depth: 3
  }
});
```

#### `setQuery`

Update the current filter query:

```typescript
panel.webview.postMessage({
  type: 'setQuery',
  source: 'function_name',  // Optional
  sink: 'other_function'    // Optional
});
```

#### `refresh`

Request the webview to ask for fresh data:

```typescript
panel.webview.postMessage({
  type: 'refresh'
});
```

### Webview → Extension

#### `ready`

Sent when the webview has initialized and is ready to receive data:

```typescript
{
  type: 'ready'
}
```

#### `navigate`

Sent when the user clicks "Open in Editor" on a node:

```typescript
{
  type: 'navigate',
  relativePath: 'src/lib.rs',
  startLine: 42,
  endLine: 58,
  displayName: 'my_function'
}
```

#### `requestRefresh`

Sent when the webview wants fresh graph data:

```typescript
{
  type: 'requestRefresh'
}
```

## Example Extension Code

### Creating the Webview Panel

```typescript
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export function createCallGraphPanel(
  context: vscode.ExtensionContext,
  graphData: any
): vscode.WebviewPanel {
  const panel = vscode.window.createWebviewPanel(
    'callGraphViewer',
    'Call Graph Explorer',
    vscode.ViewColumn.One,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [
        vscode.Uri.file(path.join(context.extensionPath, 'dist-vscode'))
      ]
    }
  );

  // Load the webview HTML
  const distPath = path.join(context.extensionPath, 'dist-vscode');
  panel.webview.html = getWebviewContent(panel.webview, distPath);

  // Handle messages from webview
  panel.webview.onDidReceiveMessage(
    async (message) => {
      switch (message.type) {
        case 'ready':
          // Webview is ready, send the graph data
          panel.webview.postMessage({
            type: 'loadGraph',
            graph: graphData
          });
          break;

        case 'navigate':
          // Open file in editor
          await openFileAtLocation(
            message.relativePath,
            message.startLine,
            message.endLine
          );
          break;

        case 'requestRefresh':
          // Regenerate graph and send new data
          const freshData = await generateCallGraph();
          panel.webview.postMessage({
            type: 'loadGraph',
            graph: freshData
          });
          break;
      }
    },
    undefined,
    context.subscriptions
  );

  return panel;
}
```

### Loading the Webview HTML

```typescript
function getWebviewContent(
  webview: vscode.Webview,
  distPath: string
): string {
  const htmlPath = path.join(distPath, 'index.html');
  let html = fs.readFileSync(htmlPath, 'utf8');

  // Convert local file paths to webview URIs
  const assetUri = webview.asWebviewUri(
    vscode.Uri.file(path.join(distPath, 'assets'))
  );

  // Replace relative asset paths with webview URIs
  html = html.replace(
    /href="\.\/assets\//g,
    `href="${assetUri}/`
  );
  html = html.replace(
    /src="\.\/assets\//g,
    `src="${assetUri}/`
  );

  return html;
}
```

### Opening Files in Editor

```typescript
async function openFileAtLocation(
  relativePath: string,
  startLine: number,
  endLine: number
): Promise<void> {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) return;

  const filePath = path.join(workspaceFolder.uri.fsPath, relativePath);
  const uri = vscode.Uri.file(filePath);

  try {
    const document = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(document);

    // Jump to the function and highlight it
    const range = new vscode.Range(
      new vscode.Position(startLine - 1, 0),
      new vscode.Position(endLine - 1, 0)
    );
    editor.selection = new vscode.Selection(range.start, range.start);
    editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
  } catch (error) {
    vscode.window.showErrorMessage(`Could not open file: ${relativePath}`);
  }
}
```

## UI Differences in VS Code Mode

When running in VS Code webview:

| Feature | Web Mode | VS Code Mode |
|---------|----------|--------------|
| File loading | File picker dialog | Graph sent via message |
| Source navigation | Opens GitHub in new tab | Opens file in editor |
| Header title | "Call Graph Viewer" | "📊 Call Graph Explorer" |
| File input | Visible | Hidden |

## Environment Detection

The webview detects VS Code environment by checking for the `acquireVsCodeApi` function:

```typescript
function isVSCodeEnvironment(): boolean {
  return typeof (window as any).acquireVsCodeApi === 'function';
}
```

## Bundling the Viewer with Your Extension

1. Build the viewer:
   ```bash
   cd web && npm run build:vscode
   ```

2. Copy `dist-vscode/` to your extension directory

3. Add to your extension's `package.json`:
   ```json
   {
     "contributes": {
       "commands": [{
         "command": "myExtension.showCallGraph",
         "title": "Show Call Graph"
       }]
     }
   }
   ```

4. Register the command in your `extension.ts`

## Related Documentation

- [INTERACTIVE_VIEWER.md](./INTERACTIVE_VIEWER.md) - Web viewer features and usage
- [Web README](../../web/README.md) - Development setup for the web viewer

