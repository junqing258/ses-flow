import { describe, it, expect } from "vitest";
import { resolveAiProviderConfig } from "../src/config.js";

describe("AI Provider Config", () => {
  it("resolves config from request payload", () => {
    const config = resolveAiProviderConfig({
      authToken: "sk-test-token",
      baseUrl: "https://api.example.com",
      model: "claude-sonnet-4-6",
    });

    expect(config.authToken).toBe("sk-test-token");
    expect(config.baseUrl).toBe("https://api.example.com");
    expect(config.model).toBe("claude-sonnet-4-6");
  });

  it("throws when request config is incomplete", () => {
    expect(() =>
      resolveAiProviderConfig({
        authToken: "sk-test-token",
        baseUrl: "",
        model: "claude-sonnet-4-6",
      }),
    ).toThrow("baseUrl is required in request aiProvider config");
  });
});
