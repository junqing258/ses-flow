# Warehouse Sorting Workflow MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first runnable MVP of the warehouse sorting workflow platform with a visual editor, publishable workflow definitions, a Node.js execution engine, callback-based resume, and a minimal operations timeline.

**Architecture:** Use a pnpm monorepo with `apps/frontend` for the Vue 3 designer and operations UI, `apps/backend` for the NestJS API and workflow engine, and `packages/workflow-schema` for shared DSL types and validators. The backend owns compilation, execution, state persistence, waiting-event recovery, and audit records; the frontend only edits and previews workflow definitions.

**Tech Stack:** pnpm workspaces, TypeScript, Vue 3, Vite, Vue Flow, Pinia, NestJS, Prisma, PostgreSQL, Redis, Vitest, Supertest

---

## Scope Note

This plan intentionally covers the MVP foundation from the technical review, not the entire long-term platform. It delivers a working vertical slice for:

- workflow definition editing and publishing
- backend definition compilation and validation
- workflow run creation and execution
- wait-and-resume by callback
- minimal operations timeline

This plan does not yet implement:

- full task center UI for PDA/station workers
- multi-tenant template inheritance
- gray release and rollback UI
- advanced connector marketplace

## Planned File Structure

### Workspace root

- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `tsconfig.base.json`
- Create: `.editorconfig`
- Create: `.gitignore`

### Shared package

- Create: `packages/workflow-schema/package.json`
- Create: `packages/workflow-schema/tsconfig.json`
- Create: `packages/workflow-schema/src/index.ts`
- Create: `packages/workflow-schema/src/definition.ts`
- Create: `packages/workflow-schema/src/runtime.ts`
- Create: `packages/workflow-schema/src/validator.ts`
- Create: `packages/workflow-schema/src/__tests__/validator.test.ts`

### Backend

- Create: `apps/backend/package.json`
- Create: `apps/backend/tsconfig.json`
- Create: `apps/backend/src/main.ts`
- Create: `apps/backend/src/app.module.ts`
- Create: `apps/backend/prisma/schema.prisma`
- Create: `apps/backend/src/modules/definitions/*`
- Create: `apps/backend/src/modules/engine/*`
- Create: `apps/backend/src/modules/operations/*`
- Create: `apps/backend/test/*.spec.ts`

### Frontend

- Create: `apps/frontend/package.json`
- Create: `apps/frontend/tsconfig.json`
- Create: `apps/frontend/vite.config.ts`
- Create: `apps/frontend/src/main.ts`
- Create: `apps/frontend/src/App.vue`
- Create: `apps/frontend/src/router.ts`
- Create: `apps/frontend/src/stores/workflow.ts`
- Create: `apps/frontend/src/pages/DesignerPage.vue`
- Create: `apps/frontend/src/pages/OperationsPage.vue`
- Create: `apps/frontend/src/components/workflow/*`
- Create: `apps/frontend/src/__tests__/workflow-store.test.ts`

## Delivery Milestones

1. Monorepo boots locally with backend and frontend apps.
2. Shared workflow definition package validates and compiles publishable DSL.
3. Backend can create a run from a published definition and persist node execution history.
4. Engine supports `start`, `fetch`, `switch`, `action`, `wait`, and `end`.
5. Callback resumes a waiting run and completes the instance.
6. Frontend can create/edit a workflow graph and publish it through backend APIs.
7. Operations page can inspect runs, node timeline, and waiting status.

### Task 1: Bootstrap The Monorepo And Tooling

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `tsconfig.base.json`
- Create: `.editorconfig`
- Create: `.gitignore`
- Create: `apps/backend/package.json`
- Create: `apps/frontend/package.json`

- [ ] **Step 1: Write the failing workspace smoke test as a root script contract**

Add these root scripts so later tasks can fail fast when packages are missing:

```json
{
  "name": "ses-flow",
  "private": true,
  "packageManager": "pnpm@10.0.0",
  "scripts": {
    "build": "pnpm -r build",
    "test": "pnpm -r test",
    "lint": "pnpm -r lint"
  }
}
```

- [ ] **Step 2: Run workspace install check to verify the repo is not wired yet**

Run: `pnpm install`
Expected: workspace installs dependencies, but `pnpm test` fails because app and package test scripts are not defined yet.

