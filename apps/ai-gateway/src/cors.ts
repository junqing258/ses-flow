const DEFAULT_CORS_ALLOW_HEADERS = [
  "Content-Type",
  "Authorization",
  "Cache-Control",
  "Last-Event-ID",
];
const DEFAULT_CORS_ALLOW_METHODS = ["GET", "POST", "OPTIONS"];
const DEFAULT_CORS_MAX_AGE = "86400";

export const resolveCorsAllowHeaders = (requestedHeaders?: string | null) => {
  if (requestedHeaders && requestedHeaders.trim().length > 0) {
    return requestedHeaders;
  }

  return DEFAULT_CORS_ALLOW_HEADERS.join(", ");
};

export const buildCorsHeaders = (requestedHeaders?: string | null) => ({
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": DEFAULT_CORS_ALLOW_METHODS.join(", "),
  "Access-Control-Allow-Headers": resolveCorsAllowHeaders(requestedHeaders),
  "Access-Control-Max-Age": DEFAULT_CORS_MAX_AGE,
  Vary: "Access-Control-Request-Headers",
});

export const isPreflightRequest = (method: string) => method.toUpperCase() === "OPTIONS";
