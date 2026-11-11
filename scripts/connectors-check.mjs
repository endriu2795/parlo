// Metadata only: initialize a CLI session without ever sending a model prompt.
import { query } from "@anthropic-ai/claude-agent-sdk";
import path from "node:path";
let release;
const wait = new Promise((resolve) => {
  release = resolve;
});
async function* emptyInput() {
  await wait;
}
const session = query({
  prompt: emptyInput(),
  options: {
    pathToClaudeCodeExecutable: path.resolve(".tools/engine/claude.exe"),
    cwd: path.resolve(".local-data/claude-session"),
    tools: [],
    settingSources: [],
    persistSession: false,
    extraArgs: {
      "no-chrome": null,
      settings:
        '{"disableAllHooks":true,"disableClaudeAiConnectors":false,"permissions":{"ask":["mcp__*"]}}',
    },
    canUseTool: async () => ({
      behavior: "deny",
      message: "Metadata probe never calls tools",
    }),
  },
});
try {
  await session.supportedModels();
  // Background connector discovery can finish after initialization.
  let servers = [];
  for (let i = 0; i < 5; i++) {
    servers = await session.mcpServerStatus();
    if (servers.length && !servers.some((s) => s.status === "pending")) break;
    await new Promise((r) => setTimeout(r, 2000));
  }
  console.log(
    JSON.stringify(
      servers.map((s) => ({
        name: s.name,
        status: s.status,
        tools: s.tools?.length || 0,
      })),
      null,
      2,
    ),
  );
} catch (e) {
  console.error(e.message);
  process.exitCode = 1;
} finally {
  session.close();
  release();
}
