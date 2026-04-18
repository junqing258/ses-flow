import { describe, expect, it } from "vitest";

import {
  GET_CURRENT_EDIT_SESSION_TOOL_NAME,
  isPreviewMutationToolName,
  isRunnerMcpToolName,
  RUNNER_MCP_SERVER_NAME,
  RUNNER_TOOL_REQUEST_TIMEOUT_MS,
  UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
} from "../src/runner-tools.js";

describe("runner MCP tools", () => {
  it("recognizes allowed runner MCP tool names", () => {
    expect(isRunnerMcpToolName(GET_CURRENT_EDIT_SESSION_TOOL_NAME)).toBe(true);
    expect(
      isRunnerMcpToolName(UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME),
    ).toBe(true);
    expect(
      isRunnerMcpToolName(
        `mcp__${RUNNER_MCP_SERVER_NAME}__${GET_CURRENT_EDIT_SESSION_TOOL_NAME}`,
      ),
    ).toBe(true);
    expect(isRunnerMcpToolName("Bash")).toBe(false);
  });

  it("marks only draft update tools as preview mutations", () => {
    expect(
      isPreviewMutationToolName(UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME),
    ).toBe(true);
    expect(
      isPreviewMutationToolName(
        `mcp__${RUNNER_MCP_SERVER_NAME}__${UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME}`,
      ),
    ).toBe(true);
    expect(isPreviewMutationToolName(GET_CURRENT_EDIT_SESSION_TOOL_NAME)).toBe(
      false,
    );
  });

  it("uses a 10 second timeout for runner tool requests", () => {
    expect(RUNNER_TOOL_REQUEST_TIMEOUT_MS).toBe(10_000);
  });
});
