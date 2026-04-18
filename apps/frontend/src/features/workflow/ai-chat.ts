import { request as sendRequest } from "@/lib/request";
import type { AiProviderConfig } from "@/features/workflow/ai-provider";
import {
  createJsonEventSource,
  type EventSourceSubscription,
} from "@/lib/sse";

export type AiThreadStatus = "idle" | "running" | "error";
export type AiChatMessageRole = "user" | "assistant" | "tool-status" | "error";
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

export interface SendAiMessageRequest {
  aiProvider?: AiProviderConfig;
  message: string;
  runnerBaseUrl: string;
  workflowId?: string;
}

class AiThreadRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AiThreadRequestError";
  }
}

const AI_THREAD_BASE_URL = "/api/ai/threads";

export const createAiChatStorageKey = (editSessionId: string) =>
  `ses-flow.ai-chat:${editSessionId}`;

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export const resolveAiChatStorage = () => {
  if (typeof window === "undefined") {
    return null;
  }

  return window.localStorage;
};

export const persistAiThreadSnapshot = (
  storage: StorageLike | null,
  snapshot: AiThreadSnapshot,
) => {
  if (!storage) {
    return;
  }

  storage.setItem(
    createAiChatStorageKey(snapshot.editSessionId),
    JSON.stringify(snapshot),
  );
};

export const readPersistedAiThreadSnapshot = (
  storage: StorageLike | null,
  editSessionId: string,
) => {
  if (!storage) {
    return null;
  }

  const rawValue = storage.getItem(createAiChatStorageKey(editSessionId));
  if (!rawValue) {
    return null;
  }

  try {
    return JSON.parse(rawValue) as AiThreadSnapshot;
  } catch {
    storage.removeItem(createAiChatStorageKey(editSessionId));
    return null;
  }
};

const parseAiResponse = async <T>(
  response: Response,
  fallbackMessage: string,
): Promise<T> => {
  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json")
    ? ((await response.json()) as Record<string, unknown>)
    : null;

  if (!response.ok) {
    const message =
      (typeof payload?.error === "string" && payload.error) ||
      (typeof payload?.message === "string" && payload.message) ||
      fallbackMessage;
    throw new AiThreadRequestError(message);
  }

  return payload as T;
};

export const fetchAiThreadSnapshot = async (
  editSessionId: string,
): Promise<AiThreadSnapshot> => {
  const response = await sendRequest(
    `${AI_THREAD_BASE_URL}/${encodeURIComponent(editSessionId)}`,
  );

  return parseAiResponse<AiThreadSnapshot>(response, "获取 AI 会话失败");
};

export const sendAiThreadMessage = async (
  editSessionId: string,
  request: SendAiMessageRequest,
): Promise<AiThreadSnapshot> => {
  const response = await sendRequest(
    `${AI_THREAD_BASE_URL}/${encodeURIComponent(editSessionId)}/messages`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
    },
  );

  return parseAiResponse<AiThreadSnapshot>(response, "发送 AI 消息失败");
};

export const cancelAiThreadTurn = async (
  editSessionId: string,
): Promise<AiThreadSnapshot> => {
  const response = await sendRequest(
    `${AI_THREAD_BASE_URL}/${encodeURIComponent(editSessionId)}/cancel`,
    {
      method: "POST",
    },
  );

  return parseAiResponse<AiThreadSnapshot>(response, "停止 AI 协作失败");
};

const AI_THREAD_EVENT_NAMES: AiThreadEventType[] = [
  "thread.snapshot",
  "turn.started",
  "message.delta",
  "message.completed",
  "tool.started",
  "tool.completed",
  "preview.updated",
  "turn.completed",
  "turn.failed",
  "turn.cancelled",
];

export const subscribeAiThreadEvents = (
  editSessionId: string,
  options: {
    onError?: () => void;
    onEvent: (event: AiThreadEvent) => void;
    onOpen?: () => void;
  },
): EventSourceSubscription | null =>
  createJsonEventSource<AiThreadEvent>(
    `${AI_THREAD_BASE_URL}/${encodeURIComponent(editSessionId)}/events`,
    {
      eventNames: AI_THREAD_EVENT_NAMES,
      onError: options.onError,
      onEvent: (payload) => options.onEvent(payload),
      onOpen: options.onOpen,
    },
  );
