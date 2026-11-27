import { setTimeout as delay } from "node:timers/promises";
import { withClaudeMetadata } from "./claude-metadata.mjs";

try {
  const servers = await withClaudeMetadata(true, async (_initialized, request) => {
    let servers = [];
    // Give background connector discovery time to finish after initialization.
    for (let attempt = 0; attempt < 5; attempt++) {
      if (attempt > 0) await delay(2000);
      ({ mcpServers: servers } = await request({ subtype: "mcp_status" }));
      if (!Array.isArray(servers)) throw new Error("Claude Code did not return connector status.");
      if (servers.length && !servers.some((server) => server.status === "pending")) break;
    }
    return servers.map(({ name, status, tools }) => ({ name, status, tools: tools?.length ?? 0 }));
  });
  console.log(JSON.stringify(servers, null, 2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
