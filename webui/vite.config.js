import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  base: './',
  build: {
    outDir: '../module/webroot',
  },
  plugins: [svelte()],
  optimizeDeps: {
    exclude: ['@material/web']
  }
})