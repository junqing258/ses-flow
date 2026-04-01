export const DEFAULT_LOCALE = "zh-CN" as const;

export const SUPPORTED_LOCALES = ["zh-CN", "en-US"] as const;

export type AppLocale = (typeof SUPPORTED_LOCALES)[number];

export const LOCALE_STORAGE_KEY = "ses-flow:locale";

export const LOCALE_OPTIONS: ReadonlyArray<{ value: AppLocale; label: string }> = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en-US", label: "English" },
];

const LOCALE_ALIAS_MAP: Record<string, AppLocale> = {
  zh: "zh-CN",
  "zh-cn": "zh-CN",
  "zh-hans": "zh-CN",
  "zh-sg": "zh-CN",
  en: "en-US",
  "en-us": "en-US",
  "en-gb": "en-US",
  "en-au": "en-US",
};

const SUPPORTED_LOCALE_SET = new Set<string>(SUPPORTED_LOCALES);

export const isSupportedLocale = (value: string): value is AppLocale => SUPPORTED_LOCALE_SET.has(value);

export const normalizeLocale = (value?: string | null): AppLocale | null => {
  if (!value) {
    return null;
  }

  const normalizedValue = value.trim().toLowerCase();

  if (!normalizedValue) {
    return null;
  }

  const directMatch = LOCALE_ALIAS_MAP[normalizedValue];
  if (directMatch) {
    return directMatch;
  }

  const [language] = normalizedValue.split("-");
  return language ? LOCALE_ALIAS_MAP[language] ?? null : null;
};

export const resolvePreferredLocale = (candidates: ReadonlyArray<string | null | undefined>): AppLocale => {
  for (const candidate of candidates) {
    const locale = normalizeLocale(candidate);
    if (locale) {
      return locale;
    }
  }

  return DEFAULT_LOCALE;
};

export const resolveInitialLocale = (options?: {
  storageLocale?: string | null;
  navigatorLanguage?: string | null;
  navigatorLanguages?: readonly string[];
}): AppLocale => {
  const candidates: Array<string | null | undefined> = [options?.storageLocale];

  if (options?.navigatorLanguages?.length) {
    candidates.push(...options.navigatorLanguages);
  }

  candidates.push(options?.navigatorLanguage);

  return resolvePreferredLocale(candidates);
};
