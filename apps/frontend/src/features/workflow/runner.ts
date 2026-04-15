import type { Edge } from "@vue-flow/core";

import { request as sendRequest } from "@/lib/request";
import { RUNNER_BASE_URL } from "./api";

import {
  getSwitchBranches,
  getSwitchFallbackHandle,
  type WorkflowBranchHandle,
  WorkflowFlowNode,
  WorkflowNodePanel,
  WorkflowTabId,
} from "./model";
import type { PersistedWorkflowDocument } from "./persistence";

type WorkflowStatus = "draft" | "published";

interface RunnerTriggerDefinition {
  type: "manual" | "webhook";
  path?: string;
  responseMode?: "sync" | "async_ack";
}

interface RunnerRetryPolicy {
  max_attempts?: number;
}

type RunnerMappingValue = Record<string, unknown> | string | null;

interface RunnerNodeDefinition {
  id: string;
  type: string;
  name: string;
  config?: Record<string, unknown>;
  inputMapping?: RunnerMappingValue;
  outputMapping?: RunnerMappingValue;
  timeoutMs?: number;
  retryPolicy?: RunnerRetryPolicy;
  annotations?: Record<string, unknown>;
}

interface RunnerTransitionDefinition {
  from: string;
  to: string;
  label?: string;
  branchType?: "default";
  priority?: number;
}

export type WorkflowRunStatus =
  | "running"
  | "completed"
  | "waiting"
  | "failed"
  | "terminated";

export const shouldPollWorkflowRunSummary = (status: WorkflowRunStatus) =>
  status === "running" || status === "waiting";

export interface WorkflowExecutionRequest {
  env?: Record<string, unknown>;
  trigger?: Record<string, unknown>;
}

export interface WorkflowExecutionAccepted {
  runId: string;
  status: string;
  statusUrl: string;
  workflowId?: string;
}

export interface WorkflowRunTimelineItem {
  branchKey?: string;
  logs?: Array<{
    level: string;
    message: string;
  }>;
  nodeId: string;
  nodeType: string;
  output: unknown;
  statePatch: unknown;
  status: string;
}

export interface WorkflowRunSummary {
  currentNodeId?: string;
  lastSignal?: unknown;
  resumeState?: unknown;
  runId: string;
  state: unknown;
  status: WorkflowRunStatus;
  timeline: WorkflowRunTimelineItem[];
  workflowKey: string;
  workflowVersion: number;
}

export interface RunnerWorkflowDefinition {
  meta: {
    key: string;
    name: string;
    version: number;
    status: WorkflowStatus;
  };
  trigger: RunnerTriggerDefinition;
  inputSchema: {
    type: "object";
  };
  nodes: RunnerNodeDefinition[];
  transitions: RunnerTransitionDefinition[];
  policies: {
    allowManualRetry: boolean;
  };
}

export interface PublishWorkflowOptions {
  editorDocument?: PersistedWorkflowDocument;
  persistedWorkflowId?: string;
  workflowId: string;
  workflowName: string;
  workflowVersion: string;
  workflowStatus?: WorkflowStatus;
}

export interface RunnerWorkflowRegistration {
  workspaceId: string;
  workflowId: string;
  workflowKey: string;
  workflowVersion: number;
}

class RunnerRequestError extends Error {
  status: number | null;

  constructor(message: string, status: number | null = null) {
    super(message);
    this.name = "RunnerRequestError";
    this.status = status;
  }
}

const DEFAULT_WORKSPACE_ID = "ses-workflow-editor";
const DEFAULT_WORKSPACE_NAME = "SES Workflow Editor";

const parseRunnerResponse = async <T>(
  response: Response,
  fallbackMessage: string,
): Promise<T> => {
  const contentType = response.headers.get("content-type") ?? "";
  const hasJsonBody = contentType.includes("application/json");
  const payload = hasJsonBody
    ? ((await response.json()) as Record<string, unknown>)
    : null;

  if (!response.ok) {
    const errorMessage =
      (typeof payload?.error === "string" && payload.error) ||
      (typeof payload?.message === "string" && payload.message) ||
      fallbackMessage;

    throw new RunnerRequestError(errorMessage, response.status);
  }

  return payload as T;
};

