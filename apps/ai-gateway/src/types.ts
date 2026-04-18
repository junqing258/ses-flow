export type AiThreadStatus = "idle" | "running" | "error";

export type AiChatMessageRole =
  | "user"
  | "assistant"
  | "tool-status"
  | "error";

export type AiChatMessageStatus = "streaming" | "completed" | "error";

export interface AiChatMessage {
  id: string;
  role: AiChatMessageRole;
  content: string;
  createdAt: string;
  status: AiChatMessageStatus;
  toolName?: string;
}

export interface AiThreadSnapshot {
  editSessionId: string;
  claudeSessionId?: string;
  status: AiThreadStatus;
  messages: AiChatMessage[];
  lastPreviewSyncAt?: string;
}

export type AiThreadEventType =
  | "thread.snapshot"
  | "turn.started"
  | "message.delta"
  | "message.completed"
  | "tool.started"
  | "tool.completed"
  | "preview.updated"
  | "turn.completed"
  | "turn.failed"
  | "turn.cancelled";

export interface AiThreadEvent {
  eventType: AiThreadEventType;
  emittedAt: string;
  snapshot: AiThreadSnapshot;
  delta?: string;
  error?: string;
  message?: AiChatMessage;
}

export interface SendAiThreadMessageRequest {
  message: string;
  runnerBaseUrl: string;
  workflowId?: string;
}
