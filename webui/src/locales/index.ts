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

// src/locales/index.ts
import { createI18n } from "vue-i18n";

// 动态导入同目录下的所有 JSON 文件
const localeModules = import.meta.glob("./*.json", { eager: false });

// 缓存语言列表
let cachedLocales: { code: string; display: string }[] | null = null;

const i18n = createI18n({
  legacy: false,
  locale: "zh-CN",
  fallbackLocale: "en-US",
  messages: {},
});

// 获取支持的语言列表（从 JSON 文件读取 lang.display）
export async function getSupportedLocales() {
  if (cachedLocales) return cachedLocales;

  const results = await Promise.all(
    Object.entries(localeModules).map(async ([path, loader]) => {
      const match = path.match(/\.\/(.+)\.json$/);
      if (!match) return null;

      const code = match[1];
      const module = await loader();
      const messages = module.default;

      return {
        code,
        display: messages.lang?.display || code,
      };
    }),
  );

  cachedLocales = results.filter(Boolean) as {
    code: string;
    display: string;
  }[];
  return cachedLocales;
}

export async function getCurrentLangIndex(): Promise<number> {
  const locales = await getSupportedLocales();
  const currentLocale = i18n.global.locale.value;
  return locales.findIndex((lang) => lang.code === currentLocale);
}

// 加载单个语言包
export async function loadLocale(locale: string) {
  if (i18n.global.availableLocales.includes(locale)) return;

  const path = `./${locale}.json`;
  const loader = localeModules[path];
  if (!loader) {
    console.error(`Locale "${locale}" not found`);
    return;
  }

  const module = await loader();
  i18n.global.setLocaleMessage(locale, module.default);
}

// 切换语言
export async function switchLocale(locale: string) {
  await loadLocale(locale);
  i18n.global.locale.value = locale;
  localStorage.setItem("locale", locale);
}

// 初始化 i18n
export async function initI18n(preferred?: string) {
  const locales = await getSupportedLocales();
  if (locales.length === 0) {
    console.error("No locale files found!");
    return;
  }

  const savedLocale = localStorage.getItem("locale");
  let defaultLocale = preferred || savedLocale || locales[0].code;

  if (!locales.some((l) => l.code === defaultLocale)) {
    defaultLocale = locales[0].code;
  }

  await loadLocale(defaultLocale);
  i18n.global.locale.value = defaultLocale;
}

export default i18n;
