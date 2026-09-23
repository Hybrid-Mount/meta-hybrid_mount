// SPDX-License-Identifier: Apache-2.0
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { RuntimeAction } from "./types";
import { runtimeStore } from "./stores/runtimeStore";
import { uiStore } from "./stores/uiStore";

/** Shared behavior; each theme owns its controls and visual hierarchy. */
export function useRuntimeModule(moduleId: () => string) {
  const { t } = useI18n();
  const working = ref(false);
  const module = computed(() =>
    runtimeStore.status?.modules.find((item) => item.id === moduleId()),
  );
  const reason = computed(() => {
    if (runtimeStore.loadError) return runtimeStore.loadError;
    if (!runtimeStore.status?.supported)
      return runtimeStore.status?.reason || t("runtime.unavailable");
    if (!module.value?.eligible) return module.value?.reason || t("runtime.ineligible");
    return null;
  });
  const busy = computed(() => runtimeStore.busy);
  const loading = computed(() => runtimeStore.loading);
  const disabled = computed(() => busy.value || loading.value || Boolean(reason.value));
  const actionError = computed(() =>
    runtimeStore.actionError?.moduleId === moduleId()
      ? runtimeStore.actionError.message
      : null,
  );
  async function run(action: RuntimeAction): Promise<void> {
    if (disabled.value) return;
    working.value = true;
    try {
      if (await runtimeStore.runAction(moduleId(), action))
        uiStore.showToast(t("runtime.success"));
    } finally {
      working.value = false;
    }
  }
  return { module, reason, disabled, actionError, busy, loading, working, run };
}
