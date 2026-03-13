/**
 * Anthropic BYOK provider.
 *
 * In local dev (Vite), requests are proxied through the Vite dev server
 * (/api/anthropic -> api.anthropic.com) so no external proxy is needed.
 * In production (GitHub Pages), requests go through a Cloudflare Worker proxy.
 */

import type { LLMProvider, ChatResponse, ToolDef, ToolCall } from '../types';

const STORAGE_KEY = 'scip-callgraph-anthropic-key';
const MODEL_KEY = 'scip-callgraph-anthropic-model';
const PERSIST_KEY = 'scip-callgraph-anthropic-persist';
const PROXY_URL_KEY = 'scip-callgraph-anthropic-proxy';

const DEFAULT_MODEL = 'claude-haiku-4-20250414';
const DEFAULT_PROXY_URL = 'https://anthropic-proxy.your-domain.workers.dev';

function isLocalDev(): boolean {
  return location.hostname === 'localhost' || location.hostname === '127.0.0.1';
}

function localProxyUrl(): string {
  return `${location.origin}/api/anthropic`;
}

export class AnthropicProvider implements LLMProvider {
  id = 'anthropic';
  name = 'Anthropic';

  async available(): Promise<boolean> {
    return !!this.getApiKey();
  }

  getApiKey(): string | null {
    return sessionStorage.getItem(STORAGE_KEY)
      || localStorage.getItem(STORAGE_KEY)
      || null;
  }

  setApiKey(key: string, persist: boolean): void {
    if (persist) {
      localStorage.setItem(STORAGE_KEY, key);
      localStorage.setItem(PERSIST_KEY, '1');
      sessionStorage.removeItem(STORAGE_KEY);
    } else {
      sessionStorage.setItem(STORAGE_KEY, key);
      localStorage.removeItem(STORAGE_KEY);
      localStorage.removeItem(PERSIST_KEY);
    }
  }

  clearApiKey(): void {
    sessionStorage.removeItem(STORAGE_KEY);
    localStorage.removeItem(STORAGE_KEY);
    localStorage.removeItem(PERSIST_KEY);
  }

  isPersisted(): boolean {
    return localStorage.getItem(PERSIST_KEY) === '1';
  }

  getModel(): string {
    return sessionStorage.getItem(MODEL_KEY)
      || localStorage.getItem(MODEL_KEY)
      || DEFAULT_MODEL;
  }

  setModel(model: string): void {
    sessionStorage.setItem(MODEL_KEY, model);
  }

  getProxyUrl(): string {
    return localStorage.getItem(PROXY_URL_KEY) || DEFAULT_PROXY_URL;
  }

  setProxyUrl(url: string): void {
    localStorage.setItem(PROXY_URL_KEY, url);
  }

  async chat(
    messages: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string; name?: string }>,
    tools: ToolDef[],
  ): Promise<ChatResponse> {
    const apiKey = this.getApiKey();
    if (!apiKey) {
      return { content: 'No Anthropic API key configured.', finishReason: 'error' };
    }

    // Convert from OpenAI-style messages to Anthropic format
    const systemMsg = messages.find(m => m.role === 'system');
    const nonSystemMessages = messages.filter(m => m.role !== 'system');

    const anthropicMessages = convertMessages(nonSystemMessages);
    const anthropicTools = tools.length > 0 ? tools.map(t => ({
      name: t.name,
      description: t.description,
      input_schema: t.parameters,
    })) : undefined;

    const body: Record<string, unknown> = {
      model: this.getModel(),
      max_tokens: 1024,
      messages: anthropicMessages,
    };

    if (systemMsg) {
      body.system = systemMsg.content;
    }

    if (anthropicTools && anthropicTools.length > 0) {
      body.tools = anthropicTools;
    }

    try {
      const useLocal = isLocalDev();
      const url = useLocal ? localProxyUrl() : this.getProxyUrl();
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };
      if (useLocal) {
        headers['x-api-key'] = apiKey;
        headers['anthropic-version'] = '2023-06-01';
      } else {
        headers['X-Anthropic-Key'] = apiKey;
      }

      const response = await fetch(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => null);
        const errorMsg = (errorData as any)?.error?.message || `HTTP ${response.status}`;
        return { content: `Anthropic API error: ${errorMsg}`, finishReason: 'error' };
      }

      const data = await response.json() as any;

      let textContent = '';
      const toolCalls: ToolCall[] = [];

      for (const block of data.content || []) {
        if (block.type === 'text') {
          textContent += block.text;
        } else if (block.type === 'tool_use') {
          toolCalls.push({
            id: block.id,
            name: block.name,
            arguments: block.input || {},
          });
        }
      }

      const finishReason: ChatResponse['finishReason'] =
        data.stop_reason === 'tool_use' ? 'tool_calls'
        : data.stop_reason === 'max_tokens' ? 'length'
        : 'stop';

      return {
        content: textContent,
        toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
        finishReason,
        usage: data.usage ? {
          promptTokens: data.usage.input_tokens,
          completionTokens: data.usage.output_tokens,
          totalTokens: (data.usage.input_tokens || 0) + (data.usage.output_tokens || 0),
        } : undefined,
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      return { content: `Network error: ${msg}`, finishReason: 'error' };
    }
  }
}

/**
 * Convert OpenAI-style messages to Anthropic message format.
 * Handles tool_calls (assistant) and tool results.
 */
function convertMessages(
  messages: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string }>,
): Array<Record<string, unknown>> {
  const result: Array<Record<string, unknown>> = [];

  for (const msg of messages) {
    if (msg.role === 'user') {
      result.push({ role: 'user', content: msg.content });
    } else if (msg.role === 'assistant') {
      const content: Array<Record<string, unknown>> = [];
      if (msg.content) {
        content.push({ type: 'text', text: msg.content });
      }
      if (msg.tool_calls) {
        for (const tc of msg.tool_calls as any[]) {
          content.push({
            type: 'tool_use',
            id: tc.id,
            name: tc.function?.name || tc.name,
            input: tc.function?.arguments
              ? (typeof tc.function.arguments === 'string'
                ? JSON.parse(tc.function.arguments)
                : tc.function.arguments)
              : tc.arguments || {},
          });
        }
      }
      result.push({ role: 'assistant', content });
    } else if (msg.role === 'tool') {
      result.push({
        role: 'user',
        content: [{
          type: 'tool_result',
          tool_use_id: msg.tool_call_id,
          content: msg.content,
        }],
      });
    }
  }

  return result;
}
