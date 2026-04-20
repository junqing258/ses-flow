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
export const APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME =
  "apply_current_edit_session_draft_operations";
export const REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME =
  "remove_node_cascade_from_current_edit_session_draft";
export const RUNNER_TOOL_REQUEST_TIMEOUT_MS = 10_000;

type FetchLike = typeof fetch;

interface CreateRunnerEditSessionMcpServerParams {
  editSessionId: string;
  fetchImpl?: FetchLike;
  runnerBaseUrl: string;
}

export const normalizeJsonLikeInput = (value: unknown) => {
  if (typeof value !== "string") {
    return value;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return value;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return value;
  }
};

const normalizeRunnerBaseUrl = (runnerBaseUrl: string) =>
  runnerBaseUrl.trim().replace(/\/$/, "");

const buildRunnerUrl = (runnerBaseUrl: string, path: string) =>
  `${normalizeRunnerBaseUrl(runnerBaseUrl)}${path}`;

export const buildGetEditSessionPath = (
  editSessionId: string,
  options: {
    includeEditorDocument?: boolean;
  } = {},
) => {
  const searchParams = new URLSearchParams();

  if (options.includeEditorDocument != null) {
    searchParams.set(
      "includeEditorDocument",
      String(options.includeEditorDocument),
    );
  }

  const queryString = searchParams.toString();
  const basePath = `/edit-sessions/${encodeURIComponent(editSessionId)}`;

  return queryString ? `${basePath}?${queryString}` : basePath;
};

const REMOVE_NODE_CASCADE_OPERATION_TYPE = "remove_node_cascade";
const UPDATE_NODE_CONFIG_OPERATION_TYPE = "update_node_config";
const ADD_EDGE_OPERATION_TYPE = "add_edge";
const REMOVE_EDGE_OPERATION_TYPE = "remove_edge";
const UPDATE_EDGE_OPERATION_TYPE = "update_edge";

const editSessionDraftOperationSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal(REMOVE_NODE_CASCADE_OPERATION_TYPE),
    nodeId: z.string().min(1),
  }),
  z.object({
    type: z.literal(UPDATE_NODE_CONFIG_OPERATION_TYPE),
    nodeId: z.string().min(1),
    config: z.record(z.unknown()),
  }),
  z.object({
    type: z.literal(ADD_EDGE_OPERATION_TYPE),
    source: z.string().min(1),
    target: z.string().min(1),
    sourceHandle: z.string().optional(),
    targetHandle: z.string().optional(),
  }),
  z.object({
    type: z.literal(REMOVE_EDGE_OPERATION_TYPE),
    edgeId: z.string().min(1),
  }),
  z.object({
    type: z.literal(UPDATE_EDGE_OPERATION_TYPE),
    edgeId: z.string().min(1),
    updates: z.record(z.unknown()),
  }),
]);

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
  toolName === APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME ||
  toolName === REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
  toolName.startsWith(`mcp__${RUNNER_MCP_SERVER_NAME}__`);

export const isPreviewMutationToolName = (toolName: string) =>
  toolName === UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
  toolName === APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME ||
  toolName === REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
  toolName.endsWith(`__${UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`) ||
  toolName.endsWith(
    `__${APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME}`,
  ) ||
  toolName.endsWith(
    `__${REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`,
  );

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
        "读取当前 edit session。默认返回轻量结果（包含 workflow 和元信息，不含 editorDocument）；只有确实需要画布文档时才传 includeEditorDocument=true。",
        {
          includeEditorDocument: z.boolean().optional(),
        },
        async ({ includeEditorDocument = false }) =>
          callRunner(
            fetchImpl,
            buildRunnerUrl(
              runnerBaseUrl,
              buildGetEditSessionPath(editSessionId, {
                includeEditorDocument,
              }),
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
                editorDocument: normalizeJsonLikeInput(editorDocument),
                workflow: normalizeJsonLikeInput(workflow),
                workflowId,
              }),
            },
            "更新当前 edit session draft 失败",
          ),
      ),
      tool(
        APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME,
        "批量更新当前 edit session draft。优先使用 operations 一次完成多个修改；支持 remove_node_cascade、update_node_config、add_edge、remove_edge、update_edge。",
        {
          operations: z.array(editSessionDraftOperationSchema).min(1),
          workflowId: z.string().min(1).optional(),
        },
        async ({ operations, workflowId }) =>
          callRunner(
            fetchImpl,
            buildRunnerUrl(
              runnerBaseUrl,
              `/edit-sessions/${encodeURIComponent(editSessionId)}/draft`,
            ),
            {
              method: "PATCH",
              headers: {
                "Content-Type": "application/json",
              },
              body: JSON.stringify({
                operations,
                workflowId,
              }),
            },
            "批量更新当前 edit session draft 失败",
          ),
      ),
      tool(
        REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
        "删除当前 edit session draft 中的一个节点，并自动清理相关连线与 editorDocument 里的节点/边/panel。",
        {
          nodeId: z.string().min(1),
          workflowId: z.string().min(1).optional(),
        },
        async ({ nodeId, workflowId }) =>
          callRunner(
            fetchImpl,
            buildRunnerUrl(
              runnerBaseUrl,
              `/edit-sessions/${encodeURIComponent(editSessionId)}/draft`,
            ),
            {
              method: "PATCH",
              headers: {
                "Content-Type": "application/json",
              },
              body: JSON.stringify({
                operations: [
                  {
                    type: REMOVE_NODE_CASCADE_OPERATION_TYPE,
                    nodeId,
                  },
                ],
                workflowId,
              }),
            },
            "删除节点并级联更新当前 edit session draft 失败",
          ),
      ),
    ],
  });