- [ ] **Step 3: Write the minimal workspace scaffolding**

Create the workspace files:

```yaml
# pnpm-workspace.yaml
packages:
  - "apps/*"
  - "packages/*"
```

```json
// tsconfig.base.json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "CommonJS",
    "moduleResolution": "Node",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "baseUrl": "."
  }
}
```

```gitignore
node_modules
dist
.turbo
.env
coverage
.DS_Store
```

- [ ] **Step 4: Add minimal app package contracts**

Create starter package manifests:

```json
// apps/backend/package.json
{
  "name": "@ses-flow/backend",
  "private": true,
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "test": "vitest run",
    "start:dev": "nest start --watch"
  }
}
```

```json
// apps/frontend/package.json
{
  "name": "@ses-flow/frontend",
  "private": true,
  "scripts": {
    "build": "vite build",
    "test": "vitest run",
    "dev": "vite"
  }
}
```

- [ ] **Step 5: Run workspace tests to verify script discovery works**

Run: `pnpm test`
Expected: command reaches child packages and fails on missing test files instead of missing workspace definitions.

- [ ] **Step 6: Commit**

```bash
git add package.json pnpm-workspace.yaml tsconfig.base.json .editorconfig .gitignore apps/backend/package.json apps/frontend/package.json
git commit -m "chore: bootstrap monorepo workspace"
```

### Task 2: Build The Shared Workflow Schema Package

**Files:**
- Create: `packages/workflow-schema/package.json`
- Create: `packages/workflow-schema/tsconfig.json`
- Create: `packages/workflow-schema/src/index.ts`
- Create: `packages/workflow-schema/src/definition.ts`
- Create: `packages/workflow-schema/src/runtime.ts`
- Create: `packages/workflow-schema/src/validator.ts`
- Test: `packages/workflow-schema/src/__tests__/validator.test.ts`

- [ ] **Step 1: Write the failing validator test**

```ts
import { describe, expect, it } from "vitest";
import { validateDefinition } from "../validator";

describe("validateDefinition", () => {
  it("accepts a minimal publishable workflow", () => {
    const result = validateDefinition({
      meta: { key: "sorting-main", version: 1, scope: { tenant: "t1", warehouse: "w1" } },
      trigger: { type: "webhook", path: "/flows/order/inbound", responseMode: "async_ack" },
      nodes: [
        { id: "start_1", type: "start", name: "Start" },
        { id: "end_1", type: "end", name: "End" }
      ],
      transitions: [{ from: "start_1", to: "end_1" }]
    });

    expect(result.ok).toBe(true);
    expect(result.errors).toEqual([]);
  });
});
```

- [ ] **Step 2: Run the package test to verify it fails**

Run: `pnpm --filter @ses-flow/workflow-schema test`
Expected: FAIL with module-not-found errors for `validator.ts` or package scripts if the package manifest is still missing.

- [ ] **Step 3: Implement the shared types and validator**

```ts
// packages/workflow-schema/src/definition.ts
export type WorkflowNodeType =
  | "start"
  | "end"
  | "fetch"
  | "switch"
  | "action"
  | "wait";

export type WorkflowDefinition = {
  meta: {
    key: string;
    version: number;
    scope: { tenant: string; warehouse: string };
  };
  trigger: {
    type: "webhook";
    path: string;
    responseMode: "sync" | "async_ack";
  };
  nodes: Array<{ id: string; type: WorkflowNodeType; name: string; config?: Record<string, unknown> }>;
  transitions: Array<{ from: string; to: string; condition?: string; branchType?: string }>;
};
```

```ts
// packages/workflow-schema/src/validator.ts
import { WorkflowDefinition } from "./definition";

export function validateDefinition(input: WorkflowDefinition) {
  const errors: string[] = [];
  const nodeIds = new Set(input.nodes.map((node) => node.id));

  if (!input.nodes.some((node) => node.type === "start")) errors.push("missing start node");
  if (!input.nodes.some((node) => node.type === "end")) errors.push("missing end node");

  for (const edge of input.transitions) {
    if (!nodeIds.has(edge.from) || !nodeIds.has(edge.to)) {
      errors.push(`invalid transition ${edge.from}->${edge.to}`);
    }
  }

  return { ok: errors.length === 0, errors };
}
```

