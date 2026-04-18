import { resolve, dirname } from "node:path";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";

import dotenv from "dotenv";

import { createAiGatewayApp } from "./server.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// 加载项目根目录的 .env 文件
dotenv.config({
  path: resolve(__dirname, "../../../.env"),
});

const port = Number(process.env.AI_GATEWAY_PORT ?? "6307");
const host = process.env.AI_GATEWAY_HOST ?? "127.0.0.1";

const app = createAiGatewayApp();
const server = createServer(app);

server.listen(port, host, () => {
  console.log(`ai-gateway listening on http://${host}:${port}`);
});
