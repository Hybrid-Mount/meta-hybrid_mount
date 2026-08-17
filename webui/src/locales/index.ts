// SPDX-License-Identifier: Apache-2.0

import { createI18n } from "vue-i18n";

type LocaleModule = { default: Record<string, unknown> };

const localeModules = import.meta.glob("./*.json", { eager: false });

let cachedLocales: { code: string; display: string }[] | null = null;

const i18n = createI18n({
  legacy: false,
  locale: "en",
  fallbackLocale: "en",
  messages: {},
});

export async function getSupportedLocales(): Promise<
  { code: string; display: string }[]
> {
  if (cachedLocales) return cachedLocales;

  const results = await Promise.all(
    Object.entries(localeModules).map(async ([path, loader]) => {
      const match = path.match(/\.\/(.+)\.json$/);
      if (!match) return null;
      const code = match[1];
      const mod = (await loader()) as LocaleModule;
      const messages = mod.default as { lang?: { display?: string } };
      return { code, display: messages.lang?.display || code };
    }),
  );

  cachedLocales = results
    .filter((item): item is { code: string; display: string } => item !== null)
    .sort((left, right) => left.code.localeCompare(right.code));
  return cachedLocales;
}

export async function loadLocale(locale: string): Promise<void> {
  if (i18n.global.availableLocales.includes(locale)) return;

  const path = `./${locale}.json`;
  const loader = localeModules[path];
  if (!loader) {
    console.error(`Locale "${locale}" not found`);
    return;
  }

  const module = (await loader()) as LocaleModule;
  i18n.global.setLocaleMessage(locale, module.default);
}

export async function preloadFallbackLocale(): Promise<void> {
  await loadLocale("en");
}

export async function switchLocale(locale: string): Promise<void> {
  await preloadFallbackLocale();
  await loadLocale(locale);
  i18n.global.locale.value = locale;
  localStorage.setItem("locale", locale);
}

export async function initI18n(preferred?: string): Promise<void> {
  const locales = await getSupportedLocales();
  if (locales.length === 0) {
    console.error("No locale files found!");
    return;
  }

  await preloadFallbackLocale();

  const savedLocale = localStorage.getItem("locale");
  let defaultLocale = preferred || savedLocale || locales[0].code;

  if (!locales.some((item) => item.code === defaultLocale)) {
    defaultLocale = locales[0].code;
  }

  await loadLocale(defaultLocale);
  i18n.global.locale.value = defaultLocale;
}

export default i18n;
