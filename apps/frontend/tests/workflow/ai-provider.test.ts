import { describe, expect, it } from "vitest";

import {
  AI_PROVIDER_CONFIG_STORAGE_KEY,
  clearPersistedAiProviderConfig,
  isAiProviderConfigComplete,
  persistAiProviderConfig,
  readPersistedAiProviderConfig,
  type StorageLike,
} from "@/features/workflow/ai-provider";

const createStorage = (): StorageLike & {
  snapshot: () => Record<string, string>;
} => {
  const data = new Map<string, string>();

  return {
    getItem(key) {
      return data.get(key) ?? null;
    },
    setItem(key, value) {
      data.set(key, value);
    },
    removeItem(key) {
      data.delete(key);
    },
    snapshot() {
      return Object.fromEntries(data.entries());
    },
  };
};

describe("ai provider storage helpers", () => {
  it("persists normalized provider config", () => {
    const storage = createStorage();

    persistAiProviderConfig(storage, {
      authToken: "  sk-ant-test  ",
      baseUrl: " https://api.anthropic.com ",
      model: " claude-sonnet-4-6 ",
    });

    expect(
      JSON.parse(storage.snapshot()[AI_PROVIDER_CONFIG_STORAGE_KEY] ?? "null"),
    ).toEqual({
      authToken: "sk-ant-test",
      baseUrl: "https://api.anthropic.com",
      model: "claude-sonnet-4-6",
    });
    expect(readPersistedAiProviderConfig(storage)).toEqual({
      authToken: "sk-ant-test",
      baseUrl: "https://api.anthropic.com",
      model: "claude-sonnet-4-6",
    });
  });

  it("clears invalid persisted payloads", () => {
    const storage = createStorage();
    storage.setItem(AI_PROVIDER_CONFIG_STORAGE_KEY, "{invalid json");

    expect(readPersistedAiProviderConfig(storage)).toBeNull();
    expect(storage.snapshot()).toEqual({});
  });

  it("removes blank configs instead of storing them", () => {
    const storage = createStorage();

    persistAiProviderConfig(storage, {
      authToken: "   ",
      baseUrl: "",
      model: undefined,
    });

    expect(storage.snapshot()).toEqual({});
  });

  it("checks completeness and clears safely", () => {
    const storage = createStorage();

    persistAiProviderConfig(storage, {
      authToken: "token",
      baseUrl: "https://api.anthropic.com",
      model: "claude-sonnet-4-6",
    });

    expect(
      isAiProviderConfigComplete(readPersistedAiProviderConfig(storage)),
    ).toBe(true);

    clearPersistedAiProviderConfig(storage);

    expect(readPersistedAiProviderConfig(storage)).toBeNull();
    expect(
      isAiProviderConfigComplete({ baseUrl: "https://api.anthropic.com" }),
    ).toBe(false);
  });
});
