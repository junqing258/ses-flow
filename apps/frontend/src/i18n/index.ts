import { watch } from "vue";
import { createI18n } from "vue-i18n";

import {
  DEFAULT_LOCALE,
  LOCALE_OPTIONS,
  LOCALE_STORAGE_KEY,
  resolveInitialLocale,
  type AppLocale,
} from "./locales";
import enUS from "./messages/en-US";
import zhCN from "./messages/zh-CN";

const messages = {
  "zh-CN": zhCN,
  "en-US": enUS,
} as const;

const getInitialLocale = (): AppLocale => {
  if (typeof window === "undefined") {
    return DEFAULT_LOCALE;
  }

  return resolveInitialLocale({
    storageLocale: window.localStorage.getItem(LOCALE_STORAGE_KEY),
    navigatorLanguages: window.navigator.languages,
    navigatorLanguage: window.navigator.language,
  });
};

const syncDocumentLanguage = (locale: AppLocale) => {
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale;
  }
};

const persistLocale = (locale: AppLocale) => {
  if (typeof window !== "undefined") {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  }
};

export const i18n = createI18n({
  legacy: false,
  locale: getInitialLocale(),
  fallbackLocale: DEFAULT_LOCALE,
  messages,
});

watch(
  i18n.global.locale,
  (locale) => {
    syncDocumentLanguage(locale);
    persistLocale(locale);
  },
  { immediate: true },
);

export const setLocale = (locale: AppLocale) => {
  i18n.global.locale.value = locale;
};

export {
  DEFAULT_LOCALE,
  LOCALE_OPTIONS,
  SUPPORTED_LOCALES,
  isSupportedLocale,
  normalizeLocale,
  resolveInitialLocale,
  resolvePreferredLocale,
  type AppLocale,
} from "./locales";