- [ ] **Step 4: Export the package entrypoint**

```ts
// packages/workflow-schema/src/index.ts
export * from "./definition";
export * from "./runtime";
export * from "./validator";
```

- [ ] **Step 5: Run the package test to verify it passes**

Run: `pnpm --filter @ses-flow/workflow-schema test`
Expected: PASS with `1 passed`.

- [ ] **Step 6: Commit**

```bash
git add packages/workflow-schema
git commit -m "feat: add shared workflow schema package"
```

### Task 3: Implement Definition Management And Publish Compilation

**Files:**
- Create: `apps/backend/src/main.ts`
- Create: `apps/backend/src/app.module.ts`
- Create: `apps/backend/prisma/schema.prisma`
- Create: `apps/backend/src/modules/definitions/definitions.module.ts`
- Create: `apps/backend/src/modules/definitions/definitions.controller.ts`
- Create: `apps/backend/src/modules/definitions/definitions.service.ts`
- Create: `apps/backend/src/modules/definitions/definition-compiler.ts`
- Test: `apps/backend/test/definitions.spec.ts`

- [ ] **Step 1: Write the failing backend definition API test**

```ts
import request from "supertest";
import { INestApplication } from "@nestjs/common";

describe("POST /definitions/publish", () => {
  let app: INestApplication;

  it("publishes a validated workflow definition", async () => {
    const response = await request(app.getHttpServer())
      .post("/definitions/publish")
      .send({
        meta: { key: "sorting-main", version: 1, scope: { tenant: "t1", warehouse: "w1" } },
        trigger: { type: "webhook", path: "/flows/order/inbound", responseMode: "async_ack" },
        nodes: [
          { id: "start_1", type: "start", name: "Start" },
          { id: "end_1", type: "end", name: "End" }
        ],
        transitions: [{ from: "start_1", to: "end_1" }]
      });

    expect(response.status).toBe(201);
    expect(response.body.workflowKey).toBe("sorting-main");
    expect(response.body.version).toBe(1);
  });
});
```

- [ ] **Step 2: Run the API test to verify it fails**

Run: `pnpm --filter @ses-flow/backend test -- definitions.spec.ts`
Expected: FAIL because Nest application bootstrap and definitions module do not exist yet.

- [ ] **Step 3: Implement Prisma definition persistence**

```prisma
model WorkflowDefinition {
  id          String   @id @default(cuid())
  workflowKey String
  version     Int
  tenantId    String
  warehouseId String
  status      String
  definition  Json
  createdAt   DateTime @default(now())

  @@unique([workflowKey, version, tenantId, warehouseId])
}
```

- [ ] **Step 4: Implement publish compilation in the definitions module**

```ts
// apps/backend/src/modules/definitions/definition-compiler.ts
import { validateDefinition, WorkflowDefinition } from "@ses-flow/workflow-schema";

export function compileDefinition(input: WorkflowDefinition) {
  const validation = validateDefinition(input);
  if (!validation.ok) {
    throw new Error(validation.errors.join("; "));
  }

  return {
    workflowKey: input.meta.key,
    version: input.meta.version,
    tenantId: input.meta.scope.tenant,
    warehouseId: input.meta.scope.warehouse,
    status: "published",
    definition: input
  };
}
```

```ts
// apps/backend/src/modules/definitions/definitions.controller.ts
@Controller("definitions")
export class DefinitionsController {
  constructor(private readonly service: DefinitionsService) {}

  @Post("publish")
  publish(@Body() body: WorkflowDefinition) {
    return this.service.publish(body);
  }
}
```

- [ ] **Step 5: Run the API test to verify publish works**

Run: `pnpm --filter @ses-flow/backend test -- definitions.spec.ts`
Expected: PASS with one successful publish test.

- [ ] **Step 6: Commit**

```bash
git add apps/backend
git commit -m "feat: add workflow definition publish service"
```

### Task 4: Implement The Execution Engine Vertical Slice

