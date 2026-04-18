import type { AiProviderConfig as RequestAiProviderConfig } from "./types.js";

export interface AiProviderConfig extends RequestAiProviderConfig {
  claudeCodeExecutable?: string;
}

const requireConfigValue = (fieldName: keyof RequestAiProviderConfig, value: unknown) => {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${fieldName} is required in request aiProvider config`);
  }

  return value.trim();
};

export const resolveAiProviderConfig = (
  config: Partial<RequestAiProviderConfig>,
): AiProviderConfig => ({
  baseUrl: requireConfigValue("baseUrl", config.baseUrl),
  authToken: requireConfigValue("authToken", config.authToken),
  model: requireConfigValue("model", config.model),
  claudeCodeExecutable: process.env.CLAUDE_CODE_EXECUTABLE,
});
