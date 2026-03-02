/**
 * Copyright 2026 Hybrid Mount Developers
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, createRoot } from "solid-js";
import type { ToastMessage, LanguageOption } from "../types";

const localeModules = import.meta.glob("../../locales/*.json");

const createUiStore = () => {
  const [lang, setLangSignal] = createSignal("en-US");
  const [loadedLocale, setLoadedLocale] = createSignal<any>(null);
  const [toast, setToast] = createSignal<ToastMessage>({
    id: "init",
    text: "",
    type: "info",
    visible: false,
  });
  const [fixBottomNav, setFixBottomNavSignal] = createSignal(false);
  const [isReady, setIsReady] = createSignal(false);

  const availableLanguages: LanguageOption[] = [
    { code: "en-US", name: "English" },
    { code: "es-ES", name: "Español" },
    { code: "ja-JP", name: "日本語" },
    { code: "ru-RU", name: "Русский" },
    { code: "uk-UA", name: "Українська" },
    { code: "zh-CN", name: "简体中文" },
    { code: "zh-TW", name: "繁體中文" },
  ].sort((a, b) => {
    if (a.code === "en-US") return -1;
    if (b.code === "en-US") return 1;
    return a.name.localeCompare(b.name);
  });

  const L = createMemo(
    (): any =>
      (loadedLocale() as { default: any })?.default ||
      (loadedLocale() as any) ||
      {},
  );

  function showToast(
    text: string,
    type: "info" | "success" | "error" = "info",
  ) {
    const id = Date.now().toString();
    setToast({ id, text, type, visible: true });
    setTimeout(() => {
      if (toast().id === id) setToast((t) => ({ ...t, visible: false }));
    }, 3000);
  }

  async function loadLocale(code: string) {
    const match = Object.entries(localeModules).find(([path]) =>
      path.endsWith(`/${code}.json`),
    );
    if (match) {
      const mod = (await match[1]()) as any;
      setLoadedLocale(mod.default || mod);
    } else {
      const fallbackMatch = Object.entries(localeModules).find(([path]) =>
        path.endsWith(`/en-US.json`),
      );
      if (fallbackMatch) {
        const fallback = (await fallbackMatch[1]()) as any;
        setLoadedLocale(fallback.default || fallback);
      }
    }
  }

  function setLang(code: string) {
    setLangSignal(code);
    localStorage.setItem("lang", code);
    loadLocale(code);
  }

  function toggleBottomNavFix() {
    const newVal = !fixBottomNav();
    setFixBottomNavSignal(newVal);
    localStorage.setItem("hm_fix_bottom_nav", String(newVal));
    const dict = L();
    const msg = newVal
      ? dict.config?.fixBottomNavOn || "Bottom Nav Fix Enabled"
      : dict.config?.fixBottomNavOff || "Bottom Nav Fix Disabled";
    showToast(msg, "info");
  }

  async function init() {
    const savedLang = localStorage.getItem("lang") || "en-US";
    setLangSignal(savedLang);
    await loadLocale(savedLang);
    setFixBottomNavSignal(localStorage.getItem("hm_fix_bottom_nav") === "true");
    setIsReady(true);
  }

  return {
    get lang() {
      return lang();
    },
    get availableLanguages() {
      return availableLanguages;
    },
    get L() {
      return L();
    },
    get toast() {
      return toast();
    },
    get toasts() {
      return toast().visible ? [toast()] : [];
    },
    get fixBottomNav() {
      return fixBottomNav();
    },
    get isReady() {
      return isReady();
    },
    toggleBottomNavFix,
    showToast,
    setLang,
    init,
  };
};

export const uiStore = createRoot(createUiStore);
