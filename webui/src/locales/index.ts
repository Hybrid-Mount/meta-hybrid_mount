// SPDX-License-Identifier: Apache-2.0

import { createI18n, type PluralizationRule } from "vue-i18n";

type LocaleMessages = Record<string, unknown>;
type LocaleModule = { default: LocaleMessages };

const localeModules = import.meta.glob("./*.json", { eager: false });

let cachedLocales: { code: string; display: string }[] | null = null;

export const eastSlavicPluralRule: PluralizationRule = (
  choice,
  choicesLength,
  originalRule,
) => {
  if (choicesLength !== 4) {
    return originalRule?.(choice, choicesLength) ?? 0;
  }

  const count = Math.abs(choice);
  if (count === 0) return 0;

  const lastDigit = count % 10;
  const lastTwoDigits = count % 100;
  if (lastDigit === 1 && lastTwoDigits !== 11) return 1;
  if (lastDigit >= 2 && lastDigit <= 4 && (lastTwoDigits < 12 || lastTwoDigits > 14)) {
    return 2;
  }
  return 3;
};

export const pluralizationRules: Record<string, PluralizationRule> = {
  "ru-RU": eastSlavicPluralRule,
  "uk-UA": eastSlavicPluralRule,
};

const i18n = createI18n({
  legacy: false,
  locale: "en-US",
  fallbackLocale: "en-US",
  messages: {},
  pluralRules: pluralizationRules,
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
      const module = (await loader()) as LocaleModule;
      const lang = module.default.lang;
      const display =
        lang && typeof lang === "object" && "display" in lang
          ? (lang as { display?: unknown }).display
          : undefined;

      return { code, display: typeof display === "string" ? display : code };
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
  await loadLocale("en-US");
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
  localStorage.setItem("locale", defaultLocale);
}

export default i18n;
