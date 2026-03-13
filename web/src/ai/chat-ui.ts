/**
 * Chat UI -- DOM rendering for the AI Guide panel.
 *
 * Handles:
 * - Tab switching between Node Details and AI Guide
 * - Static onboarding display (no LLM required)
 * - Chat message rendering
 * - API key setup
 * - Suggested query chips
 */

import type { ChatMessage, GraphSummary, SuggestedQuery, SuggestedAction, ViewerStateAccessor } from './types';
import type { ChatEngine, ChatEvent } from './chat-engine';
import { OpenAIProvider } from './providers/openai';
import { AnthropicProvider } from './providers/anthropic';
import { OpenRouterProvider } from './providers/openrouter';
import { formatSummaryText } from './static-analysis';

export class ChatUI {
  private engine: ChatEngine;
  private accessor: ViewerStateAccessor;
  private openaiProvider: OpenAIProvider;
  private anthropicProvider: AnthropicProvider;
  private openrouterProvider: OpenRouterProvider;
  private activeProviderId: string = 'openrouter';
  private unsubscribe: (() => void) | null = null;

  constructor(engine: ChatEngine, accessor: ViewerStateAccessor) {
    this.engine = engine;
    this.accessor = accessor;
    this.openaiProvider = new OpenAIProvider();
    this.anthropicProvider = new AnthropicProvider();
    this.openrouterProvider = new OpenRouterProvider();
    this.initEventListeners();
    this.syncProviderState();
  }

  private initEventListeners(): void {
    // Tab switching
    document.getElementById('tab-node-details')?.addEventListener('click', () => this.switchTab('node-details'));
    document.getElementById('tab-ai-guide')?.addEventListener('click', () => this.switchTab('ai-guide'));

    // API key setup
    document.getElementById('ai-key-save-btn')?.addEventListener('click', () => this.handleSaveKey());
    document.getElementById('ai-api-key-input')?.addEventListener('keydown', (e) => {
      if ((e as KeyboardEvent).key === 'Enter') this.handleSaveKey();
    });

    // Chat input
    document.getElementById('ai-chat-send')?.addEventListener('click', () => this.handleSendMessage());
    document.getElementById('ai-chat-input')?.addEventListener('keydown', (e) => {
      if ((e as KeyboardEvent).key === 'Enter' && !(e as KeyboardEvent).shiftKey) {
        e.preventDefault();
        this.handleSendMessage();
      }
    });

    // Stop button
    document.getElementById('ai-chat-stop')?.addEventListener('click', () => {
      this.engine.abort();
    });

    // Settings modal
    document.getElementById('ai-settings-btn')?.addEventListener('click', () => this.openSettings());
    document.getElementById('ai-settings-close')?.addEventListener('click', () => this.closeSettings());
    document.getElementById('ai-settings-save')?.addEventListener('click', () => this.saveSettings());
    document.getElementById('ai-settings-clear')?.addEventListener('click', () => this.clearSettings());
    document.getElementById('ai-settings-modal')?.addEventListener('click', (e) => {
      if ((e.target as HTMLElement).id === 'ai-settings-modal') this.closeSettings();
    });
    document.getElementById('ai-provider-select')?.addEventListener('change', (e) => {
      const val = (e.target as HTMLSelectElement).value;
      const proxySection = document.getElementById('ai-settings-proxy-section');
      if (proxySection) proxySection.style.display = (val === 'anthropic' || val === 'openrouter') ? '' : 'none';
      const keySection = document.getElementById('ai-settings-key-section');
      if (keySection) keySection.style.display = val === 'openrouter' ? 'none' : '';
    });

    // Chat engine events
    this.unsubscribe = this.engine.on((event) => this.handleChatEvent(event));
  }

  destroy(): void {
    this.unsubscribe?.();
  }

  // ============================================================================
  // Tab Switching
  // ============================================================================

