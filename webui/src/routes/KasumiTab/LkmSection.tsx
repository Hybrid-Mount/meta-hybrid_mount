import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { LkmSectionProps } from "./types";

export default function LkmSection(props: LkmSectionProps) {
  const autoloadText = props.lkm?.autoload
    ? (uiStore.L.kasumi?.autoloadOn ?? "Autoload On")
    : (uiStore.L.kasumi?.autoloadOff ?? "Autoload Off");

  return (
    <SectionShell
      id="lkm"
      title={uiStore.L.kasumi?.lkmTitle ?? "Kernel Module"}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
      badge={autoloadText}
      badgeActive={Boolean(props.lkm?.autoload)}
    >
      <div class="meta-list">
        <div class="meta-row">
          <span>{uiStore.L.kasumi?.currentKmi ?? "Current KMI"}</span>
          <strong>{props.lkm?.current_kmi || "-"}</strong>
        </div>
      </div>
      <div class="field-row">
        <button
          class="kasumi-select-button"
          type="button"
          disabled={props.pending}
          onClick={props.onShowKmiDialog}
        >
          <div class="kasumi-select-button-label">
            {uiStore.L.kasumi?.kmiOverride ?? "KMI Override"}
          </div>
          <div class="kasumi-select-button-value">
            {props.kmi || (uiStore.L.kasumi?.autoKmi ?? "Auto Detect")}
          </div>
        </button>
      </div>
      <div class="button-row">
        <md-filled-button
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiLkmKmi(props.kmi),
              uiStore.L.kasumi?.saveKmi ?? "KMI saved",
            )
          }
        >
          {uiStore.L.kasumi?.saveKmi ?? "Save KMI"}
        </md-filled-button>
      </div>
      <div class="button-row">
        <md-outlined-button
          disabled={props.pending}
          onClick={() =>
            props.runAction(
              () => API.setKasumiLkmAutoload(!Boolean(props.lkm?.autoload)),
              uiStore.L.kasumi?.autoloadUpdated ?? "Autoload updated",
            )
          }
        >
          {props.lkm?.autoload
            ? (uiStore.L.kasumi?.disableAutoload ?? "Disable autoload")
            : (uiStore.L.kasumi?.enableAutoload ?? "Enable autoload")}
        </md-outlined-button>
        <md-filled-button
          disabled={props.pending}
          onClick={() =>
            props.lkm?.loaded
              ? props.onShowUnloadWarning()
              : props.runAction(
                  () => API.loadKasumiLkm(),
                  uiStore.L.kasumi?.loadLkm ?? "LKM loaded",
                )
          }
        >
          {props.lkm?.loaded
            ? (uiStore.L.kasumi?.unloadLkm ?? "Unload LKM")
            : (uiStore.L.kasumi?.loadLkm ?? "Load LKM")}
        </md-filled-button>
      </div>
    </SectionShell>
  );
}
