/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { createSignal, createMemo, createRoot } from "solid-js";
import type { ToastMessage, LanguageOption } from "../types";
import enUS from "../../locales/en-US.json";

type Locale = typeof enUS;

const localeModules = import.meta.glob<{ default: unknown }>(
  "../../locales/*.json",
);

export function validateLocaleShape(
  reference: unknown,
  candidate: unknown,
  path: string,
): void {
  if (typeof reference === "string") {
    if (typeof candidate !== "string") {
      throw new Error(`Locale value must be a string: ${path}`);
    }
    return;
  }

  if (
    reference === null ||
    typeof reference !== "object" ||
    Array.isArray(reference) ||
    candidate === null ||
    typeof candidate !== "object" ||
    Array.isArray(candidate)
  ) {
    throw new Error(`Locale object has an invalid shape: ${path}`);
  }

  const referenceRecord = reference as Record<string, unknown>;
  const candidateRecord = candidate as Record<string, unknown>;
  const referenceKeys = Object.keys(referenceRecord);
  const candidateKeys = Object.keys(candidateRecord);

  if (
    referenceKeys.length !== candidateKeys.length ||
    candidateKeys.some((key) => !(key in referenceRecord))
  ) {
    throw new Error(`Locale keys do not match en-US: ${path}`);
  }

  for (const key of referenceKeys) {
    validateLocaleShape(
      referenceRecord[key],
      candidateRecord[key],
      `${path}.${key}`,
    );
  }
}

const createUiStore = () => {
  const [lang, setLangSignal] = createSignal("en-US");
  const [loadedLocale, setLoadedLocale] = createSignal<Locale>(enUS);
  const [toast, setToast] = createSignal<ToastMessage>({
    id: "init",
    text: "",
    type: "info",
    visible: false,
  });

  const availableLanguages: LanguageOption[] = [
    { code: "en-US", name: "English" },
    { code: "es-ES", name: "Español" },
    { code: "it-IT", name: "Italiano" },
    { code: "ja-JP", name: "日本語" },
    { code: "ru-RU", name: "Русский" },
    { code: "uk-UA", name: "Українська" },
    { code: "vi-VN", name: "Tiếng Việt" },
    { code: "id-ID", name: "Bahasa Indonesia" },
    { code: "zh-CN", name: "简体中文" },
    { code: "zh-TW", name: "繁體中文" },
  ].sort((a, b) => {
    if (a.code === "en-US") return -1;
    if (b.code === "en-US") return 1;
    return a.name.localeCompare(b.name);
  });

  const L = createMemo((): Locale => loadedLocale());

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

  async function loadLocale(code: string): Promise<Locale> {
    const loader = localeModules[`../../locales/${code}.json`];
    if (!loader) {
      throw new Error(`Unsupported locale: ${code}`);
    }
    const locale = (await loader()).default;
    validateLocaleShape(enUS, locale, code);
    return locale as Locale;
  }

  async function setLang(code: string) {
    const locale = await loadLocale(code);
    setLoadedLocale(locale);
    setLangSignal(code);
    localStorage.setItem("lang", code);
  }

  async function init() {
    const savedLang = localStorage.getItem("lang") ?? "en-US";
    const locale = await loadLocale(savedLang);
    setLoadedLocale(locale);
    setLangSignal(savedLang);
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
    showToast,
    setLang,
    init,
  };
};

export const uiStore = createRoot(createUiStore);
