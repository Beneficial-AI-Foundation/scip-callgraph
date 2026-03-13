/**
 * Cloudflare Worker proxy for OpenRouter API.
 *
 * Holds the OpenRouter API key as a Workers Secret so it never reaches
 * the browser.  Adds per-IP rate limiting to prevent abuse.
 *
 * Secrets (set via `wrangler secret put`):
 *   OPENROUTER_API_KEY  -- your OpenRouter API key
 *
 * Optional env vars (wrangler.toml [vars]):
 *   ALLOWED_ORIGIN      -- CORS origin, e.g. "https://user.github.io"
 *   RATE_LIMIT_RPM      -- requests per minute per IP (default 12)
 *
 * Deploy: wrangler deploy -c proxy/openrouter-wrangler.toml
 */

const DEFAULT_RATE_LIMIT_RPM = 12;
const WINDOW_MS = 60_000;

const ipHits = new Map();

function isRateLimited(ip, maxRpm) {
  const now = Date.now();
  let entry = ipHits.get(ip);
  if (!entry || now - entry.windowStart > WINDOW_MS) {
    entry = { windowStart: now, count: 0 };
    ipHits.set(ip, entry);
  }
  entry.count++;
  return entry.count > maxRpm;
}

function corsHeaders(origin) {
  return {
    'Access-Control-Allow-Origin': origin,
    'Access-Control-Allow-Methods': 'POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
    'Access-Control-Max-Age': '86400',
  };
}

export default {
  async fetch(request, env) {
    const allowedOrigin = env.ALLOWED_ORIGIN || '*';
    const cors = corsHeaders(allowedOrigin);

    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: cors });
    }

    if (request.method !== 'POST') {
      return new Response('Method not allowed', { status: 405, headers: cors });
    }

    const apiKey = env.OPENROUTER_API_KEY;
    if (!apiKey) {
      return new Response(
        JSON.stringify({ error: 'Server misconfigured: missing API key secret' }),
        { status: 500, headers: { 'Content-Type': 'application/json', ...cors } },
      );
    }

    const ip = request.headers.get('CF-Connecting-IP') || 'unknown';
    const maxRpm = parseInt(env.RATE_LIMIT_RPM, 10) || DEFAULT_RATE_LIMIT_RPM;
    if (isRateLimited(ip, maxRpm)) {
      return new Response(
        JSON.stringify({ error: 'Rate limit exceeded. Please wait a moment.' }),
        { status: 429, headers: { 'Content-Type': 'application/json', ...cors } },
      );
    }

    const body = await request.text();

    const response = await fetch('https://openrouter.ai/api/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${apiKey}`,
        'HTTP-Referer': allowedOrigin !== '*' ? allowedOrigin : 'https://github.com/Beneficial-AI-Foundation/scip-callgraph',
        'X-Title': 'SCIP Call Graph Viewer',
      },
      body,
    });

    const responseBody = await response.text();
    return new Response(responseBody, {
      status: response.status,
      headers: { 'Content-Type': 'application/json', ...cors },
    });
  },
};
