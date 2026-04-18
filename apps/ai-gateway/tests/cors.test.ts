import { describe, expect, it } from "vitest";

import {
  buildCorsHeaders,
  isPreflightRequest,
  resolveCorsAllowHeaders,
} from "../src/cors.js";

describe("ai gateway cors", () => {
  it("returns default allow headers when request headers are absent", () => {
    expect(resolveCorsAllowHeaders()).toBe(
      "Content-Type, Authorization, Cache-Control, Last-Event-ID",
    );
  });

  it("reuses requested headers for preflight responses", () => {
    expect(resolveCorsAllowHeaders("content-type,authorization")).toBe(
      "content-type,authorization",
    );
  });

  it("builds permissive cors headers for browser clients", () => {
    expect(buildCorsHeaders("content-type")).toEqual({
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "content-type",
      "Access-Control-Max-Age": "86400",
      Vary: "Access-Control-Request-Headers",
    });
  });

  it("detects preflight requests", () => {
    expect(isPreflightRequest("OPTIONS")).toBe(true);
    expect(isPreflightRequest("options")).toBe(true);
    expect(isPreflightRequest("POST")).toBe(false);
  });
});
