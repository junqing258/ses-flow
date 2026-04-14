---
name: ses-flow-skill
description: Use this skill when editing SES Flow workflows through AI mode, working with runner edit sessions, creating or updating a session_id for workflow preview, or pushing workflow/editorDocument drafts so the web canvas refreshes in read-only AI mode.
---

# SES Flow Skill

## Overview

This skill is for AI-driven workflow editing in this repository.

Use it when the task involves SES Flow workflow drafts, `session_id`, runner edit sessions, or AI mode preview refresh in the web editor.

## Core Rules

- `Claude Code` is the editing core. Do the workflow reasoning and changes here.
- `runner` is the source of truth for AI session drafts. Validate through runner APIs.
- `web` in AI mode is preview only. Do not rely on web-side editing controls.
- During an AI session, update the temporary edit session, not the published workflow record.
- Prefer sending both `workflow` and `editorDocument` so the preview can restore the full canvas state.

## When To Use

Use this skill when the user asks to:

- edit a workflow through AI mode
- create or use a `session_id`
- push workflow preview updates to the web editor
- modify nodes, edges, panels, mappings, or workflow metadata through runner edit sessions
- explain or implement the SES Flow AI editing contract

## Workflow

1. Confirm the current workflow source.
Read the current workflow from the repo code, the active runner payload, or a provided `session_id`.

2. Resolve the edit session.
- If a `session_id` already exists, use it.
- If the user needs a new AI session, create one through `POST /runner-api/edit-sessions`.

3. Build the draft payload.
- `workflow` must be a full runner workflow definition.
- `editorDocument` should carry graph nodes, edges, panels, selected node, active tab, and `pageMode: "ai"` when possible.
- Preserve `workflowId` if the session is tied to an existing workflow.

4. Push the draft to runner.
- Update existing sessions with `PUT /runner-api/edit-sessions/{session_id}`.
- Treat runner validation failures as authoritative and fix the payload before retrying.

5. Keep web read-only.
Do not ask the user to edit in the browser while AI mode is active. The browser should only display the latest preview from runner.

## API Contract

Read [references/edit-session-api.md](references/edit-session-api.md) when you need request or response shapes.

The short version:

- Create session: `POST /runner-api/edit-sessions`
- Update session: `PUT /runner-api/edit-sessions/{session_id}`
- Preview stream: `WS /runner-api/edit-sessions/{session_id}/ws`

## Repo Pointers

Read these files when changing the product integration:

- `apps/frontend/src/views/WorkflowEditorPage.vue`
- `apps/frontend/src/features/workflow/session.ts`
- `apps/frontend/src/features/workflow/runner.ts`
- `apps/frontend/src/features/workflow/persistence.ts`
- `apps/runner/src/api/routes.rs`
- `apps/runner/src/server/server.rs`
- `apps/runner/src/store/session.rs`

## Good Defaults

- Keep `pageMode` as `"ai"` for AI preview documents.
- Preserve existing workflow ids, names, versions, and node ids unless the task explicitly changes them.
- After updating a session, expect the web page to refresh from runner events instead of local mutation.

## Avoid

- Do not publish a workflow when the task is only to update the AI draft.
- Do not treat web state as authoritative over runner session state.
- Do not remove fields from `editorDocument` unless they are intentionally obsolete.