  switchTab(tab: 'node-details' | 'ai-guide'): void {
    const nodeTab = document.getElementById('tab-node-details');
    const aiTab = document.getElementById('tab-ai-guide');
    const nodePanel = document.getElementById('panel-node-details');
    const aiPanel = document.getElementById('panel-ai-guide');

    if (!nodeTab || !aiTab || !nodePanel || !aiPanel) return;

    if (tab === 'node-details') {
      nodeTab.classList.add('active');
      aiTab.classList.remove('active');
      nodePanel.style.display = '';
      aiPanel.style.display = 'none';
    } else {
      nodeTab.classList.remove('active');
      aiTab.classList.add('active');
      nodePanel.style.display = 'none';
      aiPanel.style.display = '';
    }
  }

  // ============================================================================
  // Static Onboarding
  // ============================================================================

  renderStaticOnboarding(summary: GraphSummary): void {
    const summaryEl = document.getElementById('ai-onboarding-summary');
    const chipsEl = document.getElementById('ai-suggested-queries');
    if (!summaryEl || !chipsEl) return;

    summaryEl.textContent = formatSummaryText(summary);
    summaryEl.style.whiteSpace = 'pre-wrap';

    this.renderSuggestedQueries(summary.suggestedQueries, chipsEl);

    // If an LLM provider is available, trigger enhanced onboarding
    this.triggerLLMOnboarding(summary);
  }

  /**
   * Send graph summary to the LLM for a richer conversational welcome.
   * Runs in the background; the static onboarding is shown immediately as a fallback.
   */
  private async triggerLLMOnboarding(summary: GraphSummary): Promise<void> {
    if (!this.engine.getProvider()) return;
    if (this.engine.getMessages().length > 0) return; // Don't re-onboard

    const summaryText = formatSummaryText(summary);
    const onboardingPrompt = `A user just loaded a new graph. Here is the summary:\n\n${summaryText}\n\nGreet the user briefly and suggest 2-3 interesting starting points for exploration. Keep it to 3-4 sentences. Don't use markdown headers or bullet points -- be conversational.`;

    // Hide static onboarding once LLM starts responding
    const onboardingEl = document.getElementById('ai-static-onboarding');

    try {
      await this.engine.sendMessage(onboardingPrompt);
      // If the LLM responded, hide the static onboarding
      if (onboardingEl && this.engine.getMessages().length > 1) {
        onboardingEl.style.display = 'none';
      }
    } catch {
      // LLM failed; static onboarding remains visible
    }
  }

  private renderSuggestedQueries(queries: SuggestedQuery[], container: HTMLElement): void {
    container.innerHTML = '';
    for (const q of queries) {
      const chip = document.createElement('button');
      chip.className = 'ai-query-chip';
      chip.innerHTML = `<span class="chip-label">${escapeHtml(q.label)}</span><span class="chip-desc">${escapeHtml(q.description)}</span>`;
      chip.addEventListener('click', () => this.executeSuggestedAction(q.action, q.label));
      container.appendChild(chip);
    }
  }

  private executeSuggestedAction(action: SuggestedAction, label: string): void {
    // If the AI provider is available, send it as a chat message instead
    if (this.engine.getProvider()) {
      this.engine.sendMessage(label);
      return;
    }

    // Directional actions need unlimited depth so BFS finds all reachable nodes
    const needsUnlimitedDepth = action.type === 'setSource' || action.type === 'setSink'
      || action.type === 'setSourceAndSink' || action.type === 'setCrateBoundary';
    if (needsUnlimitedDepth) {
      this.accessor.setDepth(null);
    }

    switch (action.type) {
      case 'setSource':
        this.accessor.setSource(action.query);
        this.accessor.applyFiltersAndUpdate();
        break;
      case 'setSink':
        this.accessor.setSink(action.query);
        this.accessor.applyFiltersAndUpdate();
        break;
      case 'setSourceAndSink':
        this.accessor.setSource(action.source);
        this.accessor.setSink(action.sink);
        this.accessor.applyFiltersAndUpdate();
        break;
      case 'filterVerification':
        this.accessor.setFilters({
          showVerifiedNodes: action.statuses.includes('verified'),
          showFailedNodes: action.statuses.includes('failed'),
          showUnverifiedNodes: action.statuses.includes('unverified'),
        });
        this.accessor.applyFiltersAndUpdate();
        break;
      case 'setCrateBoundary':
        this.accessor.setSource(`crate:${action.source}`);
        this.accessor.setSink(`crate:${action.target}`);
        this.accessor.applyFiltersAndUpdate();
        break;
      case 'switchView':
        this.accessor.switchView(action.view);
        break;
    }

    // Show feedback and switch to the graph view so the user sees the result
    showToast(label);
    if (action.type !== 'switchView') {
      this.switchTab('node-details');
    }
  }

