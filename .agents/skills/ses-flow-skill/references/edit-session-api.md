# Edit Session API

## Purpose

These endpoints back SES Flow AI mode.

- Claude Code updates the draft in runner.
- Runner validates and stores the temporary draft.
- Web subscribes to the session and refreshes preview.

## Create

`POST /runner-api/edit-sessions`

Request body:

```json
{
  "workspaceId": "ses-workflow-editor",
  "workflowId": "wf-optional",
  "editorDocument": {
    "schemaVersion": "1.0",
    "editor": {
      "pageMode": "ai"
    }
  },
  "workflow": {
    "meta": {
      "key": "sorting-main-flow",
      "name": "sorting-main-flow",
      "version": 3,
      "status": "draft"
    },
    "trigger": {
      "type": "manual"
    },
    "inputSchema": {
      "type": "object"
    },
    "nodes": [],
    "transitions": [],
    "policies": {
      "allowManualRetry": true
    }
  }
}
```

Response fields:

- `sessionId`
- `workspaceId`
- `workflowId`
- `workflow`
- `editorDocument`
- `createdAt`
- `updatedAt`

## Update

`PUT /runner-api/edit-sessions/{session_id}`

Request body matches create.

Notes:

- Send the full `workflow`, not a partial patch.
- `editorDocument` is optional but recommended for accurate canvas preview.
- Runner validates the workflow before saving.

## Preview Stream

`WS /runner-api/edit-sessions/{session_id}/ws`

Message shape:

```json
{
  "sessionId": "sess-123",
  "eventType": "snapshot",
  "session": {
    "sessionId": "sess-123",
    "workflowId": "wf-123",
    "workflow": {},
    "editorDocument": {},
    "createdAt": "2026-04-14T00:00:00Z",
    "updatedAt": "2026-04-14T00:00:00Z"
  }
}
```

`eventType` values currently include:

- `snapshot`
- `created`
- `updated`

## AI Mode Rules

- Web is preview only in AI mode.
- Claude Code should hold the editing conversation and mutate the session through runner.
- Keep `editor.editor.pageMode` or equivalent restored state aligned to AI preview intent.
