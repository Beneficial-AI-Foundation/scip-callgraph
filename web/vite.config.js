import { defineConfig } from 'vite';

/**
 * Environment-based config for local dev vs EC2/reverse-proxy deployment.
 *
 * Set VITE_MODE=remote (or any truthy value) when deploying behind a reverse proxy
 * that serves this app at /graph/. Otherwise runs in local mode (base: '/', port 3000).
 *
 * Examples:
 *   npm run dev                    # Local: http://localhost:3000
 *   VITE_MODE=remote npm run dev   # Remote: origin set for proxy, base /graph/
 */
const isRemote = process.env.VITE_MODE === 'remote' || process.env.VITE_MODE === '1';
const port = process.env.VITE_PORT ? parseInt(process.env.VITE_PORT, 10) : (isRemote ? 3001 : 3000);

export default defineConfig({
  base: isRemote ? '/graph/' : '/',
  publicDir: 'public',
  build: {
    outDir: 'dist',
    rollupOptions: {
      input: { main: './index.html' },
    },
  },
  server: {
    port,
    host: true,  // Listen on 0.0.0.0 so reverse proxy can reach it
    open: !isRemote,
    ...(isRemote && {
      origin: process.env.VITE_ORIGIN || 'http://ec2-3-23-60-0.us-east-2.compute.amazonaws.com',
    }),
  },
});