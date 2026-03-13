/**
 * OpenRouter provider with two modes:
 *
 * 1. **Default (proxy)** -- requests go through a Cloudflare Worker that
 *    holds the API key as a secret.  No key needed from the user.
 * 2. **BYOK** -- user provides their own OpenRouter key, requests go
 *    directly to openrouter.ai (CORS-friendly).
 *
 * OpenRouter exposes an OpenAI-compatible chat completions endpoint,
 * so the request/response format mirrors the OpenAI provider.
 */

import type { LLMProvider, ChatResponse, ToolDef, ToolCall } from '../types';

const STORAGE_KEY = 'scip-callgraph-openrouter-key';
const MODEL_KEY = 'scip-callgraph-openrouter-model';
const PERSIST_KEY = 'scip-callgraph-openrouter-persist';
const PROXY_URL_KEY = 'scip-callgraph-openrouter-proxy';

const DEFAULT_MODEL = 'anthropic/claude-3.5-haiku';
const DIRECT_API_URL = 'https://openrouter.ai/api/v1/chat/completions';
const DEFAULT_PROXY_URL = 'https://openrouter-proxy.baif.workers.dev';

function isLocalDev(): boolean {
  return location.hostname === 'localhost' || location.hostname === '127.0.0.1';
}

function localProxyUrl(): string {
  return `${location.origin}/api/openrouter`;
}

export class OpenRouterProvider implements LLMProvider {
  id = 'openrouter';
  name = 'OpenRouter';

  /**
   * Always available -- in default mode it uses the proxy (no key needed).
   * If the user has set a BYOK key, that takes priority.
   */
  async available(): Promise<boolean> {
    return true;
  }

  isBYOK(): boolean {
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
    const useProxy = !apiKey;

    const body: Record<string, unknown> = {
      model: this.getModel(),
      messages: messages.map(m => {
        const msg: Record<string, unknown> = { role: m.role, content: m.content };
        if (m.tool_calls) msg.tool_calls = m.tool_calls;
        if (m.tool_call_id) msg.tool_call_id = m.tool_call_id;
        if (m.name) msg.name = m.name;
        return msg;
      }),
    };

    if (tools.length > 0) {
      body.tools = tools.map(t => ({
        type: 'function',
        function: {
          name: t.name,
          description: t.description,
          parameters: t.parameters,
        },
      }));
      body.tool_choice = 'auto';
    }

    try {
      let url: string;
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };

      if (useProxy) {
        // In local dev, route through Vite's proxy (which forwards to the Cloudflare
        // Worker server-side, avoiding CORS). In production, go to the Worker directly.
        url = isLocalDev() ? localProxyUrl() : this.getProxyUrl();
      } else {
        url = DIRECT_API_URL;
        headers['Authorization'] = `Bearer ${apiKey}`;
        headers['HTTP-Referer'] = location.origin;
        headers['X-Title'] = 'SCIP Call Graph Viewer';
      }

      const response = await fetch(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => null);
        const errorMsg = (errorData as any)?.error?.message
          || (errorData as any)?.error
          || `HTTP ${response.status}`;
        if (response.status === 429) {
          return { content: `Rate limit reached -- please wait a moment and try again.`, finishReason: 'error' };
        }
        if (response.status === 401) {
          return { content: `Invalid API key: ${errorMsg}`, finishReason: 'error' };
        }
        return { content: `OpenRouter API error: ${errorMsg}`, finishReason: 'error' };
      }

      const data = await response.json() as any;
      const choice = data.choices?.[0];
      if (!choice) {
        return { content: 'No response from OpenRouter.', finishReason: 'error' };
      }

      const msg = choice.message;
      const toolCalls: ToolCall[] | undefined = msg.tool_calls?.map((tc: any) => ({
        id: tc.id,
        name: tc.function.name,
        arguments: parseToolArgs(tc.function.arguments),
      }));

      let finishReason: ChatResponse['finishReason'] = 'stop';
      if (choice.finish_reason === 'tool_calls') finishReason = 'tool_calls';
      else if (choice.finish_reason === 'length') finishReason = 'length';
      else if (toolCalls && toolCalls.length > 0) finishReason = 'tool_calls';

      return {
        content: msg.content || '',
        toolCalls,
        finishReason,
        usage: data.usage ? {
          promptTokens: data.usage.prompt_tokens,
          completionTokens: data.usage.completion_tokens,
          totalTokens: data.usage.total_tokens,
        } : undefined,
      };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      return { content: `Network error: ${errorMsg}`, finishReason: 'error' };
    }
  }
}

function parseToolArgs(argsStr: string): Record<string, unknown> {
  try {
    return JSON.parse(argsStr);
  } catch {
    return { _raw: argsStr };
  }
}
