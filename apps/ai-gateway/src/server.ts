import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import express, { type Response } from "express";

import { ClaudeCodeSdkAdapter, type ClaudeAdapter } from "./claude.js";
import { AiThreadStore } from "./state.js";
import { AiGatewayServiceError, createAiGatewayService } from "./service.js";
import type { AiThreadEvent } from "./types.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEFAULT_REPO_ROOT = resolve(__dirname, "../../..");

interface CreateAiGatewayAppOptions {
  claudeAdapter?: ClaudeAdapter;
  repoRoot?: string;
  store?: AiThreadStore;
}

const writeSseEvent = (response: Response, event: AiThreadEvent) => {
  response.write(`event: ${event.eventType}\n`);
  response.write(`data: ${JSON.stringify(event)}\n\n`);
};

export const createAiGatewayApp = (
  options: CreateAiGatewayAppOptions = {},
) => {
  const app = express();
  const store = options.store ?? new AiThreadStore();
  const claudeAdapter = options.claudeAdapter ?? new ClaudeCodeSdkAdapter();
  const repoRoot = options.repoRoot ?? DEFAULT_REPO_ROOT;
  const service = createAiGatewayService({
    claudeAdapter,
    repoRoot,
    store,
  });

  app.use(express.json({ limit: "1mb" }));

  app.get("/api/ai/threads/:editSessionId", (request, response) => {
    response.json(service.getSnapshot(request.params.editSessionId));
  });

  app.get("/api/ai/threads/:editSessionId/events", (request, response) => {
    response.setHeader("Content-Type", "text/event-stream");
    response.setHeader("Cache-Control", "no-cache");
    response.setHeader("Connection", "keep-alive");
    response.flushHeaders?.();

    const unsubscribe = service.subscribe(
      request.params.editSessionId,
      (event) => writeSseEvent(response, event),
    );

    request.on("close", () => {
      unsubscribe();
      response.end();
    });
  });

  app.post("/api/ai/threads/:editSessionId/messages", async (request, response) => {
    try {
      const snapshot = await service.sendMessage(
        request.params.editSessionId,
        request.body,
      );
      response.status(202).json(snapshot);
    } catch (error) {
      response.status(
        error instanceof AiGatewayServiceError ? error.statusCode : 500,
      ).json({
        error: error instanceof Error ? error.message : "发送 AI 消息失败",
      });
    }
  });

  app.post("/api/ai/threads/:editSessionId/cancel", (request, response) => {
    response.json(service.cancelTurn(request.params.editSessionId));
  });

  app.get("/health", (_request, response) => {
    response.json({
      status: "ok",
    });
  });

  return app;
};
