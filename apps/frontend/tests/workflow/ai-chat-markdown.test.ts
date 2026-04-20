import { describe, expect, it } from "vitest";

import { renderAiChatMarkdown } from "@/features/workflow/ai-chat-markdown";

describe("workflow ai chat markdown", () => {
  it("renders common markdown blocks for assistant messages", () => {
    const html = renderAiChatMarkdown(`
# Plan

- first item
- second item

\`\`\`ts
console.log("ok");
\`\`\`

[docs](https://example.com)
`);

    expect(html).toContain("<h1>Plan</h1>");
    expect(html).toContain("<li>first item</li>");
    expect(html).toContain('<pre><code class="language-ts">');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it("escapes raw html instead of rendering it", () => {
    const html = renderAiChatMarkdown(
      '<script>alert("xss")</script>\n\n<div>unsafe html</div>',
    );

    expect(html).not.toContain("<script>");
    expect(html).not.toContain("<div>unsafe html</div>");
    expect(html).toContain("&lt;script&gt;alert");
    expect(html).toContain("&lt;div&gt;unsafe html&lt;/div&gt;");
  });

  it("keeps single-line breaks in generated html", () => {
    const html = renderAiChatMarkdown("line 1\nline 2");

    expect(html).toContain("line 1<br>");
    expect(html).toContain("line 2");
  });
});