**Files:**
- Create: `apps/backend/src/modules/engine/engine.module.ts`
- Create: `apps/backend/src/modules/engine/engine.controller.ts`
- Create: `apps/backend/src/modules/engine/engine.service.ts`
- Create: `apps/backend/src/modules/engine/executors/start.executor.ts`
- Create: `apps/backend/src/modules/engine/executors/fetch.executor.ts`
- Create: `apps/backend/src/modules/engine/executors/switch.executor.ts`
- Create: `apps/backend/src/modules/engine/executors/action.executor.ts`
- Create: `apps/backend/src/modules/engine/executors/end.executor.ts`
- Create: `apps/backend/src/modules/engine/transition-resolver.ts`
- Test: `apps/backend/test/engine-run.spec.ts`

- [ ] **Step 1: Write the failing engine run test**

```ts
it("executes a published definition from start to end", async () => {
  const response = await request(app.getHttpServer())
    .post("/engine/runs")
    .send({
      workflowKey: "sorting-main",
      tenantId: "t1",
      warehouseId: "w1",
      trigger: { body: { orderNo: "SO-1001", route: "normal" } }
    });

  expect(response.status).toBe(201);
  expect(response.body.status).toBe("succeeded");
  expect(response.body.nodeHistory).toEqual(["start_1", "route_switch", "end_1"]);
});
```

- [ ] **Step 2: Run the engine test to verify it fails**

Run: `pnpm --filter @ses-flow/backend test -- engine-run.spec.ts`
Expected: FAIL because `/engine/runs` and executor registry are not implemented.

- [ ] **Step 3: Implement run persistence and core execution loop**

```ts
// apps/backend/src/modules/engine/engine.service.ts
async createRun(input: CreateRunDto) {
  const definition = await this.definitionRepository.findPublished(input.workflowKey, input.tenantId, input.warehouseId);
  const state = { trigger: input.trigger, runtime: {} };
  const nodeHistory: string[] = [];
  let currentNodeId = "start_1";

  while (currentNodeId) {
    const result = await this.executorRegistry.execute(currentNodeId, definition, state);
    nodeHistory.push(currentNodeId);
    Object.assign(state.runtime, result.statePatch ?? {});
    currentNodeId = this.transitionResolver.next(definition, currentNodeId, result);
    if (result.status === "waiting") break;
  }

  return { status: currentNodeId ? "waiting" : "succeeded", nodeHistory, state };
}
```

- [ ] **Step 4: Implement the executor contracts**

```ts
export interface NodeExecutor {
  supports(type: string): boolean;
  execute(node: WorkflowNode, state: Record<string, unknown>): Promise<NodeExecutionResult>;
}
```

```ts
// start.executor.ts
export class StartExecutor implements NodeExecutor {
  supports(type: string) {
    return type === "start";
  }
  async execute() {
    return { status: "success", statePatch: {} };
  }
}
```

- [ ] **Step 5: Run the engine test to verify the vertical slice passes**

Run: `pnpm --filter @ses-flow/backend test -- engine-run.spec.ts`
Expected: PASS with a completed run and ordered node history.

- [ ] **Step 6: Commit**

```bash
git add apps/backend
git commit -m "feat: add workflow engine vertical slice"
```

### Task 5: Add Waiting Events, Callback Resume, And Timeline APIs

**Files:**
- Modify: `apps/backend/prisma/schema.prisma`
- Create: `apps/backend/src/modules/engine/waiting.service.ts`
- Create: `apps/backend/src/modules/operations/operations.module.ts`
- Create: `apps/backend/src/modules/operations/operations.controller.ts`
- Create: `apps/backend/src/modules/operations/operations.service.ts`
- Test: `apps/backend/test/engine-resume.spec.ts`

- [ ] **Step 1: Write the failing wait-and-resume test**

```ts
it("resumes a waiting run after callback", async () => {
  const created = await request(app.getHttpServer()).post("/engine/runs").send({
    workflowKey: "sorting-main-wait",
    tenantId: "t1",
    warehouseId: "w1",
    trigger: { body: { orderNo: "SO-2001" } }
  });

  expect(created.body.status).toBe("waiting");

  const resumed = await request(app.getHttpServer())
    .post("/engine/callbacks/task")
    .send({ correlationKey: "task:SO-2001", status: "completed" });

  expect(resumed.status).toBe(200);
  expect(resumed.body.runStatus).toBe("succeeded");
});
```

- [ ] **Step 2: Run the resume test to verify it fails**

Run: `pnpm --filter @ses-flow/backend test -- engine-resume.spec.ts`
Expected: FAIL because waiting event persistence and callback endpoint do not exist yet.

