/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { mount } from 'svelte'
import App from './App.svelte'

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app