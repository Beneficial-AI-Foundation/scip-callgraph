import { defineConfig } from 'vite';

export default defineConfig({
  root: '.',
  base: '/probegraph/',  // GitHub Pages subdirectory
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
  }
});

