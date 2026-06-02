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
  const [isReady, setIsReady] = createSignal(false);

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

  async function init() {
    const savedLang = localStorage.getItem("lang") || "en-US";
    setLangSignal(savedLang);
    await loadLocale(savedLang);
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
    get isReady() {
      return isReady();
    },
    showToast,
    setLang,
    init,
  };
};

export const uiStore = createRoot(createUiStore);
