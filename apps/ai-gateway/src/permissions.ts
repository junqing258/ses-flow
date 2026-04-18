import { isRunnerMcpToolName } from "./runner-tools.js";

export const isAllowedToolUse = (
  toolName: string,
  _input: unknown,
  _runnerBaseUrl: string,
) =>
  isRunnerMcpToolName(toolName);
