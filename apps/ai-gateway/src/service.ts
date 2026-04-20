import type { ClaudeAdapter } from "./claude.js";
import { logger, summarizeText } from "./logger.js";
import { AiThreadStore } from "./state.js";
import type {
  AiProviderConfig,
  AiThreadEvent,
  AiThreadSnapshot,
  SendAiThreadMessageRequest,
} from "./types.js";

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

const resolveRunnerBaseUrl = (runnerBaseUrl: string) => {
  const internalRunnerBaseUrl = process.env.AI_GATEWAY_RUNNER_BASE_URL?.trim();
  if (internalRunnerBaseUrl) {
    return internalRunnerBaseUrl.replace(/\/$/, "");
  }

  return runnerBaseUrl.trim().replace(/\/$/, "");
};

const validateMessagePayload = (body: unknown): SendAiThreadMessageRequest => {
  if (typeof body !== "object" || body === null) {
    throw new AiGatewayServiceError(400, "请求体必须是 JSON 对象");
  }

  const { aiProvider, message, runnerBaseUrl, workflowId } = body as Record<
    string,
    unknown
  >;

  if (typeof message !== "string" || !message.trim()) {
    throw new AiGatewayServiceError(400, "message 不能为空");
  }

  if (typeof runnerBaseUrl !== "string" || !runnerBaseUrl.trim()) {
    throw new AiGatewayServiceError(400, "runnerBaseUrl 不能为空");
  }

  if (workflowId != null && typeof workflowId !== "string") {
    throw new AiGatewayServiceError(400, "workflowId 必须是字符串");
  }

  if (typeof aiProvider !== "object" || aiProvider === null) {
    throw new AiGatewayServiceError(400, "aiProvider 必须是对象");
  }

  const {
    authToken,
    baseUrl,
    model,
  } = aiProvider as Partial<AiProviderConfig>;

  if (typeof baseUrl !== "string" || !baseUrl.trim()) {
    throw new AiGatewayServiceError(400, "aiProvider.baseUrl 不能为空");
  }

  if (typeof authToken !== "string" || !authToken.trim()) {
    throw new AiGatewayServiceError(400, "aiProvider.authToken 不能为空");
  }

  if (typeof model !== "string" || !model.trim()) {
    throw new AiGatewayServiceError(400, "aiProvider.model 不能为空");
  }

  return {
    aiProvider: {
      authToken: authToken.trim(),
      baseUrl: baseUrl.trim(),
      model: model.trim(),
    },
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
      logger.info("thread.turn.cancel.requested", {
        editSessionId,
      });
      return store.cancelTurn(editSessionId);
    },
    async sendMessage(editSessionId: string, body: unknown): Promise<AiThreadSnapshot> {
      const payload = validateMessagePayload(body);
      const effectiveRunnerBaseUrl = resolveRunnerBaseUrl(payload.runnerBaseUrl);

      if (store.isRunning(editSessionId)) {
        throw new AiGatewayServiceError(
          409,
          "当前会话正在处理中，请等待当前回合完成后再发送新消息",
        );
      }

      logger.info("thread.turn.requested", {
        editSessionId,
        aiProviderBaseUrl: payload.aiProvider.baseUrl,
        aiProviderModel: payload.aiProvider.model,
        workflowId: payload.workflowId,
        runnerBaseUrl: effectiveRunnerBaseUrl,
        requestedRunnerBaseUrl: payload.runnerBaseUrl,
        promptPreview: summarizeText(payload.message),
      });

      store.addUserMessage(editSessionId, payload.message);
      const abortController = new AbortController();
      store.startTurn(editSessionId, abortController);
      const snapshot = store.getSnapshot(editSessionId);
      const turnStartedAt = Date.now();

      void (async () => {
        const toolStartedAt = new Map<string, number>();
        let assistantStartedAt: number | null = null;
        let assistantChars = 0;
        let toolCallCount = 0;

        try {
          await options.claudeAdapter.runTurn({
            abortController,
            claudeSessionId: snapshot.claudeSessionId,
            editSessionId,
            aiProvider: payload.aiProvider,
            prompt: payload.message,
            repoRoot: options.repoRoot,
            runnerBaseUrl: effectiveRunnerBaseUrl,
            workflowId: payload.workflowId,
            onAssistantDelta: (delta) => {
              if (assistantStartedAt == null) {
                assistantStartedAt = Date.now();
                logger.info("thread.assistant.started", {
                  editSessionId,
                });
              }
              assistantChars += delta.length;
              store.appendAssistantDelta(editSessionId, delta);
            },
            onAssistantCompleted: () => {
              store.completeAssistantMessage(editSessionId);
              logger.info("thread.assistant.completed", {
                editSessionId,
                durationMs:
                  assistantStartedAt == null ? 0 : Date.now() - assistantStartedAt,
                chars: assistantChars,
              });
            },
            onClaudeSessionId: (claudeSessionId) => {
              store.setClaudeSessionId(editSessionId, claudeSessionId);
              logger.info("thread.claude_session.bound", {
                editSessionId,
                claudeSessionId,
              });
            },
            onPreviewUpdated: () => {
              store.markPreviewUpdated(editSessionId);
              logger.info("thread.preview.updated", {
                editSessionId,
              });
            },
            onToolStarted: (toolCallId, toolName, content) => {
              toolCallCount += 1;
              toolStartedAt.set(toolCallId, Date.now());
              store.startToolCall(editSessionId, toolCallId, toolName, content);
              logger.info("thread.tool.started", {
                editSessionId,
                toolCallId,
                toolName,
                content: summarizeText(content),
              });
            },
            onToolCompleted: (toolCallId, content) => {
              store.completeToolCall(editSessionId, toolCallId, content);
              logger.info("thread.tool.completed", {
                editSessionId,
                toolCallId,
                durationMs: toolStartedAt.has(toolCallId)
                  ? Date.now() - (toolStartedAt.get(toolCallId) ?? Date.now())
                  : undefined,
                content: content ? summarizeText(content) : undefined,
              });
              toolStartedAt.delete(toolCallId);
            },
          });
          store.completeAssistantMessage(editSessionId);
          store.finishTurn(editSessionId);
          logger.info("thread.turn.completed", {
            editSessionId,
            durationMs: Date.now() - turnStartedAt,
            toolCallCount,
            assistantChars,
          });
        } catch (error) {
          if (abortController.signal.aborted) {
            logger.warn("thread.turn.aborted", {
              editSessionId,
              durationMs: Date.now() - turnStartedAt,
            });
            return;
          }

          const errorMessage =
            error instanceof Error ? error.message : "Claude 协作执行失败";
          store.failTurn(
            editSessionId,
            errorMessage,
          );
          logger.error("thread.turn.failed", {
            editSessionId,
            durationMs: Date.now() - turnStartedAt,
            toolCallCount,
            assistantChars,
            error: errorMessage,
          });
        }
      })();

      return store.getSnapshot(editSessionId);
    },
  };
};
