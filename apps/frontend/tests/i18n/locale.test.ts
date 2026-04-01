import { describe, expect, it } from "vitest";

import {
  DEFAULT_LOCALE,
  isSupportedLocale,
  normalizeLocale,
  resolveInitialLocale,
  resolvePreferredLocale,
} from "@/i18n";

describe("i18n locale helpers", () => {
  it("recognizes supported locales", () => {
    expect(isSupportedLocale("zh-CN")).toBe(true);
    expect(isSupportedLocale("en-US")).toBe(true);
    expect(isSupportedLocale("fr-FR")).toBe(false);
  });

  it("normalizes locale aliases", () => {
    expect(normalizeLocale("zh")).toBe("zh-CN");
    expect(normalizeLocale("zh-Hans")).toBe("zh-CN");
    expect(normalizeLocale("en-GB")).toBe("en-US");
    expect(normalizeLocale("fr-FR")).toBeNull();
  });

  it("prefers the first supported locale candidate", () => {
    expect(resolvePreferredLocale([null, "fr-FR", "en-GB", "zh-CN"])).toBe("en-US");
  });

  it("falls back to the default locale", () => {
    expect(resolvePreferredLocale(["fr-FR"])).toBe(DEFAULT_LOCALE);
  });

  it("uses stored locale before browser locale", () => {
    expect(
      resolveInitialLocale({
        storageLocale: "en-US",
        navigatorLanguages: ["zh-CN"],
        navigatorLanguage: "zh-CN",
      }),
    ).toBe("en-US");
  });
});
