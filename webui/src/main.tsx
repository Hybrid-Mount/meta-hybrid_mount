/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { render } from 'solid-js/web';
import './init';
import App from './App.tsx';
import './app.css';
import './layout.css';

const root = document.getElementById('app');

if (root instanceof HTMLElement) {
  render(() => <App />, root);
}