- [ ] **Step 3: Add waiting-event and node-execution persistence**

```prisma
model WorkflowRun {
  id            String   @id @default(cuid())
  workflowKey   String
  workflowVersion Int
  tenantId      String
  warehouseId   String
  status        String
  currentNodeId String?
  trigger       Json
  state         Json
  createdAt     DateTime @default(now())
  updatedAt     DateTime @updatedAt
}

model WorkflowWaitingEvent {
  id             String   @id @default(cuid())
  runId          String
  nodeId         String
  correlationKey String   @unique
  status         String
  payload        Json?
  createdAt      DateTime @default(now())
}
```

- [ ] **Step 4: Implement callback resume and operations timeline**

```ts
// apps/backend/src/modules/engine/waiting.service.ts
async resumeByCorrelationKey(correlationKey: string, payload: Record<string, unknown>) {
  const waiting = await this.waitingRepository.findByCorrelationKey(correlationKey);
  const run = await this.runRepository.findById(waiting.runId);
  return this.engineService.resume(run, waiting, payload);
}
```

```ts
// apps/backend/src/modules/operations/operations.controller.ts
@Controller("operations")
export class OperationsController {
  constructor(private readonly service: OperationsService) {}

  @Get("runs/:runId")
  getRun(@Param("runId") runId: string) {
    return this.service.getRunTimeline(runId);
  }
}
```

- [ ] **Step 5: Run the resume and operations tests**

Run: `pnpm --filter @ses-flow/backend test -- engine-resume.spec.ts`
Expected: PASS with waiting run resuming to `succeeded`.

- [ ] **Step 6: Commit**

```bash
git add apps/backend
git commit -m "feat: add waiting-event resume flow and operations timeline"
```

### Task 6: Implement The Frontend Workflow Designer MVP

**Files:**
- Create: `apps/frontend/src/main.ts`
- Create: `apps/frontend/src/App.vue`
- Create: `apps/frontend/src/router.ts`
- Create: `apps/frontend/src/stores/workflow.ts`
- Create: `apps/frontend/src/pages/DesignerPage.vue`
- Create: `apps/frontend/src/components/workflow/NodePalette.vue`
- Create: `apps/frontend/src/components/workflow/PropertiesPanel.vue`
- Test: `apps/frontend/src/__tests__/workflow-store.test.ts`

- [ ] **Step 1: Write the failing workflow store test**

```ts
import { describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useWorkflowStore } from "../stores/workflow";

describe("workflow store", () => {
  it("builds a publish payload from designer nodes and edges", () => {
    setActivePinia(createPinia());
    const store = useWorkflowStore();

    store.setGraph(
      [{ id: "start_1", type: "start", position: { x: 0, y: 0 }, data: { label: "Start" } }],
      []
    );

    const payload = store.toPublishPayload("sorting-main", "t1", "w1");
    expect(payload.meta.key).toBe("sorting-main");
    expect(payload.nodes[0].id).toBe("start_1");
  });
});
```

- [ ] **Step 2: Run the frontend test to verify it fails**

Run: `pnpm --filter @ses-flow/frontend test -- workflow-store.test.ts`
Expected: FAIL because the Pinia store and Vite test setup are not implemented.

- [ ] **Step 3: Implement the designer store and pages**

```ts
// apps/frontend/src/stores/workflow.ts
export const useWorkflowStore = defineStore("workflow", {
  state: () => ({
    nodes: [] as Array<Record<string, unknown>>,
    edges: [] as Array<Record<string, unknown>>
  }),
  actions: {
    setGraph(nodes: Array<Record<string, unknown>>, edges: Array<Record<string, unknown>>) {
      this.nodes = nodes;
      this.edges = edges;
    },
    toPublishPayload(key: string, tenant: string, warehouse: string) {
      return {
        meta: { key, version: 1, scope: { tenant, warehouse } },
        trigger: { type: "webhook", path: "/flows/order/inbound", responseMode: "async_ack" },
        nodes: this.nodes.map((node) => ({ id: node.id, type: node.type, name: node.data?.label ?? node.id })),
        transitions: this.edges.map((edge) => ({ from: edge.source, to: edge.target }))
      };
    }
  }
});
```

