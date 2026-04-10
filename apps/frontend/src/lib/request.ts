const formatRequestTimestamp = (date: Date) => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  const milliseconds = String(date.getMilliseconds()).padStart(3, "0");

  return `${year}${month}${day}_${hours}${minutes}${seconds}_${milliseconds}`;
};

const generateRequestId = () => {
  return `${formatRequestTimestamp(new Date())}_${Math.random().toString(36).slice(2, 10)}`;
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
