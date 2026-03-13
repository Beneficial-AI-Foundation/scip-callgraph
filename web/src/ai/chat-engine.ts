/**
 * Chat engine -- manages conversation history, tool-call loop, and provider dispatch.
 *
 * Key design decisions:
 * - Sliding window: keeps last MAX_HISTORY_MESSAGES messages to stay within token limits.
 * - Tool-call loop: max MAX_TOOL_CALLS_PER_TURN iterations per user message.
 * - Fresh view context injected with every user message.
 * - Abort support via AbortController.
 */

import type {
  ChatMessage, LLMProvider, ToolResult,
  GraphSummary, ViewerStateAccessor,
} from './types';
import { TOOL_DEFINITIONS, executeTool } from './tools';
import { buildGraphSummary } from './static-analysis';
import { buildSystemPrompt, buildViewContext } from './context';

const MAX_HISTORY_MESSAGES = 20;
const MAX_TOOL_CALLS_PER_TURN = 5;

let messageIdCounter = 0;
function nextId(): string {
  return `msg_${++messageIdCounter}`;
}

export type ChatEventType =
  | 'message'
  | 'tool_start'
  | 'tool_end'
  | 'error'
  | 'abort';

export interface ChatEvent {
  type: ChatEventType;
  message?: ChatMessage;
  toolName?: string;
  toolResult?: ToolResult;
  error?: string;
}

export type ChatEventListener = (event: ChatEvent) => void;

export class ChatEngine {
  private messages: ChatMessage[] = [];
  private provider: LLMProvider | null = null;
  private accessor: ViewerStateAccessor;
  private graphSummary: GraphSummary | null = null;
  private listeners: ChatEventListener[] = [];
  private abortController: AbortController | null = null;
  private _busy = false;

  constructor(accessor: ViewerStateAccessor) {
    this.accessor = accessor;
  }

  get busy(): boolean {
    return this._busy;
  }

  setProvider(provider: LLMProvider | null): void {
    this.provider = provider;
  }

  getProvider(): LLMProvider | null {
    return this.provider;
  }

  on(listener: ChatEventListener): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter(l => l !== listener);
    };
  }

  private emit(event: ChatEvent): void {
    for (const listener of this.listeners) {
      try { listener(event); } catch { /* listener errors must not break the engine */ }
    }
  }

  /** Refresh graph summary (call after graph load or reload). */
  refreshSummary(): void {
    const graph = this.accessor.getFullGraph();
    this.graphSummary = graph ? buildGraphSummary(graph) : null;
  }

  getGraphSummary(): GraphSummary | null {
    return this.graphSummary;
  }

  getMessages(): ChatMessage[] {
    return [...this.messages];
  }

  clearHistory(): void {
    this.messages = [];
  }

  abort(): void {
    this.abortController?.abort();
    this.abortController = null;
    this._busy = false;
    this.emit({ type: 'abort' });
  }

  /**
   * Send a user message and get an assistant response (with potential tool-call loop).
   */
  async sendMessage(content: string): Promise<void> {
    if (!this.provider) {
      this.emit({ type: 'error', error: 'No LLM provider configured' });
      return;
    }
    if (this._busy) {
      this.emit({ type: 'error', error: 'Chat engine is busy' });
      return;
    }

    this._busy = true;
    this.abortController = new AbortController();

    const userMsg: ChatMessage = {
      id: nextId(),
      role: 'user',
      content,
      timestamp: Date.now(),
    };
    this.messages.push(userMsg);
    this.emit({ type: 'message', message: userMsg });

    try {
      await this.runAssistantLoop();
    } catch (err) {
      if ((err as Error).name === 'AbortError') {
        this.emit({ type: 'abort' });
      } else {
        const errorMsg = err instanceof Error ? err.message : String(err);
        this.emit({ type: 'error', error: errorMsg });
      }
    } finally {
      this._busy = false;
      this.abortController = null;
    }
  }

  private async runAssistantLoop(): Promise<void> {
    let toolCallCount = 0;

    while (toolCallCount < MAX_TOOL_CALLS_PER_TURN) {
      if (this.abortController?.signal.aborted) {
        throw new DOMException('Aborted', 'AbortError');
      }

      const apiMessages = this.buildApiMessages();
      const response = await this.provider!.chat(apiMessages, TOOL_DEFINITIONS);

      if (this.abortController?.signal.aborted) {
        throw new DOMException('Aborted', 'AbortError');
      }

      if (response.finishReason === 'error') {
        this.emit({ type: 'error', error: response.content || 'LLM returned an error' });
        return;
      }

      if (response.toolCalls && response.toolCalls.length > 0) {
        // Store assistant message with tool calls
        const assistantMsg: ChatMessage = {
          id: nextId(),
          role: 'assistant',
          content: response.content || '',
          toolCalls: response.toolCalls,
          timestamp: Date.now(),
        };
        this.messages.push(assistantMsg);
        if (response.content) {
          this.emit({ type: 'message', message: assistantMsg });
        }

        // Execute each tool call
        for (const tc of response.toolCalls) {
          toolCallCount++;
          this.emit({ type: 'tool_start', toolName: tc.name });

          const result = executeTool(tc, this.accessor);
          this.emit({ type: 'tool_end', toolName: tc.name, toolResult: result });

          const toolMsg: ChatMessage = {
            id: nextId(),
            role: 'tool',
            content: JSON.stringify(result),
            toolCallId: tc.id,
            timestamp: Date.now(),
          };
          this.messages.push(toolMsg);
        }

        // Continue loop to let the assistant respond to tool results
        continue;
      }

      // Final text response (no tool calls)
      const assistantMsg: ChatMessage = {
        id: nextId(),
        role: 'assistant',
        content: response.content,
        timestamp: Date.now(),
      };
      this.messages.push(assistantMsg);
      this.emit({ type: 'message', message: assistantMsg });
      return;
    }

    // Hit tool call limit -- force a text response by sending without tools
    const apiMessages = this.buildApiMessages();
    const response = await this.provider!.chat(apiMessages, []);
    const assistantMsg: ChatMessage = {
      id: nextId(),
      role: 'assistant',
      content: response.content || 'I reached the maximum number of actions for this turn. Please ask a follow-up question.',
      timestamp: Date.now(),
    };
    this.messages.push(assistantMsg);
    this.emit({ type: 'message', message: assistantMsg });
  }

  private buildApiMessages(): Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string; name?: string }> {
    // System prompt with fresh context
    const viewCtx = buildViewContext(this.accessor);
    const systemContent = this.graphSummary
      ? buildSystemPrompt(this.graphSummary, viewCtx)
      : 'You are an AI assistant for a call graph viewer. No graph is currently loaded.';

    const result: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string; name?: string }> = [
      { role: 'system', content: systemContent },
    ];

    // Apply sliding window to conversation history
    const history = this.messages.slice(-MAX_HISTORY_MESSAGES);

    for (const msg of history) {
      if (msg.role === 'user') {
        result.push({ role: 'user', content: msg.content });
      } else if (msg.role === 'assistant') {
        const entry: any = { role: 'assistant', content: msg.content || '' };
        if (msg.toolCalls && msg.toolCalls.length > 0) {
          entry.tool_calls = msg.toolCalls.map(tc => ({
            id: tc.id,
            type: 'function',
            function: { name: tc.name, arguments: JSON.stringify(tc.arguments) },
          }));
        }
        result.push(entry);
      } else if (msg.role === 'tool') {
        result.push({
          role: 'tool',
          content: msg.content,
          tool_call_id: msg.toolCallId,
          name: undefined,
        });
      }
    }

    return result;
  }
}