  // ============================================================================
  // Provider Setup
  // ============================================================================

  private syncProviderState(): void {
    const setupEl = document.getElementById('ai-provider-setup');
    const inputArea = document.getElementById('ai-chat-input-area');

    // OpenRouter is always available (proxy mode), so always show the chat input.
    if (inputArea) inputArea.style.display = '';

    // Show the optional BYOK section unless the user has explicitly chosen a BYOK provider
    const usingBYOK = this.activeProviderId !== 'openrouter';
    if (setupEl) setupEl.style.display = usingBYOK ? 'none' : '';

    this.engine.setProvider(this.resolveProvider());

    const persistEl = document.getElementById('ai-key-persist') as HTMLInputElement | null;
    if (persistEl) {
      const active = this.resolveActiveByokProvider();
      if (active) persistEl.checked = active.isPersisted();
    }
  }

  private resolveProvider() {
    switch (this.activeProviderId) {
      case 'anthropic': return this.anthropicProvider;
      case 'openai': return this.openaiProvider;
      default: return this.openrouterProvider;
    }
  }

  private resolveActiveByokProvider(): OpenAIProvider | AnthropicProvider | OpenRouterProvider | null {
    switch (this.activeProviderId) {
      case 'anthropic': return this.anthropicProvider;
      case 'openai': return this.openaiProvider;
      case 'openrouter': return this.openrouterProvider.isBYOK() ? this.openrouterProvider : null;
      default: return null;
    }
  }

  private handleSaveKey(): void {
    const input = document.getElementById('ai-api-key-input') as HTMLInputElement | null;
    const persist = (document.getElementById('ai-key-persist') as HTMLInputElement | null)?.checked ?? false;
    const key = input?.value?.trim();

    if (!key) return;

    const isAnthropicKey = key.startsWith('sk-ant-');
    const providerLabel = isAnthropicKey ? 'Anthropic' : 'OpenAI';

    if (!sessionStorage.getItem('scip-callgraph-privacy-ack')) {
      const ack = window.confirm(
        `Privacy notice: Graph metadata (function names, file paths, crate names) will be sent to ${providerLabel}'s API. No source code is transmitted.\n\nContinue?`
      );
      if (!ack) return;
      sessionStorage.setItem('scip-callgraph-privacy-ack', '1');
    }

    if (isAnthropicKey) {
      this.anthropicProvider.setApiKey(key, persist);
      this.activeProviderId = 'anthropic';
    } else {
      this.openaiProvider.setApiKey(key, persist);
      this.activeProviderId = 'openai';
    }
    this.syncProviderState();

    if (input) input.value = '';

    this.setStatus(`${providerLabel} key saved. Switching provider.`);
    setTimeout(() => this.setStatus(''), 3000);

    const summary = this.engine.getGraphSummary();
    if (summary) this.triggerLLMOnboarding(summary);
  }

  // ============================================================================
  // Chat Messages
  // ============================================================================

  private handleSendMessage(): void {
    const input = document.getElementById('ai-chat-input') as HTMLInputElement | null;
    const content = input?.value?.trim();
    if (!content) return;
    if (input) input.value = '';

    // Hide static onboarding once chat starts
    const onboarding = document.getElementById('ai-static-onboarding');
    if (onboarding) onboarding.style.display = 'none';

    this.engine.sendMessage(content);
  }

