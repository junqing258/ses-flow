import { computed, ref } from "vue";

import {
  AI_PROVIDER_CONFIG_STORAGE_KEY,
  clearPersistedAiProviderConfig,
  isAiProviderConfigComplete,
  persistAiProviderConfig,
  readPersistedAiProviderConfig,
  resolveAiProviderConfigStorage,
  type AiProviderConfig,
} from "@/features/workflow/ai-provider";

const storage = resolveAiProviderConfigStorage();
const aiProviderConfig = ref<AiProviderConfig | null>(
  readPersistedAiProviderConfig(storage),
);

let storageListenerBound = false;

const reloadAiProviderConfig = () => {
  aiProviderConfig.value = readPersistedAiProviderConfig(storage);
  return aiProviderConfig.value;
};

const saveAiProviderConfig = (config: Partial<AiProviderConfig>) => {
  persistAiProviderConfig(storage, config);
  return reloadAiProviderConfig();
};

const clearAiProviderConfig = () => {
  clearPersistedAiProviderConfig(storage);
  return reloadAiProviderConfig();
};

const bindStorageListener = () => {
  if (storageListenerBound || typeof window === "undefined") {
    return;
  }

  window.addEventListener("storage", (event) => {
    if (event.key !== null && event.key !== AI_PROVIDER_CONFIG_STORAGE_KEY) {
      return;
    }

    reloadAiProviderConfig();
  });
  storageListenerBound = true;
};

export const useAiProviderConfig = () => {
  bindStorageListener();

  return {
    aiProviderConfig,
    clearAiProviderConfig,
    hasCompleteAiProviderConfig: computed(() =>
      isAiProviderConfigComplete(aiProviderConfig.value),
    ),
    reloadAiProviderConfig,
    saveAiProviderConfig,
  };
};
