import { request as sendRequest } from "@/lib/request";

import type { RunnerWorkflowDefinition } from "./runner";
import type { PersistedWorkflowDocument } from "./persistence";
import type { WorkflowRunStatus } from "./runner";

export interface WorkflowSummary {
  createdAt: string;
  name: string;
  ownerName: string | null;
  publishedAt: string | null;
  runningRunCount: number;
  status: "draft" | "published";
  updatedAt: string;
  version: string;
  workflowId: string;
}

export interface WorkflowDetail extends WorkflowSummary {
  document: PersistedWorkflowDocument | null;
  workflow: RunnerWorkflowDefinition;
}

export interface WorkflowRunListItem {
  createdAt: string;
  currentNodeId?: string;
  runId: string;
  status: WorkflowRunStatus;
  updatedAt: string;
  workflowKey: string;
  workflowVersion: number;
}

export const RUNNER_BASE_URL = (
  import.meta.env.VITE_RUNNER_BASE_URL?.trim() || "/runner-api"
).replace(/\/$/, "");

const WORKFLOW_API_BASE_URL = RUNNER_BASE_URL + "/workflows";

const parseResponse = async <T>(
  response: Response,
  fallbackMessage: string,
): Promise<T> => {
  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json")
    ? ((await response.json()) as Record<string, unknown>)
    : null;

  if (!response.ok) {
    const errorMessage =
      (typeof payload?.message === "string" && payload.message) ||
      (typeof payload?.error === "string" && payload.error) ||
      fallbackMessage;

    throw new Error(errorMessage);
  }

  return payload as T;
};

export const fetchWorkflowList = async (): Promise<WorkflowSummary[]> => {
  const response = await sendRequest(WORKFLOW_API_BASE_URL);
  return parseResponse<WorkflowSummary[]>(response, "获取工作流列表失败");
};

export const fetchWorkflowDetail = async (
  workflowId: string,
): Promise<WorkflowDetail> => {
  const response = await sendRequest(
    `${WORKFLOW_API_BASE_URL}/${encodeURIComponent(workflowId)}`,
  );
  return parseResponse<WorkflowDetail>(response, "获取工作流详情失败");
};

export const fetchWorkflowRuns = async (
  workflowId: string,
): Promise<WorkflowRunListItem[]> => {
  const response = await sendRequest(
    `${WORKFLOW_API_BASE_URL}/${encodeURIComponent(workflowId)}/runs`,
  );
  return parseResponse<WorkflowRunListItem[]>(
    response,
    "获取工作流运行列表失败",
  );
};
