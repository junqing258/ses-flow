export interface AiProviderConfig {
  baseUrl?: string;
  authToken: string;
  model?: string;
}

export const getAiProviderConfig = (): AiProviderConfig => {
  const authToken = process.env.ANTHROPIC_AUTH_TOKEN;
  if (!authToken) {
    throw new Error("ANTHROPIC_AUTH_TOKEN is required in .env");
  }

  return {
    baseUrl: process.env.ANTHROPIC_BASE_URL,
    authToken,
    model: process.env.ANTHROPIC_MODEL,
  };
};
