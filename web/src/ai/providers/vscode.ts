/**
 * VS Code Language Model API provider.
 *
 * Uses vscode.lm.selectChatModels() when running as a VS Code webview.
 * Since the VS Code LM API may not support native tool calling, this
 * provider uses a prompt-based fallback: the system prompt instructs
 * the model to output <action> blocks, which we parse client-side.
 *
 * Communication happens via postMessage to the extension host, which
 * has access to the vscode.lm API.
 */

import type { LLMProvider, ChatResponse, ToolDef, ToolCall } from '../types';

interface VSCodeAPI {
  postMessage(message: unknown): void;
}

export class VSCodeProvider implements LLMProvider {
  id = 'vscode';
  name = 'VS Code Copilot';

  private vscodeApi: VSCodeAPI | null = null;
  private pendingResolve: ((response: ChatResponse) => void) | null = null;

  constructor() {
    if (typeof window !== 'undefined' && typeof (window as any).acquireVsCodeApi === 'function') {
      // Note: acquireVsCodeApi can only be called once; main.ts may have already acquired it.
      // We'll use postMessage to the extension which handles the LM API calls.
      this.setupMessageListener();
    }
  }

  async available(): Promise<boolean> {
    return typeof (window as any).acquireVsCodeApi === 'function';
  }

  private setupMessageListener(): void {
    window.addEventListener('message', (event: MessageEvent) => {
      const data = event.data;
      if (data?.type === 'lmResponse' && this.pendingResolve) {
        const resolve = this.pendingResolve;
        this.pendingResolve = null;
        resolve(parseLMResponse(data));
      }
    });
  }

  async chat(
    messages: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string }>,
    tools: ToolDef[],
  ): Promise<ChatResponse> {
    // Append tool-calling instructions to system message for prompt-based extraction
    const messagesWithToolInstructions = injectToolInstructions(messages, tools);

    return new Promise<ChatResponse>((resolve) => {
      this.pendingResolve = resolve;

      // Send to extension host for LM API processing
      const api = this.getVSCodeAPI();
      if (!api) {
        resolve({ content: 'VS Code API not available.', finishReason: 'error' });
        return;
      }

      api.postMessage({
        type: 'lmRequest',
        messages: messagesWithToolInstructions,
      });

      // Timeout after 30s
      setTimeout(() => {
        if (this.pendingResolve === resolve) {
          this.pendingResolve = null;
          resolve({ content: 'VS Code LM request timed out.', finishReason: 'error' });
        }
      }, 30000);
    });
  }

  private getVSCodeAPI(): VSCodeAPI | null {
    if (this.vscodeApi) return this.vscodeApi;
    try {
      this.vscodeApi = (window as any).acquireVsCodeApi?.();
    } catch {
      // Already acquired elsewhere -- we'll rely on postMessage routing
    }
    return this.vscodeApi;
  }
}

/**
 * Inject tool-calling instructions into the system message.
 * The model is asked to output <action> blocks when it wants to call tools.
 */
function injectToolInstructions(
  messages: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string }>,
  tools: ToolDef[],
): Array<{ role: string; content: string }> {
  if (tools.length === 0) {
    return messages.map(m => ({ role: m.role, content: m.content }));
  }

  const toolDescriptions = tools.map(t => {
    const params = Object.entries(t.parameters.properties)
      .map(([name, p]) => `  ${name}: ${(p as any).type} - ${(p as any).description}`)
      .join('\n');
    return `${t.name}: ${t.description}\n  Parameters:\n${params}`;
  }).join('\n\n');

  const instructions = `

When you want to perform an action on the viewer, output an <action> block like this:
<action>{"tool": "tool_name", "args": {"param": "value"}}</action>

You can include multiple <action> blocks in one response. After each action, explain what you did.

Available tools:
${toolDescriptions}`;

  return messages.map(m => {
    if (m.role === 'system') {
      return { role: 'system', content: m.content + instructions };
    }
    return { role: m.role, content: m.content };
  });
}

/**
 * Parse the LM response from the extension host.
 * Extracts <action> blocks as tool calls if present.
 */
function parseLMResponse(data: any): ChatResponse {
  const content: string = data.content || '';

  // Extract <action> blocks
  const actionRegex = /<action>(.*?)<\/action>/gs;
  const toolCalls: ToolCall[] = [];
  let textContent = content;
  let match;
  let callId = 0;

  while ((match = actionRegex.exec(content)) !== null) {
    try {
      const parsed = JSON.parse(match[1]);
      toolCalls.push({
        id: `vsc_${callId++}`,
        name: parsed.tool,
        arguments: parsed.args || {},
      });
      textContent = textContent.replace(match[0], '');
    } catch {
      // Malformed action block -- leave in text
    }
  }

  textContent = textContent.trim();

  return {
    content: textContent,
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
    finishReason: toolCalls.length > 0 ? 'tool_calls' : 'stop',
  };
}
