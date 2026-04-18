import { describe, expect, it, vi } from "vitest";

import type { ClaudeAdapter, RunClaudeTurnParams } from "../src/claude.js";
import { isAllowedToolUse } from "../src/permissions.js";
import {
  GET_CURRENT_EDIT_SESSION_TOOL_NAME,
  RUNNER_MCP_SERVER_NAME,
  UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
} from "../src/runner-tools.js";
import {
  AiGatewayServiceError,
  createAiGatewayService,
} from "../src/service.js";

class MockClaudeAdapter implements ClaudeAdapter {
  constructor(private readonly handler: (params: RunClaudeTurnParams) => Promise<void>) {}

  runTurn(params: RunClaudeTurnParams) {
    return this.handler(params);
  }
}

describe("ai gateway permissions", () => {
  it("allows only whitelisted tools and runner MCP tools", () => {
    expect(
      isAllowedToolUse("Read", {}, "http://127.0.0.1:6302/runner-api"),
    ).toBe(true);
    expect(
      isAllowedToolUse(
        `mcp__${RUNNER_MCP_SERVER_NAME}__${GET_CURRENT_EDIT_SESSION_TOOL_NAME}`,
        {},
        "http://127.0.0.1:6302/runner-api",
      ),
    ).toBe(true);
    expect(
      isAllowedToolUse(
        UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
        {
          workflow: {
            nodes: [],
          },
        },
        "http://127.0.0.1:6302/runner-api",
      ),
    ).toBe(true);
    expect(
      isAllowedToolUse(
        "Bash",
        { command: "curl http://127.0.0.1:6302/runner-api/edit-sessions/abc" },
        "http://127.0.0.1:6302/runner-api",
      ),
    ).toBe(false);
    expect(
      isAllowedToolUse(
        "Write",
        {},
        "http://127.0.0.1:6302/runner-api",
      ),
    ).toBe(false);
  });
});

describe("ai gateway service", () => {
  it("returns 409 when a thread already has a running turn", async () => {
    const deferred = Promise.withResolvers<void>();
    const service = createAiGatewayService({
      claudeAdapter: new MockClaudeAdapter(async () => {
        await deferred.promise;
      }),
      repoRoot: process.cwd(),
    });

    await service.sendMessage("session-1", {
      message: "first",
      runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
    });

    await expect(
      service.sendMessage("session-1", {
        message: "second",
        runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
      }),
    ).rejects.toMatchObject<AiGatewayServiceError>({
      statusCode: 409,
    });

    deferred.resolve();
  });

  it("reuses an existing claude session id on the next turn", async () => {
    const runTurn = vi
      .fn<(params: RunClaudeTurnParams) => Promise<void>>()
      .mockImplementationOnce(async (params) => {
        params.onClaudeSessionId("claude-session-1");
        params.onAssistantDelta("hello");
        params.onAssistantCompleted();
      })
      .mockImplementationOnce(async (params) => {
        expect(params.claudeSessionId).toBe("claude-session-1");
        params.onAssistantDelta("resumed");
        params.onAssistantCompleted();
      });

    const service = createAiGatewayService({
      claudeAdapter: new MockClaudeAdapter(runTurn),
      repoRoot: process.cwd(),
    });

    await service.sendMessage("session-1", {
      message: "first",
      runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
    });
    await vi.waitFor(() => {
      expect(runTurn).toHaveBeenCalledTimes(1);
    });

    await service.sendMessage("session-1", {
      message: "second",
      runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
    });
    await vi.waitFor(() => {
      expect(runTurn).toHaveBeenCalledTimes(2);
    });
  });

  it("maps streaming callbacks into thread transcript and preview sync state", async () => {
    const service = createAiGatewayService({
      claudeAdapter: new MockClaudeAdapter(async (params) => {
        params.onClaudeSessionId("claude-session-1");
        params.onToolStarted(
          "tool-1",
          UPDATE_CURRENT_EDIT_SESSION_DRAFT_TOOL_NAME,
          "更新当前 edit session draft",
        );
        params.onPreviewUpdated();
        params.onToolCompleted("tool-1", "tool done");
        params.onAssistantDelta("partial");
        params.onAssistantDelta(" answer");
        params.onAssistantCompleted();
      }),
      repoRoot: process.cwd(),
    });

    await service.sendMessage("session-1", {
      message: "update the draft",
      runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
    });

    await vi.waitFor(() => {
      const snapshot = service.getSnapshot("session-1");
      expect(snapshot.claudeSessionId).toBe("claude-session-1");
      expect(snapshot.lastPreviewSyncAt).toBeTruthy();
      expect(snapshot.messages).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            role: "user",
            content: "update the draft",
          }),
          expect.objectContaining({
            role: "tool-status",
            content: "tool done",
            status: "completed",
          }),
          expect.objectContaining({
            role: "assistant",
            content: "partial answer",
            status: "completed",
          }),
        ]),
      );
    });
  });
});
