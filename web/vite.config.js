import { defineConfig } from 'vite';

export default defineConfig({
  root: '.',
  base: '/scip-callgraph/',  // GitHub Pages subdirectory
  publicDir: 'public',
  build: {
    outDir: 'dist',
    rollupOptions: {
      input: {
        main: './index.html'
      }
    }
  },
  server: {
    port: 3000,
    open: true,
    proxy: {
      '/api/anthropic': {
        target: 'https://api.anthropic.com',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/anthropic/, '/v1/messages'),
        headers: {
          'anthropic-version': '2023-06-01',
        },
      },
    },
  }
});

