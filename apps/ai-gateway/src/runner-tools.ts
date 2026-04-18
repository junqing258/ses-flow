import {
  createSdkMcpServer,
  tool,
  type McpServerConfig,
} from "@anthropic-ai/claude-agent-sdk";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";
import { z } from "zod";

export const RUNNER_MCP_SERVER_NAME = "ses-flow-runner";
export const GET_CURRENT_EDIT_SESSION_TOOL_NAME = "get_current_edit_session";
export const UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME =
  "update_current_edit_session_draft";
export const RUNNER_TOOL_REQUEST_TIMEOUT_MS = 10_000;

type FetchLike = typeof fetch;

interface CreateRunnerEditSessionMcpServerParams {
  editSessionId: string;
  fetchImpl?: FetchLike;
  runnerBaseUrl: string;
}

const normalizeRunnerBaseUrl = (runnerBaseUrl: string) =>
  runnerBaseUrl.trim().replace(/\/$/, "");

const buildRunnerUrl = (runnerBaseUrl: string, path: string) =>
  `${normalizeRunnerBaseUrl(runnerBaseUrl)}${path}`;

const createTextResult = (text: string): CallToolResult => ({
  content: [
    {
      type: "text",
      text,
    },
  ],
});

const createJsonResult = (value: unknown): CallToolResult =>
  createTextResult(JSON.stringify(value, null, 2));

const createErrorResult = (message: string): CallToolResult => ({
  content: [
    {
      type: "text",
      text: message,
    },
  ],
  isError: true,
});

const getErrorMessage = (
  payload: unknown,
  fallbackMessage: string,
) => {
  if (typeof payload === "string" && payload.trim()) {
    return payload;
  }

  if (
    typeof payload === "object" &&
    payload !== null &&
    "error" in payload &&
    typeof payload.error === "string" &&
    payload.error.trim()
  ) {
    return payload.error;
  }

  if (
    typeof payload === "object" &&
    payload !== null &&
    "message" in payload &&
    typeof payload.message === "string" &&
    payload.message.trim()
  ) {
    return payload.message;
  }

  return fallbackMessage;
};

const parseRunnerResponse = async (response: Response) => {
  const contentType = response.headers.get("content-type") ?? "";

  if (contentType.includes("application/json")) {
    return await response.json();
  }

  return await response.text();
};

const callRunner = async (
  fetchImpl: FetchLike,
  url: string,
  init: RequestInit,
  fallbackMessage: string,
) => {
  const abortController = new AbortController();
  const timeoutId = setTimeout(() => {
    abortController.abort();
  }, RUNNER_TOOL_REQUEST_TIMEOUT_MS);

  try {
    const response = await fetchImpl(url, {
      ...init,
      signal: abortController.signal,
    });
    const payload = await parseRunnerResponse(response);

    if (!response.ok) {
      return createErrorResult(getErrorMessage(payload, fallbackMessage));
    }

    return createJsonResult(payload);
  } catch (error) {
    if (abortController.signal.aborted) {
      return createErrorResult(
        `${fallbackMessage}: 请求超时（${RUNNER_TOOL_REQUEST_TIMEOUT_MS}ms）`,
      );
    }

    const message = error instanceof Error ? error.message : String(error);
    return createErrorResult(`${fallbackMessage}: ${message}`);
  } finally {
    clearTimeout(timeoutId);
  }
};

export const isRunnerMcpToolName = (toolName: string) =>
  toolName === GET_CURRENT_EDIT_SESSION_TOOL_NAME ||
  toolName === UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
  toolName.startsWith(`mcp__${RUNNER_MCP_SERVER_NAME}__`);

export const isPreviewMutationToolName = (toolName: string) =>
  toolName === UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
  toolName.endsWith(`__${UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`);

export const createRunnerEditSessionMcpServer = ({
  editSessionId,
  fetchImpl = fetch,
  runnerBaseUrl,
}: CreateRunnerEditSessionMcpServerParams): McpServerConfig =>
  createSdkMcpServer({
    name: RUNNER_MCP_SERVER_NAME,
    tools: [
      tool(
        GET_CURRENT_EDIT_SESSION_TOOL_NAME,
        "读取当前 edit session 的完整内容，包括 workflow、editorDocument 和元信息。",
        {},
        async () =>
          callRunner(
            fetchImpl,
            buildRunnerUrl(
              runnerBaseUrl,
              `/edit-sessions/${encodeURIComponent(editSessionId)}`,
            ),
            {
              method: "GET",
            },
            "获取当前 edit session 失败",
          ),
      ),
      tool(
        UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
        "更新当前 edit session 的 draft。必须传完整 workflow；可选传 editorDocument 和 workflowId。",
        {
          editorDocument: z.unknown().optional(),
          workflow: z.unknown(),
          workflowId: z.string().min(1).optional(),
        },
        async ({ editorDocument, workflow, workflowId }) =>
          callRunner(
            fetchImpl,
            buildRunnerUrl(
              runnerBaseUrl,
              `/edit-sessions/${encodeURIComponent(editSessionId)}/draft`,
            ),
            {
              method: "PUT",
              headers: {
                "Content-Type": "application/json",
              },
              body: JSON.stringify({
                editorDocument,
                workflow,
                workflowId,
              }),
            },
            "更新当前 edit session draft 失败",
          ),
      ),
    ],
  });
