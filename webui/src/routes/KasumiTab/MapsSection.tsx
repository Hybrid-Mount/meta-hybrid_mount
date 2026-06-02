import { For, Show } from "solid-js";
import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import { parseUnsignedInput } from "./utils";
import type { MapsSectionProps } from "./types";

export default function MapsSection(props: MapsSectionProps) {
  return (
    <SectionShell
      id="maps"
      title={uiStore.L.kasumi?.mapsTitle ?? "Maps Spoof Rules"}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
      badge={String(props.config?.maps_rules?.length ?? 0)}
    >
      <div class="field-stack">
        <div class="meta-list">
          <div class="meta-row">
            <span>{uiStore.L.kasumi?.mapsRuleCount ?? "Maps rules"}</span>
            <strong>{props.config?.maps_rules?.length ?? 0}</strong>
          </div>
        </div>
        <div class="sub-grid">
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi?.mapsTargetIno ?? "Target Inode"}
            value={props.mapsTargetIno}
            onInput={(e: Event) =>
              props.setMapsTargetIno(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi?.mapsTargetDev ?? "Target Device"}
            value={props.mapsTargetDev}
            onInput={(e: Event) =>
              props.setMapsTargetDev(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi?.mapsSpoofedIno ?? "Spoofed Inode"}
            value={props.mapsSpoofedIno}
            onInput={(e: Event) =>
              props.setMapsSpoofedIno(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
          <md-outlined-text-field
            class="full-field kasumi-input-field"
            label={uiStore.L.kasumi?.mapsSpoofedDev ?? "Spoofed Device"}
            value={props.mapsSpoofedDev}
            onInput={(e: Event) =>
              props.setMapsSpoofedDev(
                (e.currentTarget as HTMLInputElement).value,
              )
            }
            disabled={props.pending}
          />
        </div>
        <md-outlined-text-field
          class="full-field kasumi-input-field"
          label={uiStore.L.kasumi?.mapsSpoofedPath ?? "Spoofed Path"}
          value={props.mapsPath}
          onInput={(e: Event) =>
            props.setMapsPath((e.currentTarget as HTMLInputElement).value)
          }
          disabled={props.pending}
        />
        <div class="button-row">
          <md-filled-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(() => {
                const spoofedPath = props.mapsPath.trim();
                if (!spoofedPath) {
                  throw new Error(
                    uiStore.L.kasumi?.mapsPathRequired ??
                      "Spoofed path cannot be empty",
                  );
                }
                return API.addKasumiMapsRule({
                  target_ino: parseUnsignedInput(
                    props.mapsTargetIno,
                    "target inode",
                  ),
                  target_dev: parseUnsignedInput(
                    props.mapsTargetDev,
                    "target device",
                  ),
                  spoofed_ino: parseUnsignedInput(
                    props.mapsSpoofedIno,
                    "spoofed inode",
                  ),
                  spoofed_dev: parseUnsignedInput(
                    props.mapsSpoofedDev,
                    "spoofed device",
                  ),
                  spoofed_pathname: spoofedPath,
                });
              }, uiStore.L.kasumi?.mapsRuleAdded ?? "Maps spoof rule added")
            }
          >
            {uiStore.L.kasumi?.mapsAddRule ?? "Add Maps Rule"}
          </md-filled-button>
          <md-outlined-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.clearKasumiMapsRules(),
                uiStore.L.kasumi?.mapsCleared ?? "Maps rules cleared",
              )
            }
          >
            {uiStore.L.kasumi?.mapsClear ?? "Clear Maps Rules"}
          </md-outlined-button>
        </div>
        <div class="hide-rule-list">
          <For each={props.config?.maps_rules || []}>
            {(rule) => (
              <div class="hide-rule-item">
                <div class="hide-rule-path">
                  <div class="mono">{rule.spoofed_pathname}</div>
                  <div class="secondary-inline mono">
                    {(
                      uiStore.L.kasumi?.mapsRuleSummary ??
                      "target {target} -> spoof {spoofed}"
                    )
                      .replace(
                        "{target}",
                        `${rule.target_ino}:${rule.target_dev}`,
                      )
                      .replace(
                        "{spoofed}",
                        `${rule.spoofed_ino}:${rule.spoofed_dev}`,
                      )}
                  </div>
                </div>
              </div>
            )}
          </For>
          <Show when={(props.config?.maps_rules?.length || 0) === 0}>
            <div class="empty-inline-note">
              {uiStore.L.kasumi?.mapsEmpty ?? "No maps spoof rules configured."}
            </div>
          </Show>
        </div>
      </div>
    </SectionShell>
  );
}
