// SPDX-License-Identifier: Apache-2.0

import { createI18n } from "vue-i18n";

type LocaleModule = { default: Record<string, unknown> };

const localeModules = import.meta.glob("./*.json", { eager: false });

const LEGACY_LOCALE_ALIASES: Record<string, string> = {
  en: "en-US",
  zh: "zh-CN",
};

const LOCALE_FILE_ALIASES: Record<string, string> = {
  "en-US": "en",
  "zh-CN": "zh",
};

let cachedLocales: { code: string; display: string }[] | null = null;

const i18n = createI18n({
  legacy: false,
  locale: "en-US",
  fallbackLocale: "en-US",
  messages: {},
});

export function normalizeLocaleCode(locale: string): string {
  return LEGACY_LOCALE_ALIASES[locale] ?? locale;
}

export async function getSupportedLocales(): Promise<
  { code: string; display: string }[]
> {
  if (cachedLocales) return cachedLocales;

  const results = Object.keys(localeModules).map((path) => {
    const match = path.match(/\.\/(.+)\.json$/);
    if (!match) return null;
    const code = normalizeLocaleCode(match[1]);
    return { code, display: code };
  });

  cachedLocales = results
    .filter((item): item is { code: string; display: string } => item !== null)
    .sort((left, right) => left.code.localeCompare(right.code));
  return cachedLocales;
}

export async function loadLocale(locale: string): Promise<void> {
  const normalizedLocale = normalizeLocaleCode(locale);
  if (i18n.global.availableLocales.includes(normalizedLocale)) return;

  const fileCode = LOCALE_FILE_ALIASES[normalizedLocale] ?? normalizedLocale;
  const path = `./${fileCode}.json`;
  const loader = localeModules[path];
  if (!loader) {
    console.error(`Locale "${normalizedLocale}" not found`);
    return;
  }

  const module = (await loader()) as LocaleModule;
  i18n.global.setLocaleMessage(normalizedLocale, module.default);
}

export async function preloadFallbackLocale(): Promise<void> {
  await loadLocale("en-US");
}

export async function switchLocale(locale: string): Promise<void> {
  const normalizedLocale = normalizeLocaleCode(locale);
  await preloadFallbackLocale();
  await loadLocale(normalizedLocale);
  i18n.global.locale.value = normalizedLocale;
  localStorage.setItem("locale", normalizedLocale);
}

export async function initI18n(preferred?: string): Promise<void> {
  const locales = await getSupportedLocales();
  if (locales.length === 0) {
    console.error("No locale files found!");
    return;
  }

  await preloadFallbackLocale();

  const savedLocale = localStorage.getItem("locale");
  let defaultLocale = normalizeLocaleCode(preferred || savedLocale || locales[0].code);

  if (!locales.some((item) => item.code === defaultLocale)) {
    defaultLocale = locales[0].code;
  }

  await loadLocale(defaultLocale);
  i18n.global.locale.value = defaultLocale;
  localStorage.setItem("locale", defaultLocale);
}

export default i18n;
