import { request as sendRequest } from "@/lib/request";

import type { RunnerWorkflowDefinition } from "./runner";
import type { PersistedWorkflowDocument } from "./persistence";
import type { WorkflowRunStatus } from "./runner";
import type { WorkflowNodeDescriptor } from "./model";

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
  workflowKey: string;
  workflowVersion: number;
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

export const getRunnerBaseUrl = () => {
  const baseUrl = import.meta.env.VITE_RUNNER_BASE_URL ?? "/runner-api";
  if (/^https?:\/\//.test(baseUrl)) {
    return baseUrl.replace(/\/$/, "");
  } else {
    const origin =
      typeof globalThis.location?.origin === "string"
        ? globalThis.location.origin
        : "http://localhost:6302";
    return `${origin.replace(/\/$/, "")}/${baseUrl.replace(/^\//, "").replace(/\/$/, "")}`;
  }
};

export const RUNNER_BASE_URL = getRunnerBaseUrl();

const CATALOG_API_BASE_URL = RUNNER_BASE_URL + "/catalog";
const NODE_DESCRIPTOR_API_BASE_URL = RUNNER_BASE_URL + "/node-descriptors";
const SYSTEM_API_BASE_URL = RUNNER_BASE_URL + "/system";
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

export const refreshWorkflowCatalog = async (): Promise<void> => {
  const response = await sendRequest(`${CATALOG_API_BASE_URL}/refresh`);
  await parseResponse<{ status: string }>(response, "刷新工作流目录失败");
};

export const fetchNodeDescriptors = async (): Promise<
  WorkflowNodeDescriptor[]
> => {
  const response = await sendRequest(NODE_DESCRIPTOR_API_BASE_URL);
  return parseResponse<WorkflowNodeDescriptor[]>(
    response,
    "获取动态节点列表失败",
  );
};

export interface PluginAutoRegistrationConfig {
  baseUrls: string[];
}

export interface UpdatePluginAutoRegistrationResponse extends PluginAutoRegistrationConfig {
  descriptors: WorkflowNodeDescriptor[];
}

export const fetchPluginAutoRegistrationConfig =
  async (): Promise<PluginAutoRegistrationConfig> => {
    const response = await sendRequest(
      `${SYSTEM_API_BASE_URL}/plugin-auto-registration`,
    );
    return parseResponse<PluginAutoRegistrationConfig>(
      response,
      "获取插件自动注册配置失败",
    );
  };

export const updatePluginAutoRegistrationConfig = async (
  payload: PluginAutoRegistrationConfig,
): Promise<UpdatePluginAutoRegistrationResponse> => {
  const response = await sendRequest(
    `${SYSTEM_API_BASE_URL}/plugin-auto-registration`,
    {
      method: "PUT",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify(payload),
    },
  );
  return parseResponse<UpdatePluginAutoRegistrationResponse>(
    response,
    "保存插件自动注册配置失败",
  );
};
