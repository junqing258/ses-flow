import { describe, expect, it } from "vitest";

import {
  WORKFLOW_PALETTE_CATEGORIES,
  createWorkflowPaletteCategories,
  createWorkflowNodeDraft,
  getWorkflowFieldSelectOptions,
  resolveWorkflowIcon,
  resolveWorkflowReferenceId,
} from "@/features/workflow/model";

describe("createWorkflowNodeDraft", () => {
  it("uses the palette id as a stable fallback when the label is non-latin", () => {
    const { node } = createWorkflowNodeDraft(
      {
        id: "palette-review-step",
        kind: "effect",
        label: "人工复核",
        icon: "activity",
        accent: "#8B5CF6",
      },
      { x: 120, y: 240 },
      [],
    );

    expect(node.id).toBe("review_step");
    expect(node.data.nodeKey).toBe("review_step");
  });

  it("provides editable options for HTTP request methods", () => {
    const fetchPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-fetch");

    expect(fetchPaletteItem).toBeDefined();

    const { panel } = createWorkflowNodeDraft(
      fetchPaletteItem!,
      { x: 160, y: 240 },
      [],
    );
    const methodField = panel.fieldsByTab.base?.find(
      (field) => field.key === "method",
    );

    expect(methodField).toBeDefined();
    expect(getWorkflowFieldSelectOptions(panel, methodField!)).toEqual([
      { label: "GET", value: "GET" },
      { label: "POST", value: "POST" },
    ]);
  });

  it("creates set-state nodes with a writable state path and value", () => {
    const setStatePaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-set-state");

    expect(setStatePaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      setStatePaletteItem!,
      { x: 180, y: 240 },
      [],
    );
    const statePathField = panel.fieldsByTab.base?.find(
      (field) => field.key === "statePath",
    );
    const valueField = panel.fieldsByTab.mapping?.find(
      (field) => field.key === "value",
    );

    expect(node.data.kind).toBe("set-state");
    expect(node.data.title).toBe("Set State");
    expect(statePathField?.value).toBe("statePatch");
    expect(valueField?.value).toContain("handledBy");
  });

  it("creates db query nodes with PostgreSQL config fields", () => {
    const dbQueryPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-db-query");

    expect(dbQueryPaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      dbQueryPaletteItem!,
      { x: 180, y: 240 },
      [],
    );
    const modeField = panel.fieldsByTab.base?.find(
      (field) => field.key === "mode",
    );

    expect(node.data.kind).toBe("db-query");
    expect(node.data.title).toBe("DB Query");
    expect(
      panel.fieldsByTab.base?.find((field) => field.key === "connectionKey")
        ?.value,
    ).toBe("default");
    expect(panel.fieldsByTab.base?.find((field) => field.key === "sql")?.value)
      .toContain(":order_no");
    expect(panel.fieldsByTab.mapping?.find((field) => field.key === "params"))
      .toBeDefined();
    expect(getWorkflowFieldSelectOptions(panel, modeField!)).toEqual([
      { label: "read", value: "read" },
      { label: "write", value: "write" },
    ]);
  });

  it("maps switch fallback select options from current branches", () => {
    const switchPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-switch");

    expect(switchPaletteItem).toBeDefined();

    const { panel } = createWorkflowNodeDraft(
      switchPaletteItem!,
      { x: 220, y: 240 },
      [],
    );
    const fallbackField = panel.fieldsByTab.base?.find(
      (field) => field.key === "fallback",
    );

    expect(fallbackField).toBeDefined();
    expect(getWorkflowFieldSelectOptions(panel, fallbackField!)).toEqual([
      { label: "A", value: "branch-a" },
      { label: "B", value: "branch-b" },
    ]);
  });

  it("creates if-else nodes with default then and else branches", () => {
    const ifElsePaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-if-else");

    expect(ifElsePaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      ifElsePaletteItem!,
      { x: 220, y: 240 },
      [],
    );
    const expressionField = panel.fieldsByTab.base?.find(
      (field) => field.key === "expression",
    );
    const fallbackField = panel.fieldsByTab.base?.find(
      (field) => field.key === "fallback",
    );

    expect(node.data.kind).toBe("if-else");
    expect(node.data.title).toBe("If / Else");
    expect(node.data.branchHandles).toEqual([
      { id: "branch-a", label: "then" },
      { id: "branch-b", label: "else", isDefault: true },
    ]);
    expect(expressionField?.value).toBe("payload.condition === true");
    expect(fallbackField?.value).toBe("else");
  });

  it("creates dedicated workflow selection fields for sub-workflow nodes", () => {
    const subWorkflowPaletteItem = WORKFLOW_PALETTE_CATEGORIES.flatMap(
      (category) => category.items,
    ).find((item) => item.id === "palette-subflow");

    expect(subWorkflowPaletteItem).toBeDefined();

    const { node, panel } = createWorkflowNodeDraft(
      subWorkflowPaletteItem!,
      { x: 260, y: 240 },
      [],
    );
    const workflowRefField = panel.fieldsByTab.base?.find(
      (field) => field.key === "workflowRef",
    );

    expect(node.data.kind).toBe("sub-workflow");
    expect(node.data.title).toBe("Sub-Workflow");
    expect(workflowRefField).toBeDefined();
    expect(workflowRefField?.type).toBe("select");
    expect(
      panel.fieldsByTab.mapping?.find((field) => field.key === "payload")
        ?.value,
    ).toBe("{{input}}");
  });

  it("resolves sub-workflow references to workflow ids", () => {
    expect(
      resolveWorkflowReferenceId("child-flow", [
        {
          workflowId: "wf-child-1",
          workflowKey: "child-flow",
        },
      ]),
    ).toBe("wf-child-1");
  });

  it("prefers an exact workflow id match over a workflow key match", () => {
    expect(
      resolveWorkflowReferenceId("child-flow", [
        {
          workflowId: "wf-child-1",
          workflowKey: "child-flow",
        },
        {
          workflowId: "child-flow",
          workflowKey: "legacy-child-flow",
        },
      ]),
    ).toBe("child-flow");
  });

  it("creates plugin palette items from node descriptors", () => {
    const categories = createWorkflowPaletteCategories([
      {
        id: "hello_world",
        kind: "effect",
        runnerType: "plugin:hello_world",
        version: "1.0.0",
        category: "业务节点",
        displayName: "Hello World",
        color: "#2563EB",
        icon: "message-circle-more",
        status: "stable",
        transport: "http",
        timeoutMs: 5000,
        description: "hello world plugin",
        configSchema: {
          type: "object",
          properties: {
            target: {
              type: "string",
              title: "默认问候对象",
              "x-component": "input",
            },
          },
        },
        defaults: {
          target: "World",
        },
      },
    ]);

    const pluginCategory = categories.find(
      (category) => category.label === "业务节点",
    );
    const pluginItem = pluginCategory?.items.find(
      (item) => item.runnerType === "plugin:hello_world",
    );

    expect(pluginItem).toBeDefined();
    expect(pluginItem?.label).toBe("Hello World");
    expect(pluginItem?.accent).toBe("#2563EB");
    expect(pluginItem?.icon).toBe("message-circle-more");
    expect(pluginItem?.kind).toBe("effect");
  });

  it("resolves lucide icon names, http aliases, and image urls", () => {
    const lucideIcon = resolveWorkflowIcon("send-horizontal");
    const httpAliasIcon = resolveWorkflowIcon("http");
    const imageIcon = resolveWorkflowIcon("https://example.com/icon.svg");

    expect(lucideIcon.kind).toBe("component");
    expect(httpAliasIcon.kind).toBe("component");
    expect(imageIcon).toEqual({
      kind: "image",
      src: "https://example.com/icon.svg",
    });
  });

  it("groups plugin palette items by plugin application before category", () => {
    const categories = createWorkflowPaletteCategories([
      {
        id: "hello_world",
        kind: "effect",
        runnerType: "plugin:hello_world",
        version: "1.0.0",
        category: "人工工作台",
        displayName: "Hello World",
        pluginAppId: "hello_world",
        pluginAppName: "Hello World",
        endpoint: "http://127.0.0.1:9101",
        status: "stable",
        transport: "http",
      },
      {
        id: "manual_pick",
        kind: "effect",
        runnerType: "plugin:manual_pick",
        version: "1.0.0",
        category: "人工工作台",
        displayName: "人工拣货",
        pluginAppId: "workstation",
        endpoint: "http://127.0.0.1:9102",
        status: "stable",
        transport: "http",
      },
      {
        id: "manual_weigh",
        kind: "effect",
        runnerType: "plugin:manual_weigh",
        version: "1.0.0",
        category: "人工工作台",
        displayName: "人工称货",
        pluginAppId: "workstation",
        endpoint: "http://127.0.0.1:9102",
        status: "stable",
        transport: "http",
      },
    ]);

    const helloWorldCategory = categories.find(
      (category) => category.label === "Hello World",
    );
    const workstationCategory = categories.find(
      (category) => category.label === "Workstation",
    );

    expect(helloWorldCategory?.items.map((item) => item.label)).toEqual([
      "Hello World",
    ]);
    expect(workstationCategory?.items.map((item) => item.label)).toEqual([
      "人工拣货",
      "人工称货",
    ]);
  });
});
