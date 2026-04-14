import { request as sendRequest } from "@/lib/request";

import type { PersistedWorkflowDocument } from "./persistence";
import type { RunnerWorkflowDefinition } from "./runner";

const RUNNER_BASE_URL = (
  import.meta.env.VITE_RUNNER_BASE_URL?.trim() || "/runner-api"
).replace(/\/$/, "");
const DEFAULT_WORKSPACE_ID = "ses-workflow-editor";

export interface WorkflowEditSession {
  createdAt: string;
  editorDocument: PersistedWorkflowDocument | null;
  sessionId: string;
  updatedAt: string;
  workflow: RunnerWorkflowDefinition;
  workflowId?: string;
  workspaceId: string;
}

export interface WorkflowEditSessionEvent {
  eventType: "created" | "snapshot" | "updated";
  session: WorkflowEditSession;
  sessionId: string;
}

export interface WorkflowEditSessionRequest {
  editorDocument?: PersistedWorkflowDocument;
  workflow: RunnerWorkflowDefinition;
  workflowId?: string;
  workspaceId?: string;
}

class SessionRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SessionRequestError";
  }
}

const parseSessionResponse = async <T>(
  response: Response,
  fallbackMessage: string,
): Promise<T> => {
  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json")
    ? ((await response.json()) as Record<string, unknown>)
    : null;

  if (!response.ok) {
    const errorMessage =
      (typeof payload?.error === "string" && payload.error) ||
      (typeof payload?.message === "string" && payload.message) ||
      fallbackMessage;

    throw new SessionRequestError(errorMessage);
  }

  return payload as T;
};

const buildRequestBody = (request: WorkflowEditSessionRequest) => ({
  editorDocument: request.editorDocument,
  workflow: request.workflow,
  workflowId: request.workflowId,
  workspaceId: request.workspaceId ?? DEFAULT_WORKSPACE_ID,
});

export const createWorkflowEditSession = async (
  request: WorkflowEditSessionRequest,
): Promise<WorkflowEditSession> => {
  let response: Response;

  try {
    response = await sendRequest(`${RUNNER_BASE_URL}/edit-sessions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(buildRequestBody(request)),
    });
  } catch {
    throw new SessionRequestError(
      "Runner 服务不可达，请确认本地 runner 已启动",
    );
  }

  return parseSessionResponse<WorkflowEditSession>(
    response,
    "创建 AI 编辑会话失败",
  );
};

export const updateWorkflowEditSession = async (
  sessionId: string,
  request: WorkflowEditSessionRequest,
): Promise<WorkflowEditSession> => {
  let response: Response;

  try {
    response = await sendRequest(
      `${RUNNER_BASE_URL}/edit-sessions/${encodeURIComponent(sessionId)}`,
      {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(buildRequestBody(request)),
      },
    );
  } catch {
    throw new SessionRequestError(
      "Runner 服务不可达，请确认本地 runner 已启动",
    );
  }

  return parseSessionResponse<WorkflowEditSession>(
    response,
    "同步 AI 编辑会话失败",
  );
};

export const buildWorkflowEditSessionWsUrl = (sessionId: string) => {
  const path = `${RUNNER_BASE_URL}/edit-sessions/${encodeURIComponent(sessionId)}/ws`;

  if (path.startsWith("http://") || path.startsWith("https://")) {
    return path.replace(/^http/, "ws");
  }

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}${path}`;
};
