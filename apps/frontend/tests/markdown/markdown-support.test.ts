import { describe, expect, it } from "vitest";

import HelpPage from "../fixtures/help-page.md";

describe("markdown support", () => {
  it("imports markdown files as Vue components", () => {
    expect(HelpPage).toBeTruthy();
    expect(typeof HelpPage).toBe("object");
    expect(
      "render" in HelpPage
        || "setup" in HelpPage
        || "__name" in HelpPage,
    ).toBe(true);
  });
});
