import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelAiThreadTurn,
  createAiChatStorageKey,
  fetchAiThreadSnapshot,
  persistAiThreadSnapshot,
  readPersistedAiThreadSnapshot,
  sendAiThreadMessage,
} from "@/features/workflow/ai-chat";

const createJsonResponse = (body: unknown, init?: ResponseInit) =>
  new Response(JSON.stringify(body), {
    headers: {
      "content-type": "application/json",
    },
    ...init,
  });

describe("workflow ai chat api", () => {
  const fetchMock = vi.fn<typeof fetch>();

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    fetchMock.mockReset();
  });

  it("loads an ai thread snapshot", async () => {
    fetchMock.mockResolvedValueOnce(
      createJsonResponse({
        editSessionId: "session-1",
        status: "idle",
        messages: [],
      }),
    );

    const snapshot = await fetchAiThreadSnapshot("session-1");

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/ai/threads/session-1",
      expect.objectContaining({
        headers: expect.any(Headers),
      }),
    );
    expect(snapshot.editSessionId).toBe("session-1");
  });

  it("sends a new ai message", async () => {
    fetchMock.mockResolvedValueOnce(
      createJsonResponse({
        editSessionId: "session-1",
        status: "running",
        messages: [
          {
            id: "user-1",
            role: "user",
            content: "please update",
            createdAt: "2026-04-18T02:00:00.000Z",
            status: "completed",
          },
        ],
      }),
    );

    const snapshot = await sendAiThreadMessage("session-1", {
      message: "please update",
      runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
      workflowId: "wf-1",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/ai/threads/session-1/messages",
      expect.objectContaining({
        body: JSON.stringify({
          message: "please update",
          runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
          workflowId: "wf-1",
        }),
        method: "POST",
      }),
    );
    expect(snapshot.status).toBe("running");
  });

  it("surfaces gateway errors for cancel requests", async () => {
    fetchMock.mockResolvedValueOnce(
      createJsonResponse(
        {
          error: "thread is not running",
        },
        {
          status: 409,
          statusText: "Conflict",
        },
      ),
    );

    await expect(cancelAiThreadTurn("session-1")).rejects.toThrow(
      "thread is not running",
    );
  });
});

describe("workflow ai chat persistence", () => {
  it("persists and restores a cached snapshot", () => {
    const storage = new Map<string, string>();
    const storageLike = {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => {
        storage.set(key, value);
      },
      removeItem: (key: string) => {
        storage.delete(key);
      },
    };

    persistAiThreadSnapshot(storageLike, {
      editSessionId: "session-1",
      claudeSessionId: "claude-session-1",
      status: "idle",
      messages: [],
      lastPreviewSyncAt: "2026-04-18T03:00:00.000Z",
    });

    expect(storage.has(createAiChatStorageKey("session-1"))).toBe(true);
    expect(readPersistedAiThreadSnapshot(storageLike, "session-1")).toEqual(
      expect.objectContaining({
        claudeSessionId: "claude-session-1",
        lastPreviewSyncAt: "2026-04-18T03:00:00.000Z",
      }),
    );
  });
});
