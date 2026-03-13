/**
 * OpenAI BYOK (Bring Your Own Key) provider.
 *
 * Makes direct browser fetch calls to api.openai.com (CORS allowed).
 * API key stored in sessionStorage by default, with opt-in localStorage.
 */

import type { LLMProvider, ChatResponse, ToolDef, ToolCall } from '../types';

const STORAGE_KEY = 'scip-callgraph-openai-key';
const MODEL_KEY = 'scip-callgraph-openai-model';
const PERSIST_KEY = 'scip-callgraph-openai-persist';

const DEFAULT_MODEL = 'gpt-4o-mini';
const API_URL = 'https://api.openai.com/v1/chat/completions';

export class OpenAIProvider implements LLMProvider {
  id = 'openai';
  name = 'OpenAI';

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

  async chat(
    messages: Array<{ role: string; content: string; tool_calls?: unknown[]; tool_call_id?: string; name?: string }>,
    tools: ToolDef[],
  ): Promise<ChatResponse> {
    const apiKey = this.getApiKey();
    if (!apiKey) {
      return { content: 'No API key configured.', finishReason: 'error' };
    }

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
      const response = await fetch(API_URL, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${apiKey}`,
        },
        body: JSON.stringify(body),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => null);
        const errorMsg = errorData?.error?.message || `HTTP ${response.status}`;
        if (response.status === 401) {
          return { content: `Invalid API key: ${errorMsg}`, finishReason: 'error' };
        }
        return { content: `OpenAI API error: ${errorMsg}`, finishReason: 'error' };
      }

      const data = await response.json();
      const choice = data.choices?.[0];
      if (!choice) {
        return { content: 'No response from OpenAI.', finishReason: 'error' };
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
      const msg = err instanceof Error ? err.message : String(err);
      return { content: `Network error: ${msg}`, finishReason: 'error' };
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
