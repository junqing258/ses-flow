import {
  query,
  type PermissionResult,
  type SDKMessage,
} from "@anthropic-ai/claude-agent-sdk";

import { isAllowedToolUse } from "./permissions.js";
import { resolveAiProviderConfig } from "./config.js";
import {
  createRunnerEditSessionMcpServer,
  GET_CURRENT_EDIT_SESSION_TOOL_NAME,
  isPreviewMutationToolName,
  RUNNER_MCP_SERVER_NAME,
  UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
} from "./runner-tools.js";
import type { AiProviderConfig } from "./types.js";

export interface ClaudeTurnCallbacks {
  onAssistantDelta: (delta: string) => void;
  onAssistantCompleted: () => void;
  onClaudeSessionId: (sessionId: string) => void;
  onPreviewUpdated: () => void;
  onToolStarted: (toolCallId: string, toolName: string, content: string) => void;
  onToolCompleted: (toolCallId: string, content?: string) => void;
}

export interface RunClaudeTurnParams extends ClaudeTurnCallbacks {
  abortController: AbortController;
  aiProvider: AiProviderConfig;
  claudeSessionId?: string;
  editSessionId: string;
  prompt: string;
  repoRoot: string;
  runnerBaseUrl: string;
  workflowId?: string;
}

export interface ClaudeAdapter {
  runTurn(params: RunClaudeTurnParams): Promise<void>;
}

const RUNNER_MCP_ALLOWED_TOOL_NAMES = [
  GET_CURRENT_EDIT_SESSION_TOOL_NAME,
  UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
  `mcp__${RUNNER_MCP_SERVER_NAME}__${GET_CURRENT_EDIT_SESSION_TOOL_NAME}`,
  `mcp__${RUNNER_MCP_SERVER_NAME}__${UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`,
];

const SYSTEM_PROMPT = `你是 SES Flow 页面内 AI 协作助手。

必须遵守以下规则：
1. 必须使用 ses-flow-skill。
2. runner edit session 是唯一事实来源。
3. 只能读取并更新当前 edit session 对应的草稿。
4. 不允许读取仓库源码、搜索仓库文件、修改仓库文件、提交代码或运行任何写文件命令。
5. 不要为当前任务分析或排查 MCP/SDK/后端实现细节；若工具报错，应根据错误直接重试或向用户简要说明失败原因。
6. 若要读取或修改工作流，只能使用提供的 ses-flow-runner MCP 工具。
6. ses-flow-runner MCP 工具已经预授权，不需要向用户申请额外权限。
7. 对于“删除节点/改连线/改配置”等直接编辑需求，优先在 1 次读取后直接更新草稿，不要进行额外探索。
8. 回复末尾必须给出“本次改动摘要”。`;

const getText = (value: unknown): string => {
  if (typeof value === "string") {
    return value;
  }

  if (Array.isArray(value)) {
    return value.map((item) => getText(item)).join("");
  }

  if (typeof value === "object" && value !== null) {
    if ("text" in value && typeof value.text === "string") {
      return value.text;
    }

    if ("content" in value) {
      return getText(value.content);
    }
  }

  return "";
};

const getAssistantTextFromMessage = (message: SDKMessage) => {
  if (message.type !== "assistant") {
    return "";
  }

  return message.message.content
    .map((block: { type: string; text?: string }) =>
      block.type === "text" ? (block.text ?? "") : "",
    )
    .join("");
};

const getToolUseBlocks = (message: SDKMessage) => {
  if (message.type !== "assistant") {
    return [];
  }

  return message.message.content.filter(block => block.type === "tool_use") as {
    type: "tool_use";
    id: string;
    name: string;
    input: Record<string, unknown>;
  }[];
};

const getToolResultText = (message: SDKMessage) => {
  if (message.type !== "user" || !message.isSynthetic) {
    return "";
  }

  return getText(message.tool_use_result ?? message.message.content);
};

