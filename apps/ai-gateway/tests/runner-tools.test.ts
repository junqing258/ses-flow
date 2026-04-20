import { describe, expect, it } from "vitest";

import {
  APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME,
  GET_CURRENT_EDIT_SESSION_TOOL_NAME,
  isPreviewMutationToolName,
  isRunnerMcpToolName,
  normalizeJsonLikeInput,
  REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
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
        APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME,
      ),
    ).toBe(true);
    expect(
      isRunnerMcpToolName(
        REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
      ),
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
        APPLY_CURRENT_EDIT_SESSION_DRAFT_OPERATIONS_TOOL_NAME,
      ),
    ).toBe(true);
    expect(
      isPreviewMutationToolName(
        REMOVE_NODE_CASCADE_FROM_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
      ),
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

  it("parses stringified JSON tool inputs before sending to runner", () => {
    expect(normalizeJsonLikeInput("{\"nodes\":[]}")).toEqual({ nodes: [] });
    expect(normalizeJsonLikeInput("plain-text")).toBe("plain-text");
    expect(normalizeJsonLikeInput({ nodes: [] })).toEqual({ nodes: [] });
  });
});
