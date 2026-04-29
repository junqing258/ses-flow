export interface EventSourceSubscription {
  close: () => void;
}

interface JsonEventSourceOptions<T> {
  eventNames: string[];
  onError?: () => void;
  onEvent: (payload: T, eventName: string) => void;
  onOpen?: () => void;
  token?: string | null;
}

export const createJsonEventSource = <T>(
  url: string,
  options: JsonEventSourceOptions<T>,
): EventSourceSubscription | null => {
  if (typeof window === "undefined" || typeof EventSource === "undefined") {
    return null;
  }

  let finalUrl = url;
  if (options.token) {
    const sep = url.includes("?") ? "&" : "?";
    finalUrl = `${url}${sep}token=${encodeURIComponent(options.token)}`;
  }

  const source = new EventSource(finalUrl);
  const removeListeners: Array<() => void> = [];

  if (options.onOpen) {
    source.onopen = () => {
      options.onOpen?.();
    };
  }

  if (options.onError) {
    source.onerror = () => {
      options.onError?.();
    };
  }

  options.eventNames.forEach((eventName) => {
    const listener: EventListener = (event) => {
      if (!(event instanceof MessageEvent) || typeof event.data !== "string") {
        return;
      }

      try {
        options.onEvent(JSON.parse(event.data) as T, eventName);
      } catch (error) {
        console.error(`Failed to parse SSE payload for ${eventName}`, error);
      }
    };

    source.addEventListener(eventName, listener);
    removeListeners.push(() => {
      source.removeEventListener(eventName, listener);
    });
  });

  return {
    close: () => {
      removeListeners.forEach((removeListener) => removeListener());
      source.close();
    },
  };
};
