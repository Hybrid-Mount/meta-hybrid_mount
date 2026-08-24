// SPDX-License-Identifier: Apache-2.0

import { ref } from "vue";
import { toast } from "kernelsu";
import { getSupportedLocales, normalizeLocaleCode, switchLocale } from "../../locales";
import type { UiStyle } from "../types";

const lang = ref("en-US");
const isReady = ref(false);
const uiStyle = ref<UiStyle>("md3");
const monetEnabled = ref(false);
const navindex = ref(0);

const availableLanguages = ref<{ code: string; display: string }[]>([]);
let toastHandler: (text: string) => void = toast;

async function fetchAvailableLanguages(): Promise<void> {
  availableLanguages.value = await getSupportedLocales();
}

function showToast(text: string): void {
  toastHandler(text);
}

function setToastHandler(handler?: (text: string) => void): void {
  toastHandler = handler ?? toast;
}

async function setLang(code: string): Promise<void> {
  const normalizedCode = normalizeLocaleCode(code);
  lang.value = normalizedCode;
  await switchLocale(normalizedCode);
}

function setUiStyle(style: UiStyle): void {
  uiStyle.value = style;
  localStorage.setItem("uiStyle", style);
  document.documentElement.classList.toggle(
    "miuix-monet",
    style === "miuix" && monetEnabled.value,
  );
}

function setNavindex(index: number): void {
  navindex.value = index;
}

function setMonetEnabled(enabled: boolean): void {
  monetEnabled.value = enabled;
  localStorage.setItem("monetEnabled", enabled ? "1" : "0");
  document.documentElement.classList.toggle(
    "miuix-monet",
    enabled && uiStyle.value === "miuix",
  );
}

async function init(): Promise<void> {
  await fetchAvailableLanguages();
  const requestedLang = normalizeLocaleCode(localStorage.getItem("locale") ?? "en-US");
  const savedLang = availableLanguages.value.some(
    (language) => language.code === requestedLang,
  )
    ? requestedLang
    : (availableLanguages.value[0]?.code ?? "en-US");
  await switchLocale(savedLang);
  lang.value = savedLang;

  const savedStyle = localStorage.getItem("uiStyle");
  if (savedStyle === "miuix" || savedStyle === "md3") {
    uiStyle.value = savedStyle;
  }

  const savedMonet = localStorage.getItem("monetEnabled");
  monetEnabled.value = savedMonet === "1";
  if (monetEnabled.value && uiStyle.value === "miuix") {
    document.documentElement.classList.add("miuix-monet");
  }
  isReady.value = true;
}

export const uiStore = {
  get lang() {
    return lang.value;
  },
  get availableLanguages() {
    return availableLanguages.value;
  },
  get isReady() {
    return isReady.value;
  },
  get uiStyle() {
    return uiStyle.value;
  },
  get monetEnabled() {
    return monetEnabled.value;
  },
  get navindex() {
    return navindex.value;
  },
  showToast,
  setToastHandler,
  setLang,
  setUiStyle,
  setNavindex,
  setMonetEnabled,
  init,
  fetchAvailableLanguages,
};