const parseJsonObject = (value: string) => {
  if (!value) {
    return null;
  }

  try {
    const parsed = JSON.parse(value);
    return typeof parsed === "object" && parsed !== null
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
};

const getToolUseSummary = (
  toolName: string,
  input: Record<string, unknown>,
) => {
  if (
    toolName === GET_CURRENT_EDIT_SESSION_TOOL_NAME ||
    toolName.endsWith(`__${GET_CURRENT_EDIT_SESSION_TOOL_NAME}`)
  ) {
    return "读取当前 edit session";
  }

  if (
    toolName === UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
    toolName.endsWith(`__${UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`)
  ) {
    return "更新当前 edit session draft";
  }

  return `${toolName}: ${getText(input) || "执行中"}`;
};

const getToolCompletedSummary = (
  toolName: string,
  resultText: string,
) => {
  if (
    toolName === GET_CURRENT_EDIT_SESSION_TOOL_NAME ||
    toolName.endsWith(`__${GET_CURRENT_EDIT_SESSION_TOOL_NAME}`)
  ) {
    const payload = parseJsonObject(resultText);
    const workflowId =
      typeof payload?.workflowId === "string" ? payload.workflowId : "";
    const workflow =
      typeof payload?.workflow === "object" && payload.workflow !== null
        ? (payload.workflow as Record<string, unknown>)
        : null;
    const nodes = Array.isArray(workflow?.nodes) ? workflow.nodes.length : null;

    if (workflowId && nodes != null) {
      return `已读取当前 edit session（workflowId: ${workflowId}，nodes: ${nodes}）`;
    }

    return "已读取当前 edit session";
  }

  if (
    toolName === UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME ||
    toolName.endsWith(`__${UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`)
  ) {
    const payload = parseJsonObject(resultText);
    const updatedAt =
      typeof payload?.updatedAt === "string" ? payload.updatedAt : "";

    if (updatedAt) {
      return `已更新当前 edit session draft（updatedAt: ${updatedAt}）`;
    }

    return "已更新当前 edit session draft";
  }

  return resultText ? `工具已完成：${resultText}` : "工具已完成";
};

const allowToolUse = (input: Record<string, unknown>): PermissionResult => ({
  behavior: "allow",
  updatedInput: input,
});

const denyToolUse = (message: string): PermissionResult => ({
  behavior: "deny",
  message,
});

export class ClaudeCodeSdkAdapter implements ClaudeAdapter {
  async runTurn(params: RunClaudeTurnParams) {
    const config = resolveAiProviderConfig(params.aiProvider);

    const prompt = `${SYSTEM_PROMPT}

runner_base_url: ${params.runnerBaseUrl}
session_id: ${params.editSessionId}
${params.workflowId ? `workflow_id: ${params.workflowId}` : ""}

用户需求：
${params.prompt}`.trim();

    const toolUseInputs = new Map<
      string,
      { input: Record<string, unknown>; toolName: string }
    >();
    let streamedAssistantText = false;

    const iterator = query({
      prompt,
      options: {
        cwd: params.repoRoot,
        permissionMode: "dontAsk",
        tools: [],
        allowedTools: RUNNER_MCP_ALLOWED_TOOL_NAMES,
        maxTurns: 6,
        mcpServers: {
          [RUNNER_MCP_SERVER_NAME]: createRunnerEditSessionMcpServer({
            editSessionId: params.editSessionId,
            runnerBaseUrl: params.runnerBaseUrl,
          }),
        },
        settingSources: ["project", "local"],
        includePartialMessages: true,
        resume: params.claudeSessionId || undefined,
        abortController: params.abortController,
        pathToClaudeCodeExecutable: config.claudeCodeExecutable,
        systemPrompt: {
          type: "preset",
          preset: "claude_code",
          append: SYSTEM_PROMPT,
        },
        model: config.model,
        env: {
          ...process.env,
          ANTHROPIC_API_KEY: config.authToken,
          ANTHROPIC_BASE_URL: config.baseUrl,
        },
        canUseTool: async (toolName, input) => {
          if (isAllowedToolUse(toolName, input, params.runnerBaseUrl)) {
            return allowToolUse(input);
          }

          return denyToolUse(
            `禁止使用 ${toolName} 执行当前操作。只能使用 ses-flow-runner MCP 工具访问当前 runner edit session。`,
          );
        },
      },
    });

    for await (const item of iterator) {
      if (item.session_id) {
        params.onClaudeSessionId(item.session_id);
      }

      if (item.type === "stream_event") {
        if (
          item.event.type !== "content_block_delta" ||
          item.event.delta.type !== "text_delta"
        ) {
          continue;
        }

        const delta = item.event.delta.text;
        if (delta) {
          streamedAssistantText = true;
          params.onAssistantDelta(delta);
        }
        continue;
      }

      if (item.type === "assistant") {
        if (!streamedAssistantText) {
          const text = getAssistantTextFromMessage(item);
          if (text) {
            params.onAssistantDelta(text);
          }
        }

        for (const toolUse of getToolUseBlocks(item)) {
          toolUseInputs.set(toolUse.id, {
            input: toolUse.input,
            toolName: toolUse.name,
          });
          params.onToolStarted(
            toolUse.id,
            toolUse.name,
            getToolUseSummary(toolUse.name, toolUse.input),
          );
        }
        continue;
      }

      if (item.type === "tool_progress") {
        if (!toolUseInputs.has(item.tool_use_id)) {
          params.onToolStarted(
            item.tool_use_id,
            item.tool_name,
            `${item.tool_name}: 执行中`,
          );
        }
        continue;
      }

      if (item.type === "user" && item.parent_tool_use_id) {
        const toolCall = toolUseInputs.get(item.parent_tool_use_id);
        if (toolCall && isPreviewMutationToolName(toolCall.toolName)) {
          params.onPreviewUpdated();
        }

        const resultText = getToolResultText(item);
        params.onToolCompleted(
          item.parent_tool_use_id,
          toolCall
            ? getToolCompletedSummary(toolCall.toolName, resultText)
            : resultText
              ? `工具已完成：${resultText}`
              : "工具已完成",
        );
        continue;
      }

      if (item.type === "result") {
        if (item.subtype !== "success" || item.is_error) {
          const errors =
            "errors" in item && Array.isArray(item.errors) ? item.errors : [];
          throw new Error(errors.join("\n") || "Claude 协作执行失败");
        }

        params.onAssistantCompleted();
      }
    }
  }
}