const getFieldValue = (
  panel: WorkflowNodePanel | undefined,
  tab: WorkflowTabId,
  fieldKey: string,
) =>
  panel?.fieldsByTab[tab]
    ?.find((field) => field.key === fieldKey)
    ?.value.trim() ?? "";

const parsePositiveInteger = (value: string) => {
  const parsed = Number.parseInt(value.replace(/[^\d]/g, ""), 10);

  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
};

const normalizeReferencePath = (rawValue: string) => {
  const value = rawValue.trim();

  if (!value) {
    return value;
  }

  if (value.startsWith("{{") && value.endsWith("}}")) {
    return value;
  }

  if (
    /^['"].*['"]$/.test(value) ||
    /^(true|false|null|-?\d+(\.\d+)?)$/i.test(value)
  ) {
    return value;
  }

  let normalized = value
    .replace(/^trigger\.payload\./, "trigger.body.")
    .replace(/^payload\./, "input.")
    .replace(/^response\.data\./, "input.data.")
    .replace(/^response\./, "input.");

  if (/^(body|headers)\./.test(normalized)) {
    normalized = `trigger.${normalized}`;
  }

  if (/^(trigger|input|state|env)\./.test(normalized)) {
    return `{{${normalized}}}`;
  }

  return value;
};

const parseScalarValue = (rawValue: string): unknown => {
  const value = rawValue.trim();

  if (!value) {
    return "";
  }

  if (
    (value.startsWith("'") && value.endsWith("'")) ||
    (value.startsWith('"') && value.endsWith('"'))
  ) {
    return value.slice(1, -1);
  }

  if (value === "true") {
    return true;
  }

  if (value === "false") {
    return false;
  }

  if (value === "null") {
    return null;
  }

  if (/^-?\d+(\.\d+)?$/.test(value)) {
    return Number(value);
  }

  return normalizeReferencePath(value);
};

const parseLooseObjectLiteral = (
  rawValue: string,
): Record<string, unknown> | null => {
  const trimmed = rawValue.trim();

  if (!trimmed) {
    return null;
  }

  if (!(trimmed.startsWith("{") && trimmed.endsWith("}"))) {
    return null;
  }

  const lines = trimmed
    .slice(1, -1)
    .split("\n")
    .map((line) => line.trim().replace(/,$/, ""))
    .filter(Boolean);

  const entries = lines
    .map((line) => {
      const separatorIndex = line.indexOf(":");

      if (separatorIndex <= 0) {
        return null;
      }

      const key = line
        .slice(0, separatorIndex)
        .trim()
        .replace(/^['"]|['"]$/g, "");
      const value = line.slice(separatorIndex + 1).trim();

      if (!key) {
        return null;
      }

      return [key, parseScalarValue(value)] as const;
    })
    .filter((entry): entry is readonly [string, unknown] => entry !== null);

  return entries.length > 0 ? Object.fromEntries(entries) : null;
};

const parseMappingValue = (rawValue: string): RunnerMappingValue => {
  const objectValue = parseLooseObjectLiteral(rawValue);

  if (objectValue) {
    return objectValue;
  }

  const scalarValue = parseScalarValue(rawValue);

  if (typeof scalarValue === "string" || scalarValue === null) {
    return scalarValue;
  }

  return JSON.stringify(scalarValue);
};

const parseObjectValue = (
  rawValue: string,
): Record<string, unknown> | undefined => {
  const objectValue = parseLooseObjectLiteral(rawValue);

  if (objectValue) {
    return objectValue;
  }

  const trimmed = rawValue.trim();
  if (!trimmed) {
    return undefined;
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    return undefined;
  }

  return undefined;
};

const normalizeFetchMethod = (rawValue: string) => {
  const value = rawValue.trim().toUpperCase();

  if (value === "POST") {
    return "POST";
  }

  return "GET";
};

const normalizeExpression = (rawValue: string, fallback = "default") => {
  const value = rawValue.trim();

  if (!value) {
    return fallback;
  }

  return normalizeReferencePath(value);
};

const resolveSwitchBranch = (
  panel: WorkflowNodePanel | undefined,
  sourceHandle?: string | null,
) =>
  getSwitchBranches(panel).find((branch) => branch.id === sourceHandle);

const getSwitchBranchPriority = (
  branches: WorkflowBranchHandle[],
  branchId?: string | null,
) => {
  const branchIndex = branches.findIndex((branch) => branch.id === branchId);

  if (branchIndex < 0) {
    return 1;
  }

  return Math.max(100 - branchIndex * 10, 10);
};

const extractNodeType = (node: WorkflowFlowNode) => {
  if (node.data.kind === "start") {
    return "start";
  }

  if (node.data.kind === "end") {
    return "end";
  }

  if (node.data.kind === "fetch") {
    return "fetch";
  }

  if (node.data.kind === "wait") {
    return "wait";
  }

  if (node.data.kind === "switch") {
    return "switch";
  }

  if (node.data.kind === "if-else") {
    return "if_else";
  }

  if (node.data.title === "Webhook Trigger") {
    return "webhook_trigger";
  }

  if (node.data.title === "Respond") {
    return "respond";
  }

  if (node.data.title === "Task") {
    return "task";
  }

  if (node.data.title === "Sub-Workflow") {
    return "sub_workflow";
  }

  if (node.data.title === "Code") {
    return "code";
  }

  return "shell";
};

const buildNodeDefinition = (
  node: WorkflowFlowNode,
  panel: WorkflowNodePanel | undefined,
): RunnerNodeDefinition => {
  const type = extractNodeType(node);
  const timeoutMs = parsePositiveInteger(
    getFieldValue(panel, "base", "timeout"),
  );
  const maxAttempts =
    parsePositiveInteger(getFieldValue(panel, "retry", "maxAttempts")) ??
    parsePositiveInteger(getFieldValue(panel, "retry", "retryCount"));

  const definition: RunnerNodeDefinition = {
    id: node.id,
    name:
      getFieldValue(panel, "base", "nodeName") ||
      node.data.subtitle ||
      node.data.title,
    type,
    annotations: {
      editorPosition: node.position,
      note: getFieldValue(panel, "base", "note") || undefined,
    },
  };

  if (timeoutMs !== undefined) {
    definition.timeoutMs = timeoutMs;
  }

  if (maxAttempts !== undefined) {
    definition.retryPolicy = {
      max_attempts: maxAttempts,
    };
  }

  if (type === "fetch") {
    definition.config = {
      method: normalizeFetchMethod(getFieldValue(panel, "base", "method")),
      url:
        getFieldValue(panel, "base", "url") ||
        "https://jsonplaceholder.typicode.com/todos",
    };
    const headers = parseObjectValue(getFieldValue(panel, "base", "headers"));
    if (headers && Object.keys(headers).length > 0) {
      definition.config.headers = headers;
    }
    definition.inputMapping = parseMappingValue(
      getFieldValue(panel, "mapping", "inputFrom"),
    );
  }

  if (type === "switch" || type === "if_else") {
    definition.config = {
      expression: normalizeExpression(
        getFieldValue(panel, "base", "expression"),
      ),
    };
  }

  if (type === "switch") {
    const switchBranches = getSwitchBranches(panel);
    const defaultBranchHandle = getSwitchFallbackHandle(panel);

    definition.annotations = {
      ...definition.annotations,
      defaultBranchHandle,
      switchBranches: switchBranches.map((branch) => ({
        id: branch.id,
        label: branch.label,
      })),
    };
  }

  if (type === "shell") {
    definition.config = {
      command:
        getFieldValue(panel, "base", "command") ||
        "printf '%s' \"$WORKFLOW_PARAMS\"",
    };
    const shell = getFieldValue(panel, "base", "shell");
    const workingDirectory = getFieldValue(panel, "base", "workingDirectory");
    if (shell) {
      definition.config.shell = shell;
    }
    if (workingDirectory) {
      definition.config.workingDirectory = workingDirectory;
    }
    definition.inputMapping = parseMappingValue(
      getFieldValue(panel, "mapping", "payload"),
    );
  }

  if (type === "code") {
    definition.config = {
      language:
        getFieldValue(panel, "base", "language") || "javascript",
      source:
        getFieldValue(panel, "base", "source") ||
        "return { output: params };",
    };
    definition.inputMapping = parseMappingValue(
      getFieldValue(panel, "mapping", "payload"),
    );
  }

  if (type === "task") {
    definition.config = {
      taskType: getFieldValue(panel, "base", "command") || "task.unknown",
      completeEvent: "task.completed",
    };
    definition.inputMapping = parseMappingValue(
      getFieldValue(panel, "mapping", "payload"),
    );
  }

  if (type === "respond") {
    definition.config = {
      statusCode: 200,
    };
    definition.inputMapping = parseMappingValue(
      getFieldValue(panel, "mapping", "payload"),
    );
  }

  if (type === "sub_workflow") {
    definition.config = {
      workflowKey: getFieldValue(panel, "base", "command") || undefined,
    };
    definition.inputMapping = parseMappingValue(
      getFieldValue(panel, "mapping", "payload"),
    );
  }

  if (type === "wait") {
    definition.config = {
      event: getFieldValue(panel, "base", "waitEvent") || "event.unknown",
    };
  }

  if (type === "webhook_trigger") {
    definition.config = {
      mode: "body",
    };
  }

  return definition;
};

const buildTransitions = (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
): RunnerTransitionDefinition[] =>
  edges.map((edge, index) => {
    const sourceNode = nodes.find((node) => node.id === edge.source);
    const sourcePanel = panelByNodeId[edge.source];

    if (sourceNode?.data.kind === "if-else") {
      const label =
        edge.sourceHandle === "branch-a"
          ? "then"
          : edge.sourceHandle === "branch-b"
            ? "else"
            : undefined;
      return {
        from: edge.source,
        to: edge.target,
        label,
        priority: label === "then" ? 100 : label === "else" ? 90 : 50 - index,
      };
    }

    if (sourceNode?.data.kind === "switch") {
      const branches = getSwitchBranches(sourcePanel);
      const branch = resolveSwitchBranch(sourcePanel, edge.sourceHandle);
      const defaultHandle = getSwitchFallbackHandle(sourcePanel);

      if (edge.sourceHandle && defaultHandle === edge.sourceHandle) {
        return {
          from: edge.source,
          to: edge.target,
          branchType: "default",
          priority: 1,
        };
      }

      if (branch) {
        return {
          from: edge.source,
          to: edge.target,
          label: branch.label,
          priority: getSwitchBranchPriority(branches, branch.id),
        };
      }

      return {
        from: edge.source,
        to: edge.target,
        priority: 1,
      };
    }

    return {
      from: edge.source,
      to: edge.target,
    };
  });

const buildWorkflowTrigger = (
  nodes: WorkflowFlowNode[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
): RunnerTriggerDefinition => {
  const webhookNode = nodes.find(
    (node) => extractNodeType(node) === "webhook_trigger",
  );

  if (!webhookNode) {
    return {
      type: "manual",
    };
  }

  const panel = panelByNodeId[webhookNode.id];
  const path =
    getFieldValue(panel, "base", "path") || `/workflows/${webhookNode.id}`;
  const responseMode =
    getFieldValue(panel, "base", "responseMode") || "async_ack";

  return {
    type: "webhook",
    path,
    responseMode:
      responseMode === "sync" || responseMode === "async_ack"
        ? responseMode
        : "async_ack",
  };
};

export const buildRunnerWorkflowDefinition = (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
  options: PublishWorkflowOptions,
): RunnerWorkflowDefinition => ({
  meta: {
    key: options.workflowId,
    name: options.workflowName,
    version: parsePositiveInteger(options.workflowVersion) ?? 1,
    status: options.workflowStatus ?? "published",
  },
  trigger: buildWorkflowTrigger(nodes, panelByNodeId),
  inputSchema: {
    type: "object",
  },
  nodes: nodes
    .filter((node) => node.data.kind !== "branch-label")
    .map((node) => buildNodeDefinition(node, panelByNodeId[node.id])),
  transitions: buildTransitions(nodes, edges, panelByNodeId),
  policies: {
    allowManualRetry: true,
  },
});

export const syncWorkflowToRunner = async (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
  options: PublishWorkflowOptions,
): Promise<RunnerWorkflowRegistration> => {
  const workflow = buildRunnerWorkflowDefinition(nodes, edges, panelByNodeId, {
    ...options,
    workflowStatus: options.workflowStatus ?? "draft",
  });

  let response: Response;

  try {
    response = await sendRequest(`${RUNNER_BASE_URL}/workflows`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        workspaceId: DEFAULT_WORKSPACE_ID,
        workspaceName: DEFAULT_WORKSPACE_NAME,
        workflowId: options.persistedWorkflowId,
        editorDocument: options.editorDocument,
        workflow,
      }),
    });
  } catch {
    throw new RunnerRequestError("Runner 服务不可达，请确认本地 runner 已启动");
  }

  return parseRunnerResponse<RunnerWorkflowRegistration>(
    response,
    "同步工作流到 Runner 失败",
  );
};

export const publishWorkflowToRunner = async (
  nodes: WorkflowFlowNode[],
  edges: Edge[],
  panelByNodeId: Record<string, WorkflowNodePanel>,
  options: PublishWorkflowOptions,
): Promise<RunnerWorkflowRegistration> =>
  syncWorkflowToRunner(nodes, edges, panelByNodeId, {
    ...options,
    workflowStatus: "published",
  });

export const executeWorkflowRun = async (
  workflowId: string,
  request: WorkflowExecutionRequest,
): Promise<WorkflowExecutionAccepted> => {
  let response: Response;

  try {
    response = await sendRequest(
      `${RUNNER_BASE_URL}/workflows/${encodeURIComponent(workflowId)}/run`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(request),
      },
    );
  } catch {
    throw new RunnerRequestError("Runner 服务不可达，请确认本地 runner 已启动");
  }

  return parseRunnerResponse<WorkflowExecutionAccepted>(
    response,
    "启动工作流运行失败",
  );
};

export const fetchWorkflowRunSummary = async (
  runId: string,
): Promise<WorkflowRunSummary> => {
  let response: Response;

  try {
    response = await sendRequest(
      `${RUNNER_BASE_URL}/runs/${encodeURIComponent(runId)}`,
    );
  } catch {
    throw new RunnerRequestError("Runner 服务不可达，请确认本地 runner 已启动");
  }

  return parseRunnerResponse<WorkflowRunSummary>(response, "获取运行状态失败");
};

export const terminateWorkflowRun = async (
  runId: string,
): Promise<WorkflowRunSummary> => {
  let response: Response;

  try {
    response = await sendRequest(
      `${RUNNER_BASE_URL}/runs/${encodeURIComponent(runId)}/terminate`,
      {
        method: "POST",
      },
    );
  } catch {
    throw new RunnerRequestError("Runner 服务不可达，请确认本地 runner 已启动");
  }

  return parseRunnerResponse<WorkflowRunSummary>(
    response,
    "终止工作流运行失败",
  );
};