  private handleChatEvent(event: ChatEvent): void {
    switch (event.type) {
      case 'message':
        if (event.message) this.appendMessage(event.message);
        break;
      case 'tool_start':
        this.setStatus(`Running: ${event.toolName}...`);
        this.showStopButton(true);
        break;
      case 'tool_end':
        this.appendToolAction(event.toolName!, event.toolResult!);
        this.setStatus('');
        if (event.toolResult?.success && event.toolName) {
          showToast(`AI: ${event.toolName}${event.toolResult.matchedNodes !== undefined ? ` (${event.toolResult.matchedNodes} nodes)` : ''}`);
        }
        break;
      case 'error':
        this.appendError(event.error || 'Unknown error');
        this.setStatus('');
        this.showStopButton(false);
        break;
      case 'abort':
        this.setStatus('Stopped.');
        this.showStopButton(false);
        setTimeout(() => this.setStatus(''), 2000);
        break;
    }

    if (event.type === 'message' && event.message?.role === 'assistant') {
      this.showStopButton(false);
    }
  }

  private appendMessage(msg: ChatMessage): void {
    const container = document.getElementById('ai-chat-messages');
    if (!container) return;

    const el = document.createElement('div');
    if (msg.role === 'user') {
      el.className = 'ai-msg ai-msg-user';
      el.textContent = msg.content;
    } else if (msg.role === 'assistant') {
      el.className = 'ai-msg ai-msg-assistant';
      el.textContent = msg.content;
    } else {
      return; // Don't render system/tool messages directly
    }

    container.appendChild(el);
    container.scrollTop = container.scrollHeight;
  }

  private appendToolAction(toolName: string, result: import('./types').ToolResult): void {
    const container = document.getElementById('ai-chat-messages');
    if (!container) return;

    const el = document.createElement('div');
    el.className = 'ai-msg-tool-action';

    let text = `Action: ${toolName}`;
    if (result.matchedNodes !== undefined) {
      text += ` (${result.matchedNodes} nodes)`;
    }
    if (!result.success && result.error) {
      text += ` -- ${result.error}`;
    }
    el.textContent = text;

    container.appendChild(el);
    container.scrollTop = container.scrollHeight;
  }

  private appendError(error: string): void {
    const container = document.getElementById('ai-chat-messages');
    if (!container) return;

    const el = document.createElement('div');
    el.className = 'ai-msg-error';
    el.textContent = `Error: ${error}`;
    container.appendChild(el);
    container.scrollTop = container.scrollHeight;
  }

  private setStatus(text: string): void {
    const el = document.getElementById('ai-chat-status');
    if (el) el.textContent = text;
  }

  private showStopButton(show: boolean): void {
    const el = document.getElementById('ai-chat-stop');
    if (el) el.style.display = show ? '' : 'none';
  }

  // ============================================================================
  // Settings Modal
  // ============================================================================

  private openSettings(): void {
    const modal = document.getElementById('ai-settings-modal');
    if (!modal) return;
    modal.style.display = '';

    const providerSelect = document.getElementById('ai-provider-select') as HTMLSelectElement | null;
    const keyInput = document.getElementById('ai-settings-key') as HTMLInputElement | null;
    const modelInput = document.getElementById('ai-settings-model') as HTMLInputElement | null;
    const proxyInput = document.getElementById('ai-settings-proxy-url') as HTMLInputElement | null;
    const persistCb = document.getElementById('ai-settings-persist') as HTMLInputElement | null;
    const proxySection = document.getElementById('ai-settings-proxy-section');
    const keySection = document.getElementById('ai-settings-key-section');

    if (providerSelect) providerSelect.value = this.activeProviderId;

    const isProxyProvider = this.activeProviderId === 'anthropic' || this.activeProviderId === 'openrouter';
    if (proxySection) proxySection.style.display = isProxyProvider ? '' : 'none';
    if (keySection) keySection.style.display = this.activeProviderId === 'openrouter' ? 'none' : '';

    if (this.activeProviderId === 'openrouter') {
      if (keyInput) { keyInput.value = ''; keyInput.placeholder = 'Not needed (using default proxy)'; }
      if (modelInput) modelInput.value = this.openrouterProvider.getModel();
      if (persistCb) persistCb.checked = false;
      if (proxyInput) proxyInput.value = this.openrouterProvider.getProxyUrl();
    } else if (this.activeProviderId === 'anthropic') {
      if (keyInput) { keyInput.value = ''; keyInput.placeholder = this.anthropicProvider.getApiKey() ? '••••••••' : 'API key...'; }
      if (modelInput) modelInput.value = this.anthropicProvider.getModel();
      if (persistCb) persistCb.checked = this.anthropicProvider.isPersisted();
      if (proxyInput) proxyInput.value = this.anthropicProvider.getProxyUrl();
    } else {
      if (keyInput) { keyInput.value = ''; keyInput.placeholder = this.openaiProvider.getApiKey() ? '••••••••' : 'API key...'; }
      if (modelInput) modelInput.value = this.openaiProvider.getModel();
      if (persistCb) persistCb.checked = this.openaiProvider.isPersisted();
    }
  }

