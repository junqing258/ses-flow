#!/usr/bin/env node
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import dotenv from "dotenv";
import { getAiProviderConfig } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// 加载项目根目录的 .env 文件
dotenv.config({
  path: resolve(__dirname, "../../../.env"),
});

console.log("=== AI Provider Configuration ===");
try {
  const config = getAiProviderConfig();
  console.log("✓ Auth Token:", config.authToken.substring(0, 15) + "...");
  console.log("✓ Base URL:", config.baseUrl || "(using default)");
  console.log("✓ Model:", config.model || "(using default)");
  console.log("\n✓ Configuration loaded successfully!");
} catch (error) {
  console.error("✗ Configuration error:", error instanceof Error ? error.message : String(error));
  process.exit(1);
}