```vue
<!-- apps/frontend/src/pages/DesignerPage.vue -->
<template>
  <section class="designer-page">
    <NodePalette />
    <VueFlow v-model:nodes="nodes" v-model:edges="edges" />
    <PropertiesPanel />
    <button @click="publish">发布流程</button>
  </section>
</template>
```

- [ ] **Step 4: Connect the publish action to backend**

```ts
async function publish() {
  const payload = store.toPublishPayload("sorting-main", "t1", "w1");
  await fetch("/api/definitions/publish", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload)
  });
}
```

- [ ] **Step 5: Run the frontend test**

Run: `pnpm --filter @ses-flow/frontend test -- workflow-store.test.ts`
Expected: PASS with the publish payload generated from graph state.

- [ ] **Step 6: Commit**

```bash
git add apps/frontend
git commit -m "feat: add workflow designer mvp"
```

### Task 7: Add The Operations Page And End-To-End Smoke Coverage

**Files:**
- Create: `apps/frontend/src/pages/OperationsPage.vue`
- Modify: `apps/frontend/src/router.ts`
- Create: `apps/backend/test/operations.spec.ts`
- Create: `docs/runbook-mvp.md`

- [ ] **Step 1: Write the failing operations API smoke test**

```ts
it("returns a node timeline for a run", async () => {
  const response = await request(app.getHttpServer()).get("/operations/runs/run_123");
  expect(response.status).toBe(200);
  expect(response.body).toHaveProperty("timeline");
});
```

- [ ] **Step 2: Run the smoke test to verify it fails**

Run: `pnpm --filter @ses-flow/backend test -- operations.spec.ts`
Expected: FAIL because the operations read model is incomplete or route returns 404.

- [ ] **Step 3: Implement the operations page and backend read model**

```vue
<!-- apps/frontend/src/pages/OperationsPage.vue -->
<template>
  <section>
    <h1>运行时间线</h1>
    <ul>
      <li v-for="item in timeline" :key="item.executionId">
        {{ item.nodeName }} - {{ item.status }} - {{ item.startedAt }}
      </li>
    </ul>
  </section>
</template>
```

```ts
// apps/backend/src/modules/operations/operations.service.ts
async getRunTimeline(runId: string) {
  const executions = await this.executionRepository.findByRunId(runId);
  return {
    runId,
    timeline: executions.map((item) => ({
      executionId: item.id,
      nodeName: item.nodeId,
      status: item.status,
      startedAt: item.createdAt
    }))
  };
}
```

- [ ] **Step 4: Add a local runbook for manual verification**

```md
# MVP Runbook

1. `pnpm install`
2. `pnpm --filter @ses-flow/backend prisma migrate dev`
3. `pnpm --filter @ses-flow/backend start:dev`
4. `pnpm --filter @ses-flow/frontend dev`
5. Publish a workflow from `/designer`
6. Trigger a run with `POST /engine/runs`
7. Inspect timeline in `/operations`
```

- [ ] **Step 5: Run the smoke tests**

Run: `pnpm --filter @ses-flow/backend test -- operations.spec.ts && pnpm --filter @ses-flow/frontend test`
Expected: backend operations test passes and frontend unit tests remain green.

- [ ] **Step 6: Commit**

```bash
git add apps/frontend apps/backend docs/runbook-mvp.md
git commit -m "feat: add operations timeline and mvp runbook"
```

## Self-Review

### Spec Coverage

- DSL separation from Vue Flow: covered by Tasks 2 and 3.
- Unified node protocol and execution engine: covered by Task 4.
- Waiting event and callback resume: covered by Task 5.
- Minimal visual designer: covered by Task 6.
- Operations observability timeline: covered by Task 7.
- MVP-first phased delivery from the technical review: reflected in milestone ordering and task order.

### Placeholder Scan

- No `TBD`, `TODO`, or deferred placeholders remain in the task steps.
- Each task includes concrete file paths, commands, and code snippets.

### Type Consistency

- Workflow definition types originate in `packages/workflow-schema` and are reused by the backend definitions and engine modules.
- Waiting-event terminology is consistent across Prisma models, engine service, and operations timeline API.
- Frontend publish payload uses the same `meta`, `trigger`, `nodes`, and `transitions` shape introduced in Task 2.

