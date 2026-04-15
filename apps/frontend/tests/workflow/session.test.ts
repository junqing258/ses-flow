import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  WORKFLOW_EDIT_SESSION_RUNNER_BASE_URL,
  fetchWorkflowEditSession,
} from "@/features/workflow/session";

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    headers: {
      "content-type": "application/json",
    },
    ...init,
  });

describe("workflow edit session api", () => {
  const fetchMock = vi.fn<typeof fetch>();

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    fetchMock.mockReset();
  });

  it("loads an edit session snapshot with GET", async () => {
    fetchMock.mockResolvedValueOnce(
      createJsonResponse({
        createdAt: "2026-04-15T10:00:00.000Z",
        editorDocument: null,
        sessionId: "session-123",
        updatedAt: "2026-04-15T10:00:01.000Z",
        workflow: {
          meta: {
            key: "sorting-main-flow",
            name: "Sorting Main Flow",
            status: "draft",
            version: 3,
          },
          trigger: {
            type: "manual",
          },
          inputSchema: {
            type: "object",
          },
          nodes: [],
          transitions: [],
          policies: {
            allowManualRetry: true,
          },
        },
        workflowId: "wf-123",
        workspaceId: "ses-workflow-editor",
      }),
    );

    const session = await fetchWorkflowEditSession("session-123");

    expect(fetchMock).toHaveBeenCalledWith(
      `${WORKFLOW_EDIT_SESSION_RUNNER_BASE_URL}/edit-sessions/session-123`,
      expect.objectContaining({
        headers: expect.any(Headers),
      }),
    );
    expect(session.sessionId).toBe("session-123");
    expect(session.workflowId).toBe("wf-123");
  });

  it("surfaces runner error messages for failed GET requests", async () => {
    fetchMock.mockResolvedValueOnce(
      createJsonResponse(
        {
          error: "workflow edit session not found",
        },
        {
          status: 404,
          statusText: "Not Found",
        },
      ),
    );

    await expect(fetchWorkflowEditSession("missing-session")).rejects.toThrow(
      "workflow edit session not found",
    );
  });
});
