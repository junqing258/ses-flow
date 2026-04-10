import { request as sendRequest } from "@/lib/request";

import type { RunnerWorkflowDefinition } from "./runner";
import type { PersistedWorkflowDocument } from "./persistence";

export interface WorkflowSummary {
  createdAt: string;
  name: string;
  ownerName: string | null;
  publishedAt: string | null;
  status: "draft" | "published";
  updatedAt: string;
  version: string;
  workflowId: string;
}

export interface WorkflowDetail extends WorkflowSummary {
  document: PersistedWorkflowDocument | null;
  workflow: RunnerWorkflowDefinition;
}

const WORKFLOW_API_BASE_URL = "/runner-api/workflows";

const parseResponse = async <T>(response: Response, fallbackMessage: string): Promise<T> => {
  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json") ? ((await response.json()) as Record<string, unknown>) : null;

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

export const fetchWorkflowDetail = async (workflowId: string): Promise<WorkflowDetail> => {
  const response = await sendRequest(`${WORKFLOW_API_BASE_URL}/${encodeURIComponent(workflowId)}`);
  return parseResponse<WorkflowDetail>(response, "获取工作流详情失败");
};
