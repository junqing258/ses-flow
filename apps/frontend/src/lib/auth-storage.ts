export const AUTH_TOKEN_STORAGE_KEY = "ses.auth.access-token";

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export const resolveAuthStorage = () => {
  if (typeof window === "undefined") {
    return null;
  }

  return window.localStorage;
};

export const readPersistedAccessToken = (storage: StorageLike | null) => {
  if (!storage) {
    return null;
  }

  const token = storage.getItem(AUTH_TOKEN_STORAGE_KEY);
  return token && token.trim() ? token : null;
};

export const persistAccessToken = (storage: StorageLike | null, token: string) => {
  if (!storage) {
    return;
  }

  storage.setItem(AUTH_TOKEN_STORAGE_KEY, token);
};

export const clearPersistedAccessToken = (storage: StorageLike | null) => {
  if (!storage) {
    return;
  }

  storage.removeItem(AUTH_TOKEN_STORAGE_KEY);
};
