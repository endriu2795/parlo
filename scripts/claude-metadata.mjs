import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";

// Send control requests only. Keep stdin open until all metadata is read.
export async function withClaudeMetadata(connectors, readMetadata) {
  const executable = path.resolve(".tools/engine/claude.exe");
  const cwd = path.resolve(".local-data/claude-session");
  await mkdir(cwd, { recursive: true });

  const args = [
    "-p", "--verbose", "--input-format", "stream-json", "--output-format", "stream-json",
    "--tools", "", "--setting-sources", "", "--no-chrome", "--no-session-persistence",
    "--permission-prompt-tool", "stdio", "--settings", JSON.stringify({
      disableAllHooks: true,
      disableClaudeAiConnectors: !connectors,
      ...(connectors && { permissions: { ask: ["mcp__*"] } }),
    }),
  ];
  if (!connectors) {
    args.push("--safe-mode", "--strict-mcp-config", "--mcp-config", '{"mcpServers":{}}');
  }
  const child = spawn(executable, args, { cwd, stdio: ["pipe", "pipe", "inherit"], windowsHide: true });
  const lines = createInterface({ input: child.stdout, crlfDelay: Infinity });
  const stream = lines[Symbol.asyncIterator]();
  let failure;
  child.stdin.on("error", (error) => {
    failure ??= error;
    child.kill();
  });
  const timer = setTimeout(() => {
    failure = new Error("Claude Code did not return metadata within one minute.");
    child.kill();
  }, 60_000);

  async function request(request) {
    const id = randomUUID();
    child.stdin.write(JSON.stringify({ type: "control_request", request_id: id, request }) + "\n");
    for (let next = await stream.next(); !next.done; next = await stream.next()) {
      if (!next.value.startsWith("{")) continue;
      const message = JSON.parse(next.value);
      if (message.type !== "control_response" || message.response?.request_id !== id) continue;
      if (message.response.subtype === "error") throw new Error(message.response.error);
      return message.response.response;
    }
    throw failure ?? new Error("Claude Code closed before returning metadata.");
  }

  try {
    await once(child, "spawn");
    const initialized = await request({ subtype: "initialize", hooks: null });
    return await readMetadata(initialized, request);
  } finally {
    clearTimeout(timer);
    lines.close();
    child.stdin.destroy();
    child.kill();
  }
}
