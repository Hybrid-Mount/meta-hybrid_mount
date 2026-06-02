import { createSignal, createRoot } from "solid-js";
import type { KasumiStatus } from "../types";
import type { InitPayload } from "../api/contracts";
import { API } from "../api";
import { DEFAULT_CONFIG } from "../constants";
import { buildKasumiStatusFromPayload } from "../api/codec/runtimeCodec";
import { uiStore } from "./uiStore";

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
    if (payload.kasumi_status != null) {
      const s = buildKasumiStatusFromPayload(
        payload.kasumi_status,
        DEFAULT_CONFIG.kasumi,
        {},
      );
      if (s) {
        setStatus(s);
        hasLoaded = true;
        lastLoadedAt = Date.now();
      } else {
        console.warn("kasumiStore: failed to parse init kasumi_status payload");
      }
    } else {
      console.warn("kasumiStore: init payload missing kasumi_status");
    }
  }

  async function loadStatus(showError = true, force = false) {
    if (pendingLoad) return pendingLoad;
    if (!force && hasFreshStatus()) return Promise.resolve();

    setLoading(true);
    pendingLoad = (async () => {
      try {
        const nextStatus = await API.getKasumiStatus();
        setStatus(nextStatus);
        hasLoaded = true;
        lastLoadedAt = Date.now();
      } catch (_e) {
        setStatus(null);
        if (showError) {
          uiStore.showToast(
            uiStore.L.kasumi?.loadError || "Failed to load Kasumi status",
            "error",
          );
        }
      } finally {
        setLoading(false);
        pendingLoad = null;
      }
    })();

    return pendingLoad;
  }

  function ensureStatusLoaded() {
    return loadStatus(false, false);
  }

  function setEnabledOptimistic(enabled: boolean) {
    const current = status();
    if (!current) {
      setStatus({
        status: "unknown",
        available: false,
        kernel_supported: false,
        protocol_version: null,
        feature_names: [],
        hooks: [],
        rule_count: 0,
        user_hide_rule_count: 0,
        mirror_path: "/dev/kasumi_mirror",
        lkm: { loaded: false, autoload: false, kmi_override: "" },
        config: { ...DEFAULT_CONFIG.kasumi, enabled },
        runtime: { snapshot: {}, kasumi_modules: [] },
      });
      hasLoaded = true;
      lastLoadedAt = Date.now();
      return;
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
    if (!current) return;
    const s = state as Record<string, unknown> | null;
    if (!s || typeof s !== "object") return;
    const kasumi = s.kasumi as Record<string, unknown> | null;
    const kasumiModules = Array.isArray(s.kasumi_modules)
      ? (s.kasumi_modules as string[])
      : [];
    setStatus({
      ...current,
      runtime: {
        ...current.runtime,
        snapshot: kasumi ?? current.runtime?.snapshot ?? {},
        kasumi_modules: kasumiModules,
      },
    });
    hasLoaded = true;
    lastLoadedAt = Date.now();
  }

  return {
    get status() {
      return status();
    },
    get enabled() {
      return Boolean(status()?.config?.enabled);
    },
    get loading() {
      return loading();
    },
    ensureStatusLoaded,
    loadFromInit,
    refreshStatus: (showError = true, force = true) =>
      loadStatus(showError, force),
    setEnabledOptimistic,
    handleSseUpdate,
  };
};

export const kasumiStore = createRoot(createKasumiStore);
