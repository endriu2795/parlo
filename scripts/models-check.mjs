import { withClaudeMetadata } from "./claude-metadata.mjs";

try {
  const models = await withClaudeMetadata(false, ({ models }) => {
    if (!Array.isArray(models)) throw new Error("Claude Code did not return a model catalog.");
    return models;
  });
  console.log(JSON.stringify({ models }, null, 2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
