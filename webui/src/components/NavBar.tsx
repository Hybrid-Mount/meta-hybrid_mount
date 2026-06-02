import { createEffect, For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { ICONS } from "../lib/constants";
import { ENABLE_KASUMI } from "../lib/constants_gen";
import "./NavBar.css";
import "@material/web/icon/icon.js";

interface Props {
  activeTab: string;
  onTabChange: (id: string) => void;
  tabs: readonly { id: string }[];
}

export default function NavBar(props: Props) {
  let navContainer: HTMLElement | undefined;
  const tabRefs: Record<string, HTMLButtonElement> = {};

  const iconMap: Record<string, { regular: string; filled: string }> = {
    status: { regular: ICONS.home, filled: ICONS.home_filled },
    config: { regular: ICONS.settings, filled: ICONS.settings_filled },
    ...(ENABLE_KASUMI
      ? { kasumi: { regular: ICONS.snowflake, filled: ICONS.snowflake_filled } }
      : {}),
    modules: { regular: ICONS.modules, filled: ICONS.modules_filled },
    info: { regular: ICONS.info, filled: ICONS.info_filled },
  };

  createEffect(() => {
    const active = props.activeTab;
    const tab = tabRefs[active];
    if (tab && navContainer) {
      const containerWidth = navContainer.clientWidth;
      const tabLeft = tab.offsetLeft;
      const tabWidth = tab.clientWidth;
      const scrollLeft = tabLeft - containerWidth / 2 + tabWidth / 2;
      navContainer.scrollTo({ left: scrollLeft, behavior: "smooth" });
    }
  });

  return (
    <nav class="bottom-nav" ref={navContainer}>
      <For each={props.tabs}>
        {(tab) => (
          <button
            class={`nav-tab ${props.activeTab === tab.id ? "active" : ""}`}
            onClick={() => props.onTabChange(tab.id)}
            ref={(el) => (tabRefs[tab.id] = el)}
            type="button"
            aria-current={props.activeTab === tab.id ? "page" : undefined}
          >
            <div class="icon-container">
              <md-icon>
                <svg viewBox="0 0 24 24">
                  <path
                    d={
                      props.activeTab === tab.id
                        ? iconMap[tab.id]?.filled
                        : iconMap[tab.id]?.regular || ICONS.description
                    }
                  />
                </svg>
              </md-icon>
            </div>
            <span class="label">
              {uiStore.L.tabs?.[tab.id as keyof typeof uiStore.L.tabs] ||
                tab.id}
            </span>
          </button>
        )}
      </For>
    </nav>
  );
}
