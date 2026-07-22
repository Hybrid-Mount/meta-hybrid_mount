/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { createSignal, createRoot } from "solid-js";
import type { KasumiStatus } from "../types";
import type { InitPayload } from "../api/contracts";
import { API } from "../api";
import { buildKasumiStatusFromPayload } from "../api/codec/runtimeCodec";
import { runtimeStateSchema } from "../api/schemas";

const STATUS_CACHE_TTL_MS = 3000;

const createKasumiStore = () => {
  const [status, setStatus] = createSignal<KasumiStatus | null>(null);
  const [loading, setLoading] = createSignal(false);
  let pendingLoad: Promise<void> | null = null;
  let hasLoaded = false;
  let lastLoadedAt = 0;

  function hasFreshStatus() {
    return hasLoaded && Date.now() - lastLoadedAt < STATUS_CACHE_TTL_MS;
  }

  function loadFromInit(payload: InitPayload) {
    if (!payload.kasumi_status) {
      throw new Error("init payload is missing kasumi_status");
    }
    setStatus(buildKasumiStatusFromPayload(payload.kasumi_status));
    hasLoaded = true;
    lastLoadedAt = Date.now();
  }

  async function loadStatus(force = false) {
    if (pendingLoad) return pendingLoad;
    if (!force && hasFreshStatus()) return Promise.resolve();

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const nextStatus = await API.getKasumiStatus();
        setStatus(nextStatus);
        hasLoaded = true;
        lastLoadedAt = Date.now();
      } finally {
        setLoading(false);
        pendingLoad = null;
      }
    })();

    return pendingLoad;
  }

  function ensureStatusLoaded() {
    return loadStatus(false);
  }

  function setEnabledOptimistic(enabled: boolean) {
    const current = status();
    if (!current) {
      throw new Error("Kasumi status must be loaded before it can be updated");
    }
    setStatus({
      ...current,
      config: {
        ...current.config,
        enabled,
      },
    });
    hasLoaded = true;
    lastLoadedAt = Date.now();
  }

  function handleSseUpdate(state: unknown) {
    const current = status();
    if (!current) {
      throw new Error("Kasumi status must be loaded before processing updates");
    }
    const next = runtimeStateSchema.parse(state);
    setStatus({
      ...current,
      runtime: {
        snapshot: next.kasumi,
        kasumi_modules: next.kasumi_modules,
        active_mounts: next.active_mounts,
      },
    });
    hasLoaded = true;
    lastLoadedAt = Date.now();
  }

  return {
    get status() {
      const value = status();
      if (!value) throw new Error("Kasumi status has not been initialized");
      return value;
    },
    get enabled() {
      const value = status();
      if (!value) throw new Error("Kasumi status has not been initialized");
      return value.config.enabled;
    },
    get loading() {
      return loading();
    },
    ensureStatusLoaded,
    loadFromInit,
    refreshStatus: (force = true) => loadStatus(force),
    setEnabledOptimistic,
    handleSseUpdate,
  };
};

export const kasumiStore = createRoot(createKasumiStore);
