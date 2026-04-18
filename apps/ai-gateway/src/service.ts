import type { ClaudeAdapter } from "./claude.js";
import { AiThreadStore } from "./state.js";
import type { AiThreadEvent, AiThreadSnapshot, SendAiThreadMessageRequest } from "./types.js";

export class AiGatewayServiceError extends Error {
  constructor(
    readonly statusCode: number,
    message: string,
  ) {
    super(message);
    this.name = "AiGatewayServiceError";
  }
}

interface CreateAiGatewayServiceOptions {
  claudeAdapter: ClaudeAdapter;
  repoRoot: string;
  store?: AiThreadStore;
}

const validateMessagePayload = (body: unknown): SendAiThreadMessageRequest => {
  if (typeof body !== "object" || body === null) {
    throw new AiGatewayServiceError(400, "请求体必须是 JSON 对象");
  }

  const { message, runnerBaseUrl, workflowId } = body as Record<string, unknown>;

  if (typeof message !== "string" || !message.trim()) {
    throw new AiGatewayServiceError(400, "message 不能为空");
  }

  if (typeof runnerBaseUrl !== "string" || !runnerBaseUrl.trim()) {
    throw new AiGatewayServiceError(400, "runnerBaseUrl 不能为空");
  }

  if (workflowId != null && typeof workflowId !== "string") {
    throw new AiGatewayServiceError(400, "workflowId 必须是字符串");
  }

  return {
    message: message.trim(),
    runnerBaseUrl: runnerBaseUrl.trim().replace(/\/$/, ""),
    workflowId: typeof workflowId === "string" ? workflowId : undefined,
  };
};

export const createAiGatewayService = (
  options: CreateAiGatewayServiceOptions,
) => {
  const store = options.store ?? new AiThreadStore();

  return {
    getSnapshot(editSessionId: string) {
      return store.getSnapshot(editSessionId);
    },
    subscribe(editSessionId: string, listener: (event: AiThreadEvent) => void) {
      return store.subscribe(editSessionId, listener);
    },
    cancelTurn(editSessionId: string) {
      return store.cancelTurn(editSessionId);
    },
    async sendMessage(editSessionId: string, body: unknown): Promise<AiThreadSnapshot> {
      const payload = validateMessagePayload(body);

      if (store.isRunning(editSessionId)) {
        throw new AiGatewayServiceError(
          409,
          "当前会话正在处理中，请等待当前回合完成后再发送新消息",
        );
      }

      store.addUserMessage(editSessionId, payload.message);
      const abortController = new AbortController();
      store.startTurn(editSessionId, abortController);
      const snapshot = store.getSnapshot(editSessionId);

      void (async () => {
        try {
          await options.claudeAdapter.runTurn({
            abortController,
            claudeSessionId: snapshot.claudeSessionId,
            editSessionId,
            prompt: payload.message,
            repoRoot: options.repoRoot,
            runnerBaseUrl: payload.runnerBaseUrl,
            workflowId: payload.workflowId,
            onAssistantDelta: (delta) => {
              store.appendAssistantDelta(editSessionId, delta);
            },
            onAssistantCompleted: () => {
              store.completeAssistantMessage(editSessionId);
            },
            onClaudeSessionId: (claudeSessionId) => {
              store.setClaudeSessionId(editSessionId, claudeSessionId);
            },
            onPreviewUpdated: () => {
              store.markPreviewUpdated(editSessionId);
            },
            onToolStarted: (toolCallId, toolName, content) => {
              store.startToolCall(editSessionId, toolCallId, toolName, content);
            },
            onToolCompleted: (toolCallId, content) => {
              store.completeToolCall(editSessionId, toolCallId, content);
            },
          });
          store.completeAssistantMessage(editSessionId);
          store.finishTurn(editSessionId);
        } catch (error) {
          if (abortController.signal.aborted) {
            return;
          }

          store.failTurn(
            editSessionId,
            error instanceof Error ? error.message : "Claude 协作执行失败",
          );
        }
      })();

      return store.getSnapshot(editSessionId);
    },
  };
};
