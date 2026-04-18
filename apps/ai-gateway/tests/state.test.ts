import { describe, expect, it } from "vitest";

import { AiThreadStore } from "../src/state.js";

describe("ai thread store timeline ordering", () => {
  it("splits assistant replies around tool calls so the transcript stays chronological", () => {
    const store = new AiThreadStore();
    const abortController = new AbortController();

    store.startTurn("session-1", abortController);
    store.appendAssistantDelta("session-1", "先读取当前工作流。");
    store.startToolCall("session-1", "tool-1", "get_current_edit_session", "读取当前 edit session");
    store.completeToolCall("session-1", "tool-1", "已读取当前 edit session");
    store.appendAssistantDelta("session-1", "我已经定位到需要删除的节点。");
    store.completeAssistantMessage("session-1");

    const snapshot = store.getSnapshot("session-1");

    expect(snapshot.messages.map((message) => [message.role, message.content])).toEqual([
      ["assistant", "先读取当前工作流。"],
      ["tool-status", "已读取当前 edit session"],
      ["assistant", "我已经定位到需要删除的节点。"],
    ]);
    expect(snapshot.messages.map((message) => message.status)).toEqual([
      "completed",
      "completed",
      "completed",
    ]);
  });
});
