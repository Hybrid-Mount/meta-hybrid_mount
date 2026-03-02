/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createEffect, For } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { ICONS } from "../lib/constants";
import "./NavBar.css";
import "@material/web/icon/icon.js";
import "@material/web/ripple/ripple.js";

interface Props {
  activeTab: string;
  onTabChange: (id: string) => void;
  tabs: { id: string }[];
}

export default function NavBar(props: Props) {
  let navContainer: HTMLElement | undefined;
  const tabRefs: Record<string, HTMLButtonElement> = {};

  const iconMap: Record<string, string> = {
    status: ICONS.home,
    config: ICONS.settings,
    modules: ICONS.modules,
    info: ICONS.info,
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
    <nav
      class="bottom-nav"
      ref={navContainer}
      style={{
        "padding-bottom": uiStore.fixBottomNav
          ? "48px"
          : "max(16px, env(safe-area-inset-bottom, 0px))",
      }}
    >
      <For each={props.tabs}>
        {(tab) => (
          <button
            class={`nav-tab ${props.activeTab === tab.id ? "active" : ""}`}
            onClick={() => props.onTabChange(tab.id)}
            ref={(el) => (tabRefs[tab.id] = el)}
            type="button"
          >
            <md-ripple></md-ripple>
            <div class="icon-container">
              <md-icon>
                <svg viewBox="0 0 24 24">
                  <path
                    d={iconMap[tab.id] || ICONS.description}
                    style={{ transition: "none" }}
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
