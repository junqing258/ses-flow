#!/usr/bin/env node
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import dotenv from "dotenv";
import { resolveAiProviderConfig } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// 加载项目根目录的 .env 文件
dotenv.config({
  path: resolve(__dirname, "../../../.env"),
});

console.log("=== AI Provider Configuration ===");
try {
  const config = resolveAiProviderConfig({
    authToken: process.env.ANTHROPIC_AUTH_TOKEN,
    baseUrl: process.env.ANTHROPIC_BASE_URL,
    model: process.env.ANTHROPIC_MODEL,
  });
  console.log("✓ Auth Token:", config.authToken.substring(0, 15) + "...");
  console.log("✓ Base URL:", config.baseUrl);
  console.log("✓ Model:", config.model);
  console.log("\n✓ Configuration loaded successfully!");
} catch (error) {
  console.error("✗ Configuration error:", error instanceof Error ? error.message : String(error));
  process.exit(1);
}
