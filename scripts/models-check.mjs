// Official SDK oracle: initialize without ever yielding a prompt or generating a response.
import { query } from "@anthropic-ai/claude-agent-sdk";
import path from "node:path";
import { mkdir } from "node:fs/promises";
await mkdir(".local-data/claude-session", { recursive: true });
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
      "safe-mode": null,
      "strict-mcp-config": null,
      "mcp-config": '{"mcpServers":{}}',
      "no-chrome": null,
      settings: '{"disableAllHooks":true,"disableClaudeAiConnectors":true}',
    },
  },
});
try {
  const models = await session.supportedModels();
  console.log(JSON.stringify({ models }, null, 2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  session.close();
  release();
}
