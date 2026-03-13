/**
 * Cloudflare Worker proxy for Anthropic API.
 *
 * Anthropic's API does not allow direct browser CORS requests.
 * This stateless proxy forwards requests with the user's API key
 * (passed as a header). No keys are stored.
 *
 * Deploy: wrangler deploy --name anthropic-proxy
 */

export default {
  async fetch(request) {
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: {
          'Access-Control-Allow-Origin': '*',
          'Access-Control-Allow-Methods': 'POST, OPTIONS',
          'Access-Control-Allow-Headers': 'Content-Type, X-Anthropic-Key',
          'Access-Control-Max-Age': '86400',
        },
      });
    }

    if (request.method !== 'POST') {
      return new Response('Method not allowed', { status: 405 });
    }

    const anthropicKey = request.headers.get('X-Anthropic-Key');
    if (!anthropicKey) {
      return new Response(JSON.stringify({ error: 'Missing X-Anthropic-Key header' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': '*' },
      });
    }

    const body = await request.text();
    const response = await fetch('https://api.anthropic.com/v1/messages', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': anthropicKey,
        'anthropic-version': '2023-06-01',
      },
      body,
    });

    const responseBody = await response.text();
    return new Response(responseBody, {
      status: response.status,
      headers: {
        'Content-Type': 'application/json',
        'Access-Control-Allow-Origin': '*',
      },
    });
  },
};
