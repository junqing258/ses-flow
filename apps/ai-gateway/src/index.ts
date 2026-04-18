import { resolve } from "node:path";
import { createServer } from "node:http";

import dotenv from "dotenv";

import { createAiGatewayApp } from "./server.js";

dotenv.config({
  path: resolve(process.cwd(), ".env"),
});

const port = Number(process.env.AI_GATEWAY_PORT ?? "3000");
const host = process.env.AI_GATEWAY_HOST ?? "127.0.0.1";

const app = createAiGatewayApp();
const server = createServer(app);

server.listen(port, host, () => {
  console.log(`ai-gateway listening on http://${host}:${port}`);
});
