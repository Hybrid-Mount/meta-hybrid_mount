// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { createI18n } from "vue-i18n";
import en from "./en-US.json";
import es from "./es-ES.json";
import fr from "./fr-FR.json";
import id from "./id-ID.json";
import itLocale from "./it-IT.json";
import ja from "./ja-JP.json";
import ru from "./ru-RU.json";
import tr from "./tr-TR.json";
import uk from "./uk-UA.json";
import vi from "./vi-VN.json";
import zhTw from "./zh-TW.json";
import zh from "./zh-CN.json";
import i18n, { getSupportedLocales, loadLocale, pluralizationRules } from "./index";

type Example = readonly [count: number, countLabel: string, detailsLabel: string];
type LocaleMessages = Record<string, string | Record<string, string>>;
type LocaleCase = {
  locale: string;
  messages: LocaleMessages;
  examples: readonly Example[];
};

const localeCases: readonly LocaleCase[] = [
  {
    locale: "en-US",
    messages: en,
    examples: [
      [0, "0 targets", "View 0 mount targets"],
      [1, "1 target", "View 1 mount target"],
      [2, "2 targets", "View 2 mount targets"],
    ],
  },
  {
    locale: "es-ES",
    messages: es,
    examples: [
      [0, "0 destinos", "Ver 0 destinos de montaje"],
      [1, "1 destino", "Ver 1 destino de montaje"],
      [2, "2 destinos", "Ver 2 destinos de montaje"],
    ],
  },
  {
    locale: "fr-FR",
    messages: fr,
    examples: [
      [0, "0 cible", "Afficher 0 cible de montage"],
      [1, "1 cible", "Afficher 1 cible de montage"],
      [2, "2 cibles", "Afficher 2 cibles de montage"],
    ],
  },
  {
    locale: "id-ID",
    messages: id,
    examples: [
      [0, "0 titik mount", "Lihat 0 titik mount"],
      [1, "1 titik mount", "Lihat 1 titik mount"],
      [2, "2 titik mount", "Lihat 2 titik mount"],
    ],
  },
  {
    locale: "it-IT",
    messages: itLocale,
    examples: [
      [0, "0 destinazioni", "Visualizza 0 destinazioni di montaggio"],
      [1, "1 destinazione", "Visualizza 1 destinazione di montaggio"],
      [2, "2 destinazioni", "Visualizza 2 destinazioni di montaggio"],
    ],
  },
  {
    locale: "ja-JP",
    messages: ja,
    examples: [
      [0, "マウント先 0 件", "0 件のマウント先を表示"],
      [1, "マウント先 1 件", "1 件のマウント先を表示"],
      [2, "マウント先 2 件", "2 件のマウント先を表示"],
    ],
  },
  {
    locale: "ru-RU",
    messages: ru,
    examples: [
      [0, "0 точек монтирования", "Просмотреть 0 точек монтирования"],
      [1, "1 точка монтирования", "Просмотреть 1 точку монтирования"],
      [2, "2 точки монтирования", "Просмотреть 2 точки монтирования"],
      [5, "5 точек монтирования", "Просмотреть 5 точек монтирования"],
      [11, "11 точек монтирования", "Просмотреть 11 точек монтирования"],
      [21, "21 точка монтирования", "Просмотреть 21 точку монтирования"],
      [22, "22 точки монтирования", "Просмотреть 22 точки монтирования"],
      [111, "111 точек монтирования", "Просмотреть 111 точек монтирования"],
      [112, "112 точек монтирования", "Просмотреть 112 точек монтирования"],
    ],
  },
  {
    locale: "tr-TR",
    messages: tr,
    examples: [
      [0, "0 bağlama noktası", "0 bağlama noktasını görüntüle"],
      [1, "1 bağlama noktası", "1 bağlama noktasını görüntüle"],
      [2, "2 bağlama noktası", "2 bağlama noktasını görüntüle"],
    ],
  },
  {
    locale: "uk-UA",
    messages: uk,
    examples: [
      [0, "0 точок монтування", "Переглянути 0 точок монтування"],
      [1, "1 точка монтування", "Переглянути 1 точку монтування"],
      [2, "2 точки монтування", "Переглянути 2 точки монтування"],
      [5, "5 точок монтування", "Переглянути 5 точок монтування"],
      [11, "11 точок монтування", "Переглянути 11 точок монтування"],
      [21, "21 точка монтування", "Переглянути 21 точку монтування"],
      [22, "22 точки монтування", "Переглянути 22 точки монтування"],
      [111, "111 точок монтування", "Переглянути 111 точок монтування"],
      [112, "112 точок монтування", "Переглянути 112 точок монтування"],
    ],
  },
  {
    locale: "vi-VN",
    messages: vi,
    examples: [
      [0, "0 điểm gắn kết", "Xem 0 điểm gắn kết"],
      [1, "1 điểm gắn kết", "Xem 1 điểm gắn kết"],
      [2, "2 điểm gắn kết", "Xem 2 điểm gắn kết"],
    ],
  },
  {
    locale: "zh-TW",
    messages: zhTw,
    examples: [
      [0, "0 個掛載點", "檢視 0 個掛載點"],
      [1, "1 個掛載點", "檢視 1 個掛載點"],
      [2, "2 個掛載點", "檢視 2 個掛載點"],
    ],
  },
  {
    locale: "zh-CN",
    messages: zh,
    examples: [
      [0, "0 个挂载点", "查看 0 个挂载点"],
      [1, "1 个挂载点", "查看 1 个挂载点"],
      [2, "2 个挂载点", "查看 2 个挂载点"],
    ],
  },
];

function createTranslator(locale: string, messages: LocaleMessages) {
  const i18n = createI18n({
    legacy: false,
    locale,
    messages: { [locale]: messages },
    pluralRules: pluralizationRules,
  });

  return (key: "mountTargetCount" | "mountDetails", count: number): string =>
    i18n.global.t(`status.${key}`, { count });
}

for (const localeCase of localeCases) {
  describe(`${localeCase.locale} mount target counts`, () => {
    const translate = createTranslator(localeCase.locale, localeCase.messages);

    for (const [count, countLabel, detailsLabel] of localeCase.examples) {
      it(`renders ${count} correctly`, () => {
        expect(translate("mountTargetCount", count)).toBe(countLabel);
        expect(translate("mountDetails", count)).toBe(detailsLabel);
      });
    }
  });
}

describe("production i18n pluralization", () => {
  it("discovers every locale from a language-region filename", async () => {
    const localeCodes = (await getSupportedLocales()).map(({ code }) => code);

    expect(localeCodes).toContain("en-US");
    expect(localeCodes).toContain("zh-CN");
    expect(localeCodes.every((code) => /^[a-z]{2}-[A-Z]{2}$/.test(code))).toBe(true);
  });

  it("registers the east Slavic rules", async () => {
    await loadLocale("ru-RU");
    const previousLocale = i18n.global.locale.value;
    i18n.global.locale.value = "ru-RU";

    try {
      expect(i18n.global.t("status.mountTargetCount", { count: 21 })).toBe(
        "21 точка монтирования",
      );
      expect(i18n.global.t("status.mountTargetCount", { count: 22 })).toBe(
        "22 точки монтирования",
      );
    } finally {
      i18n.global.locale.value = previousLocale;
    }
  });
});
