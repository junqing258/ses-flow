type LogLevel = "info" | "warn" | "error";

interface LogFields {
  [key: string]: unknown;
}

const writeLog = (
  level: LogLevel,
  event: string,
  fields: LogFields = {},
) => {
  const payload = {
    timestamp: new Date().toISOString(),
    level,
    service: "ai-gateway",
    event,
    ...fields,
  };

  const line = JSON.stringify(payload);
  if (level === "error") {
    console.error(line);
    return;
  }

  if (level === "warn") {
    console.warn(line);
    return;
  }

  console.log(line);
};

export const logger = {
  info(event: string, fields?: LogFields) {
    writeLog("info", event, fields);
  },
  warn(event: string, fields?: LogFields) {
    writeLog("warn", event, fields);
  },
  error(event: string, fields?: LogFields) {
    writeLog("error", event, fields);
  },
};

export const summarizeText = (value: string, limit = 120) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= limit) {
    return normalized;
  }

  return `${normalized.slice(0, limit)}...`;
};
