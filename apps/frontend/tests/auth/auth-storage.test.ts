import { describe, expect, it } from "vitest";

import {
  AUTH_TOKEN_STORAGE_KEY,
  clearPersistedAccessToken,
  persistAccessToken,
  readPersistedAccessToken,
  type StorageLike,
} from "@/lib/auth-storage";

const createStorage = (): StorageLike & { snapshot: () => Record<string, string> } => {
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

describe("auth storage helpers", () => {
  it("stores and reads access tokens", () => {
    const storage = createStorage();

    persistAccessToken(storage, "token-123");

    expect(storage.snapshot()).toEqual({
      [AUTH_TOKEN_STORAGE_KEY]: "token-123",
    });
    expect(readPersistedAccessToken(storage)).toBe("token-123");
  });

  it("treats blank values as empty", () => {
    const storage = createStorage();
    storage.setItem(AUTH_TOKEN_STORAGE_KEY, "   ");

    expect(readPersistedAccessToken(storage)).toBeNull();
  });

  it("clears tokens safely", () => {
    const storage = createStorage();
    persistAccessToken(storage, "token-123");

    clearPersistedAccessToken(storage);

    expect(readPersistedAccessToken(storage)).toBeNull();
  });
});
