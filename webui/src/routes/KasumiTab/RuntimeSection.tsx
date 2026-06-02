import { Show } from "solid-js";
import { ICONS } from "../../lib/constants";
import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { RuntimeSectionProps } from "./types";

export default function RuntimeSection(props: RuntimeSectionProps) {
  return (
    <SectionShell
      id="runtime"
      title={uiStore.L.kasumi?.runtimeTitle ?? "Runtime"}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
    >
      <div class="kasumi-config-grid">
        <button
          type="button"
          class={`kasumi-config-tile ${props.config?.enable_stealth ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () =>
                API.setKasumiStealth(!Boolean(props.config?.enable_stealth)),
              uiStore.L.kasumi?.stealthUpdated ?? "Stealth updated",
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.ghost} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi?.stealthTitle ?? "Stealth"}
          </span>
        </button>
        <button
          type="button"
          class={`kasumi-config-tile ${props.config?.enable_hidexattr ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () =>
                API.setKasumiHidexattr(
                  !Boolean(props.config?.enable_hidexattr),
                ),
              uiStore.L.kasumi?.hidexattrUpdated ?? "HideXattr updated",
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.visibility_off} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi?.hidexattrTitle ?? "HideXattr"}
          </span>
        </button>
        <button
          type="button"
          class={`kasumi-config-tile ${props.config?.enable_kernel_debug ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () =>
                API.setKasumiDebug(!Boolean(props.config?.enable_kernel_debug)),
              uiStore.L.kasumi?.kernelDebugUpdated ?? "Kernel debug updated",
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.bug} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi?.kernelDebugTitle ?? "Kernel Debug"}
          </span>
        </button>
        <button
          type="button"
          class={`kasumi-config-tile ${props.config?.enable_selinux_fix ? "active" : ""}`}
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () =>
                API.setKasumiSelinuxFix(
                  !Boolean(props.config?.enable_selinux_fix),
                ),
              uiStore.L.kasumi?.selinuxFixUpdated ?? "SELinux guard updated",
            )
          }
        >
          <md-ripple />
          <div class="kasumi-config-icon">
            <md-icon>
              <svg viewBox="0 0 24 24">
                <path d={ICONS.shield} />
              </svg>
            </md-icon>
          </div>
          <span class="kasumi-config-label">
            {uiStore.L.kasumi?.selinuxFixTitle ?? "SELinux Guard"}
          </span>
        </button>
      </div>
      <Show
        when={
          !props.status?.available &&
          props.status?.status !== "disabled" &&
          !props.lkm?.loaded
        }
      >
        <div class="runtime-note warning">
          {uiStore.L.kasumi?.lkmUnavailableHint ??
            "Kasumi is enabled, but the kernel module is not loaded yet."}
        </div>
      </Show>
    </SectionShell>
  );
}
