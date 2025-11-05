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
    open: true
  }
});