  private closeSettings(): void {
    const modal = document.getElementById('ai-settings-modal');
    if (modal) modal.style.display = 'none';
  }

  private saveSettings(): void {
    const providerSelect = document.getElementById('ai-provider-select') as HTMLSelectElement | null;
    const keyInput = document.getElementById('ai-settings-key') as HTMLInputElement | null;
    const modelInput = document.getElementById('ai-settings-model') as HTMLInputElement | null;
    const proxyInput = document.getElementById('ai-settings-proxy-url') as HTMLInputElement | null;
    const persistCb = document.getElementById('ai-settings-persist') as HTMLInputElement | null;

    const providerId = providerSelect?.value || 'openrouter';
    const key = keyInput?.value?.trim();
    const model = modelInput?.value?.trim();
    const persist = persistCb?.checked ?? false;

    if (key && !sessionStorage.getItem('scip-callgraph-privacy-ack')) {
      const ack = window.confirm(
        'Privacy notice: Graph metadata (function names, file paths) will be sent to the AI provider. No source code is transmitted.\n\nContinue?'
      );
      if (!ack) return;
      sessionStorage.setItem('scip-callgraph-privacy-ack', '1');
    }

    if (providerId === 'openrouter') {
      if (model) this.openrouterProvider.setModel(model);
      const proxyUrl = proxyInput?.value?.trim();
      if (proxyUrl) this.openrouterProvider.setProxyUrl(proxyUrl);
    } else if (providerId === 'anthropic') {
      if (key) this.anthropicProvider.setApiKey(key, persist);
      if (model) this.anthropicProvider.setModel(model);
      const proxyUrl = proxyInput?.value?.trim();
      if (proxyUrl) this.anthropicProvider.setProxyUrl(proxyUrl);
    } else {
      if (key) this.openaiProvider.setApiKey(key, persist);
      if (model) this.openaiProvider.setModel(model);
    }

    this.activeProviderId = providerId;
    this.syncProviderState();
    this.closeSettings();

    const labels: Record<string, string> = { openrouter: 'OpenRouter (default)', anthropic: 'Anthropic', openai: 'OpenAI' };
    showToast(`${labels[providerId] || providerId} provider configured`);
  }

  private clearSettings(): void {
    const providerSelect = document.getElementById('ai-provider-select') as HTMLSelectElement | null;
    const providerId = providerSelect?.value || 'openrouter';

    if (providerId === 'anthropic') {
      this.anthropicProvider.clearApiKey();
    } else if (providerId === 'openai') {
      this.openaiProvider.clearApiKey();
    } else if (providerId === 'openrouter') {
      this.openrouterProvider.clearApiKey();
    }

    // Fall back to default OpenRouter proxy
    this.activeProviderId = 'openrouter';
    this.syncProviderState();
    this.closeSettings();
    showToast('Switched to default provider');
  }

}

function escapeHtml(str: string): string {
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/**
 * Show a brief toast notification above the graph.
 */
function showToast(message: string): void {
  const container = document.getElementById('ai-toast-container');
  if (!container) return;

  const toast = document.createElement('div');
  toast.className = 'ai-toast';
  toast.textContent = message;
  container.appendChild(toast);

  setTimeout(() => toast.remove(), 3000);
}
