import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
export default defineConfig({
  plugins: [svelte()], base: './', clearScreen: false,
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: { cssCodeSplit: false, rollupOptions: { output: { entryFileNames: 'app.js', assetFileNames: asset => asset.name?.endsWith('.css') ? 'style.css' : 'assets/[name]-[hash][extname]', inlineDynamicImports: true } } }
});
