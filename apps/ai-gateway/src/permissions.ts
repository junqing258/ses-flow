import { isRunnerMcpToolName } from "./runner-tools.js";

const READ_ONLY_TOOLS = new Set(["Read", "Glob", "Grep", "LS"]);

export const isAllowedToolUse = (
  toolName: string,
  _input: unknown,
  _runnerBaseUrl: string,
) =>
  READ_ONLY_TOOLS.has(toolName) || isRunnerMcpToolName(toolName);
