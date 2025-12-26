/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

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