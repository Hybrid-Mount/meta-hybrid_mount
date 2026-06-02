import { Show } from "solid-js";
import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { IdentitySectionProps } from "./types";

export default function IdentitySection(props: IdentitySectionProps) {
  return (
    <SectionShell
      id="identity"
      title={uiStore.L.kasumi?.identityTitle ?? "Identity Spoof"}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
      badge={
        props.unameMode === "global"
          ? (uiStore.L.kasumi?.unameModeGlobal ?? "Global")
          : (uiStore.L.kasumi?.unameModeScoped ?? "Scoped")
      }
      badgeActive={props.unameMode === "global"}
    >
      <div class="field-stack">
        <div class="uname-panel">
          <div class="uname-panel-head">
            <div>
              <div class="uname-panel-title">
                {uiStore.L.kasumi?.unameSpoofTitle ?? "Kernel version spoofing"}
              </div>
              <div class="uname-panel-subtitle">
                {props.release || props.version
                  ? `${props.release || "-"} · ${props.version || "-"}`
                  : (uiStore.L.kasumi?.unameEmptyHint ??
                    "No spoofed uname configured.")}
              </div>
            </div>
          </div>

          <div class="uname-field-grid">
            <md-outlined-text-field
              class="full-field kasumi-input-field"
              label={uiStore.L.kasumi?.unameRelease ?? "Version name"}
              value={props.release}
              supporting-text={
                uiStore.L.kasumi?.unameReleaseDesc ??
                "Kernel release, for example 5.15.0-generic."
              }
              onInput={(e: Event) =>
                props.setRelease((e.currentTarget as HTMLInputElement).value)
              }
              disabled={props.pending}
            />
            <md-outlined-text-field
              class="full-field kasumi-input-field"
              label={uiStore.L.kasumi?.unameVersion ?? "Build time"}
              value={props.version}
              supporting-text={
                uiStore.L.kasumi?.unameVersionDesc ??
                "Kernel version/build timestamp."
              }
              onInput={(e: Event) =>
                props.setVersion((e.currentTarget as HTMLInputElement).value)
              }
              disabled={props.pending}
            />
          </div>

          <div class="uname-mode-row">
            <div class="uname-segmented" role="radiogroup">
              <button
                type="button"
                class={props.unameMode === "scoped" ? "selected" : ""}
                disabled={props.pending}
                onClick={() => props.setUnameMode("scoped")}
              >
                {uiStore.L.kasumi?.unameModeScoped ?? "Scoped"}
              </button>
              <button
                type="button"
                class={props.unameMode === "global" ? "selected" : ""}
                disabled={props.pending}
                onClick={() => props.setUnameMode("global")}
              >
                {uiStore.L.kasumi?.unameModeGlobal ?? "Global"}
              </button>
            </div>
            <div class="uname-mode-desc">{props.unameModeDescription}</div>
          </div>

          <div class="button-row">
            <md-outlined-button
              disabled={props.pending}
              onClick={() => void props.fillOriginalKernelUname()}
            >
              {uiStore.L.kasumi?.fillOriginalKernel ??
                "Use current kernel info"}
            </md-outlined-button>
            <md-filled-button
              disabled={
                props.pending || !props.release.trim() || !props.version.trim()
              }
              onClick={() =>
                props.runAction(
                  props.saveAndApplyUname,
                  uiStore.L.kasumi?.applyUname ?? "Uname applied",
                )
              }
            >
              {uiStore.L.kasumi?.applyUname ?? "Apply Uname"}
            </md-filled-button>
            <md-outlined-button
              disabled={props.pending}
              onClick={() =>
                props.runAction(
                  props.clearUname,
                  uiStore.L.kasumi?.clearUname ?? "Uname cleared",
                )
              }
            >
              {uiStore.L.kasumi?.clearUname ?? "Clear Uname"}
            </md-outlined-button>
            <Show when={props.unameMode === "global"}>
              <md-outlined-button
                disabled={props.pending}
                onClick={() =>
                  props.runAction(
                    () => API.restoreKasumiUnameGlobal(),
                    uiStore.L.kasumi?.restoreUnameGlobal ??
                      "Original uname restored",
                  )
                }
              >
                {uiStore.L.kasumi?.restoreUnameGlobal ?? "Restore original"}
              </md-outlined-button>
            </Show>
          </div>
        </div>

        <md-outlined-text-field
          class="full-field kasumi-input-field"
          label={uiStore.L.kasumi?.cmdlineValue ?? "Cmdline Value"}
          value={props.cmdline}
          onInput={(e: Event) =>
            props.setCmdline((e.currentTarget as HTMLInputElement).value)
          }
          disabled={props.pending}
        />
        <div class="button-row">
          <md-filled-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.setKasumiCmdline(props.cmdline),
                uiStore.L.common?.saved ?? "Saved",
              )
            }
          >
            {uiStore.L.kasumi?.saveCmdline ?? "Save Cmdline"}
          </md-filled-button>
          <md-outlined-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.clearKasumiCmdline(),
                uiStore.L.kasumi?.clearCmdline ?? "Cmdline cleared",
              )
            }
          >
            {uiStore.L.kasumi?.clearCmdline ?? "Clear Cmdline"}
          </md-outlined-button>
        </div>
      </div>
    </SectionShell>
  );
}
