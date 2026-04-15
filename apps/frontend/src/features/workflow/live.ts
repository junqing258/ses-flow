import { createJsonEventSource, type EventSourceSubscription } from "@/lib/sse";
import { RUNNER_BASE_URL } from "./api";
import type { WorkflowRunStatus } from "./runner";

export interface WorkflowStreamNotification {
  emittedAt: string;
  eventType: string;
  missedEvents?: number;
  runId?: string;
  sessionId?: string;
  status?: WorkflowRunStatus;
  workflowId?: string;
}

interface WorkflowStreamSubscriptionOptions {
  onError?: () => void;
  onEvent: (notification: WorkflowStreamNotification) => void;
  onOpen?: () => void;
}

const STREAM_EVENT_NAMES = [
  "stream.connected",
  "stream.resync-required",
  "run.changed",
  "session.changed",
  "workflow.changed",
  "workflow.runs.changed",
];

export const subscribeWorkflowRunEvents = (
  runId: string,
  options: WorkflowStreamSubscriptionOptions,
): EventSourceSubscription | null =>
  createJsonEventSource<WorkflowStreamNotification>(
    `${RUNNER_BASE_URL}/runs/${encodeURIComponent(runId)}/events`,
    {
      eventNames: STREAM_EVENT_NAMES,
      onError: options.onError,
      onEvent: options.onEvent,
      onOpen: options.onOpen,
    },
  );

export const subscribeWorkflowEditSessionEvents = (
  sessionId: string,
  options: WorkflowStreamSubscriptionOptions,
): EventSourceSubscription | null =>
  createJsonEventSource<WorkflowStreamNotification>(
    `${RUNNER_BASE_URL}/edit-sessions/${encodeURIComponent(sessionId)}/events`,
    {
      eventNames: STREAM_EVENT_NAMES,
      onError: options.onError,
      onEvent: options.onEvent,
      onOpen: options.onOpen,
    },
  );

export const subscribeWorkflowEvents = (
  workflowId: string,
  options: WorkflowStreamSubscriptionOptions,
): EventSourceSubscription | null =>
  createJsonEventSource<WorkflowStreamNotification>(
    `${RUNNER_BASE_URL}/workflows/${encodeURIComponent(workflowId)}/events`,
    {
      eventNames: STREAM_EVENT_NAMES,
      onError: options.onError,
      onEvent: options.onEvent,
      onOpen: options.onOpen,
    },
  );

export const subscribeWorkflowsEvents = (
  options: WorkflowStreamSubscriptionOptions,
): EventSourceSubscription | null =>
  createJsonEventSource<WorkflowStreamNotification>(
    `${RUNNER_BASE_URL}/workflows/events`,
    {
      eventNames: STREAM_EVENT_NAMES,
      onError: options.onError,
      onEvent: options.onEvent,
      onOpen: options.onOpen,
    },
  );
