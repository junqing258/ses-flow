const SHELL_CONTROL_OPERATOR_PATTERN = /(?:&&|\|\||;|\$\(|`|>|<)/;
const DANGEROUS_COMMAND_PATTERN =
  /\b(rm|mv|cp|touch|mkdir|rmdir|git|pnpm|npm|yarn|bun|python|node|perl|ruby|chmod|chown|tee|dd)\b/;
const READ_ONLY_COMMANDS = new Set([
  "pwd",
  "ls",
  "find",
  "rg",
  "grep",
  "sed",
  "cat",
  "head",
  "tail",
  "wc",
  "jq",
]);

const normalizeRunnerBaseUrl = (runnerBaseUrl: string) =>
  runnerBaseUrl.trim().replace(/\/$/, "");

const parseCurlUrl = (command: string) => {
  const match = command.match(/https?:\/\/[^\s"'`]+/);
  return match?.[0] ?? "";
};

const isAllowedCurlCommand = (command: string, runnerBaseUrl: string) => {
  const normalizedBaseUrl = normalizeRunnerBaseUrl(runnerBaseUrl);
  const url = parseCurlUrl(command);

  if (!url || !url.startsWith(normalizedBaseUrl)) {
    return false;
  }

  if (command.includes("@")) {
    return false;
  }

  return !SHELL_CONTROL_OPERATOR_PATTERN.test(command);
};

export const commandTouchesPreview = (
  command: string,
  editSessionId: string,
  runnerBaseUrl: string,
) => {
  const normalizedBaseUrl = normalizeRunnerBaseUrl(runnerBaseUrl);
  return command.includes(
    `${normalizedBaseUrl}/edit-sessions/${editSessionId}/draft`,
  );
};

export const isAllowedBashCommand = (
  command: string,
  runnerBaseUrl: string,
) => {
  const trimmedCommand = command.trim();
  if (!trimmedCommand || SHELL_CONTROL_OPERATOR_PATTERN.test(trimmedCommand)) {
    return false;
  }

  if (trimmedCommand.startsWith("curl ")) {
    return isAllowedCurlCommand(trimmedCommand, runnerBaseUrl);
  }

  const [binary] = trimmedCommand.split(/\s+/, 1);

  if (!READ_ONLY_COMMANDS.has(binary)) {
    return false;
  }

  return !DANGEROUS_COMMAND_PATTERN.test(trimmedCommand);
};

export const isAllowedToolUse = (
  toolName: string,
  input: unknown,
  runnerBaseUrl: string,
) => {
  if (toolName === "Read" || toolName === "Glob" || toolName === "Grep" || toolName === "LS") {
    return true;
  }

  if (toolName !== "Bash") {
    return false;
  }

  const command =
    typeof input === "object" &&
    input !== null &&
    "command" in input &&
    typeof input.command === "string"
      ? input.command
      : "";

  return isAllowedBashCommand(command, runnerBaseUrl);
};
