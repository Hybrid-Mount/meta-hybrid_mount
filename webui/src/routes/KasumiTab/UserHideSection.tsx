import { For, Show } from "solid-js";
import { uiStore } from "../../lib/stores/uiStore";
import { API } from "../../lib/api";
import SectionShell from "./SectionShell";
import type { UserHideSectionProps } from "./types";

export default function UserHideSection(props: UserHideSectionProps) {
  return (
    <SectionShell
      id="user-hide"
      title={uiStore.L.kasumi?.userHideTitle ?? "User Hide Rules"}
      isExpanded={props.isExpanded}
      onToggle={props.onToggle}
    >
      <div class="field-stack">
        <md-outlined-text-field
          class="full-field kasumi-input-field"
          label={uiStore.L.kasumi?.userHidePathLabel ?? "Persistent Hide Path"}
          value={props.userHidePath}
          onInput={(e: Event) =>
            props.setUserHidePath((e.currentTarget as HTMLInputElement).value)
          }
          disabled={props.pending}
        />
        <div class="button-row">
          <md-filled-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => {
                  const path = props.userHidePath.trim();
                  if (!path) {
                    throw new Error(
                      uiStore.L.kasumi?.userHidePathRequired ??
                        "Hide path cannot be empty",
                    );
                  }
                  return API.addUserHideRule(path);
                },
                uiStore.L.kasumi?.hideRuleAdded ?? "Hide rule added",
                "full",
              )
            }
          >
            {uiStore.L.kasumi?.addHideRule ?? "Add Hide Rule"}
          </md-filled-button>
          <md-outlined-button
            disabled={props.pending}
            onClick={() =>
              props.runAction(
                () => API.applyUserHideRules(),
                uiStore.L.kasumi?.hideRulesApplied ?? "User hide rules applied",
                "full",
              )
            }
          >
            {uiStore.L.kasumi?.applyHideRules ?? "Apply Stored Hides"}
          </md-outlined-button>
        </div>
        <div class="hide-rule-list">
          <For each={props.userHideRules}>
            {(path) => (
              <div class="hide-rule-item">
                <span class="hide-rule-path mono">{path}</span>
                <button
                  class="hide-rule-remove"
                  type="button"
                  disabled={props.pending}
                  onClick={() =>
                    props.runAction(
                      () => API.removeUserHideRule(path),
                      uiStore.L.kasumi?.hideRuleRemoved ?? "Hide rule removed",
                      "full",
                    )
                  }
                >
                  {uiStore.L.kasumi?.removeHideRule ?? "Remove"}
                </button>
              </div>
            )}
          </For>
          <Show when={props.userHideRules.length === 0}>
            <div class="empty-inline-note">
              {uiStore.L.kasumi?.noUserHideRules ??
                "No persistent user hide rules yet."}
            </div>
          </Show>
        </div>
      </div>
    </SectionShell>
  );
}
