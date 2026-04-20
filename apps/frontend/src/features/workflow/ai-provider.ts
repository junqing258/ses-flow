export interface AiProviderConfig {
  baseUrl?: string;
  authToken?: string;
  model?: string;
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export const AI_PROVIDER_CONFIG_STORAGE_KEY = "ses-flow.ai-provider-config";

const normalizeString = (value: unknown) => {
  if (typeof value !== "string") {
    return undefined;
  }

  const normalizedValue = value.trim();
  return normalizedValue ? normalizedValue : undefined;
};

export const normalizeAiProviderConfig = (
  config: Partial<AiProviderConfig>,
): AiProviderConfig => ({
  baseUrl: normalizeString(config.baseUrl),
  authToken: normalizeString(config.authToken),
  model: normalizeString(config.model),
});

export const isAiProviderConfigEmpty = (config: Partial<AiProviderConfig>) => {
  const normalizedConfig = normalizeAiProviderConfig(config);

  return !(
    normalizedConfig.baseUrl ||
    normalizedConfig.authToken ||
    normalizedConfig.model
  );
};

export const isAiProviderConfigComplete = (
  config: Partial<AiProviderConfig> | null | undefined,
) => {
  const normalizedConfig = normalizeAiProviderConfig(config ?? {});

  return Boolean(
    normalizedConfig.baseUrl &&
    normalizedConfig.authToken &&
    normalizedConfig.model,
  );
};

export const resolveAiProviderConfigStorage = () => {
  if (typeof window === "undefined") {
    return null;
  }

  return window.localStorage;
};

export const readPersistedAiProviderConfig = (storage: StorageLike | null) => {
  if (!storage) {
    return null;
  }

  const rawValue = storage.getItem(AI_PROVIDER_CONFIG_STORAGE_KEY);
  if (!rawValue) {
    return null;
  }

  try {
    const parsedValue = JSON.parse(rawValue) as Partial<AiProviderConfig>;
    const normalizedConfig = normalizeAiProviderConfig(parsedValue);

    return isAiProviderConfigEmpty(normalizedConfig) ? null : normalizedConfig;
  } catch {
    storage.removeItem(AI_PROVIDER_CONFIG_STORAGE_KEY);
    return null;
  }
};

export const persistAiProviderConfig = (
  storage: StorageLike | null,
  config: Partial<AiProviderConfig>,
) => {
  if (!storage) {
    return;
  }

  const normalizedConfig = normalizeAiProviderConfig(config);

  if (isAiProviderConfigEmpty(normalizedConfig)) {
    storage.removeItem(AI_PROVIDER_CONFIG_STORAGE_KEY);
    return;
  }

  storage.setItem(
    AI_PROVIDER_CONFIG_STORAGE_KEY,
    JSON.stringify(normalizedConfig),
  );
};

export const clearPersistedAiProviderConfig = (storage: StorageLike | null) => {
  storage?.removeItem(AI_PROVIDER_CONFIG_STORAGE_KEY);
};
