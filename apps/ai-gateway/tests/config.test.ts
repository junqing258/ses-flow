import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { getAiProviderConfig } from "../src/config.js";

describe("AI Provider Config", () => {
  const originalEnv = process.env;

  beforeEach(() => {
    process.env = { ...originalEnv };
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  it("should read config from environment variables", () => {
    process.env.ANTHROPIC_AUTH_TOKEN = "sk-test-token";
    process.env.ANTHROPIC_BASE_URL = "https://api.example.com";
    process.env.ANTHROPIC_MODEL = "claude-sonnet-4-6";
    process.env.CLAUDE_CODE_EXECUTABLE = "/tmp/claude-code";

    const config = getAiProviderConfig();

    expect(config.authToken).toBe("sk-test-token");
    expect(config.baseUrl).toBe("https://api.example.com");
    expect(config.model).toBe("claude-sonnet-4-6");
    expect(config.claudeCodeExecutable).toBe("/tmp/claude-code");
  });

  it("should allow optional baseUrl and model", () => {
    process.env.ANTHROPIC_AUTH_TOKEN = "sk-test-token";
    delete process.env.ANTHROPIC_BASE_URL;
    delete process.env.ANTHROPIC_MODEL;
    delete process.env.CLAUDE_CODE_EXECUTABLE;

    const config = getAiProviderConfig();

    expect(config.authToken).toBe("sk-test-token");
    expect(config.baseUrl).toBeUndefined();
    expect(config.model).toBeUndefined();
    expect(config.claudeCodeExecutable).toBeUndefined();
  });

  it("should throw error when authToken is missing", () => {
    delete process.env.ANTHROPIC_AUTH_TOKEN;

    expect(() => getAiProviderConfig()).toThrow(
      "ANTHROPIC_AUTH_TOKEN is required in .env",
    );
  });
});
