const generateRequestId = () => {
  return `req_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
};

const withRequestId = (headers?: HeadersInit) => {
  const nextHeaders = new Headers(headers ?? {});

  if (!nextHeaders.has("x-request-id")) {
    nextHeaders.set("x-request-id", generateRequestId());
  }

  return nextHeaders;
};

export const request = (input: RequestInfo | URL, init: RequestInit = {}) =>
  fetch(input, {
    ...init,
    headers: withRequestId(init.headers),
  });
