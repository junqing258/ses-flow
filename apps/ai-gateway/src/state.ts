import { randomUUID } from "node:crypto";

import type {
  AiChatMessage,
  AiChatMessageRole,
  AiChatMessageStatus,
  AiThreadEvent,
  AiThreadEventType,
  AiThreadSnapshot,
  AiThreadStatus,
} from "./types.js";

type Listener = (event: AiThreadEvent) => void;

interface ThreadState {
  editSessionId: string;
  claudeSessionId?: string;
  status: AiThreadStatus;
  messages: AiChatMessage[];
  lastPreviewSyncAt?: string;
  listeners: Set<Listener>;
  activeAbortController?: AbortController;
  activeAssistantMessageId?: string;
  activeToolMessageIds: Map<string, string>;
}

const buildMessage = (
  role: AiChatMessageRole,
  content: string,
  status: AiChatMessageStatus = "completed",
  options: {
    id?: string;
    createdAt?: string;
    toolName?: string;
  } = {},
): AiChatMessage => ({
  id: options.id ?? randomUUID(),
  role,
  content,
  createdAt: options.createdAt ?? new Date().toISOString(),
  status,
  toolName: options.toolName,
});

export class AiThreadStore {
  private readonly threads = new Map<string, ThreadState>();

  getSnapshot(editSessionId: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    return this.serialize(thread);
  }

  subscribe(editSessionId: string, listener: Listener): () => void {
    const thread = this.getOrCreate(editSessionId);
    thread.listeners.add(listener);
    listener(this.buildEvent("thread.snapshot", thread));

    return () => {
      thread.listeners.delete(listener);
    };
  }

  addUserMessage(editSessionId: string, content: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.messages.push(buildMessage("user", content));
    this.emit(thread, "thread.snapshot");
    return this.serialize(thread);
  }

  startTurn(editSessionId: string, abortController: AbortController): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.status = "running";
    thread.activeAbortController = abortController;
    thread.activeAssistantMessageId = undefined;
    thread.activeToolMessageIds.clear();
    this.emit(thread, "turn.started");
    return this.serialize(thread);
  }

  setClaudeSessionId(editSessionId: string, claudeSessionId: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.claudeSessionId = claudeSessionId;
    this.emit(thread, "thread.snapshot");
    return this.serialize(thread);
  }

  appendAssistantDelta(editSessionId: string, delta: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    if (!delta) {
      return this.serialize(thread);
    }

    const message = this.ensureAssistantMessage(thread);
    message.content += delta;
    this.emit(thread, "message.delta", {
      delta,
      message,
    });
    return this.serialize(thread);
  }

  completeAssistantMessage(editSessionId: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    if (!thread.activeAssistantMessageId) {
      return this.serialize(thread);
    }

    const message = thread.messages.find(
      (entry) => entry.id === thread.activeAssistantMessageId,
    );
    if (!message) {
      return this.serialize(thread);
    }

    message.status = "completed";
    this.emit(thread, "message.completed", {
      message,
    });
    return this.serialize(thread);
  }

  startToolCall(
    editSessionId: string,
    toolCallId: string,
    toolName: string,
    content: string,
  ): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    this.resetActiveAssistantMessage(thread);
    const message = buildMessage("tool-status", content, "streaming", {
      toolName,
    });
    thread.messages.push(message);
    thread.activeToolMessageIds.set(toolCallId, message.id);
    this.emit(thread, "tool.started", {
      message,
    });
    return this.serialize(thread);
  }

  completeToolCall(
    editSessionId: string,
    toolCallId: string,
    content?: string,
  ): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    const messageId = thread.activeToolMessageIds.get(toolCallId);
    if (!messageId) {
      return this.serialize(thread);
    }

    const message = thread.messages.find((entry) => entry.id === messageId);
    if (!message) {
      thread.activeToolMessageIds.delete(toolCallId);
      return this.serialize(thread);
    }

    if (content) {
      message.content = content;
    }
    message.status = "completed";
    thread.activeToolMessageIds.delete(toolCallId);
    this.emit(thread, "tool.completed", {
      message,
    });
    return this.serialize(thread);
  }

  markPreviewUpdated(editSessionId: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.lastPreviewSyncAt = new Date().toISOString();
    this.emit(thread, "preview.updated");
    return this.serialize(thread);
  }

  finishTurn(editSessionId: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.status = "idle";
    thread.activeAbortController = undefined;
    thread.activeAssistantMessageId = undefined;
    thread.activeToolMessageIds.clear();
    this.emit(thread, "turn.completed");
    return this.serialize(thread);
  }

  failTurn(editSessionId: string, error: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.status = "error";
    thread.activeAbortController = undefined;
    thread.activeAssistantMessageId = undefined;
    thread.activeToolMessageIds.clear();
    const message = buildMessage("error", error, "error");
    thread.messages.push(message);
    this.emit(thread, "turn.failed", {
      error,
      message,
    });
    return this.serialize(thread);
  }

  cancelTurn(editSessionId: string): AiThreadSnapshot {
    const thread = this.getOrCreate(editSessionId);
    thread.activeAbortController?.abort();
    thread.status = "idle";
    thread.activeAbortController = undefined;
    thread.activeAssistantMessageId = undefined;
    thread.activeToolMessageIds.clear();
    this.emit(thread, "turn.cancelled");
    return this.serialize(thread);
  }

  isRunning(editSessionId: string): boolean {
    return this.getOrCreate(editSessionId).status === "running";
  }

  private getOrCreate(editSessionId: string): ThreadState {
    const existing = this.threads.get(editSessionId);
    if (existing) {
      return existing;
    }

    const thread: ThreadState = {
      editSessionId,
      status: "idle",
      messages: [],
      listeners: new Set(),
      activeToolMessageIds: new Map(),
    };
    this.threads.set(editSessionId, thread);
    return thread;
  }

  private ensureAssistantMessage(thread: ThreadState): AiChatMessage {
    if (thread.activeAssistantMessageId) {
      const existing = thread.messages.find(
        (entry) => entry.id === thread.activeAssistantMessageId,
      );
      if (existing) {
        return existing;
      }
    }

    const message = buildMessage("assistant", "", "streaming");
    thread.messages.push(message);
    thread.activeAssistantMessageId = message.id;
    return message;
  }

  private resetActiveAssistantMessage(thread: ThreadState) {
    if (!thread.activeAssistantMessageId) {
      return;
    }

    const message = thread.messages.find(
      (entry) => entry.id === thread.activeAssistantMessageId,
    );
    if (message && message.status === "streaming") {
      message.status = "completed";
    }
    thread.activeAssistantMessageId = undefined;
  }

  private serialize(thread: ThreadState): AiThreadSnapshot {
    return {
      editSessionId: thread.editSessionId,
      claudeSessionId: thread.claudeSessionId,
      status: thread.status,
      messages: thread.messages.map((message) => ({ ...message })),
      lastPreviewSyncAt: thread.lastPreviewSyncAt,
    };
  }

  private buildEvent(
    eventType: AiThreadEventType,
    thread: ThreadState,
    extra: Partial<AiThreadEvent> = {},
  ): AiThreadEvent {
    return {
      eventType,
      emittedAt: new Date().toISOString(),
      snapshot: this.serialize(thread),
      ...extra,
    };
  }

  private emit(
    thread: ThreadState,
    eventType: AiThreadEventType,
    extra: Partial<AiThreadEvent> = {},
  ) {
    const event = this.buildEvent(eventType, thread, extra);
    thread.listeners.forEach((listener) => listener(event));
  }
}
