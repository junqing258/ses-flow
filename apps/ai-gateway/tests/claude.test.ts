import { describe, expect, it } from "vitest";

import {
  buildClaudeTurnPrompt,
  CLAUDE_SYSTEM_PROMPT_CONFIG,
} from "../src/claude.js";

describe("claude turn prompt optimization", () => {
  it("builds a dynamic prompt without duplicating the static system instructions", () => {
    const prompt = buildClaudeTurnPrompt({
      editSessionId: "session-1",
      prompt: "删除 fetch 节点",
      runnerBaseUrl: "http://127.0.0.1:6302/runner-api",
      workflowId: "wf-1",
    });

    expect(prompt).toContain("runner_base_url: http://127.0.0.1:6302/runner-api");
    expect(prompt).toContain("session_id: session-1");
    expect(prompt).toContain("workflow_id: wf-1");
    expect(prompt).toContain("用户需求：\n删除 fetch 节点");
    expect(prompt).not.toContain("你是 SES Flow 页面内 AI 协作助手");
  });

  it("uses the SDK prompt-caching friendly system prompt preset", () => {
    expect(CLAUDE_SYSTEM_PROMPT_CONFIG).toMatchObject({
      type: "preset",
      preset: "claude_code",
      excludeDynamicSections: true,
    });
  });
});
