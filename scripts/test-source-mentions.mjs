import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";

const source = await readFile(
  new URL("../src/sourceMentions.ts", import.meta.url),
  "utf8",
);
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
  },
}).outputText;
const { mentionQuery, matchingSources, insertMention, splitSourceMentions } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`
);
const file = (name, scope = "chat", enabled = true, available = true) => ({
  name,
  scope,
  enabled,
  available,
  path: `C:/materiali/${name}`,
  mention: `@«${scope}/${name}»`,
});

test("@ opens at the caret, supports spaces, and ignores email and existing mentions", () => {
  assert.equal(mentionQuery("nome@example.com", 16), null);
  assert.equal(mentionQuery("usa @«Chat/Proposta»", 19), null);
  assert.deepEqual(mentionQuery("Confronta @proposta cliente dopo", 27), {
    start: 10,
    end: 27,
    query: "proposta cliente",
  });
});
test("suggestions include shared files, group chats first, and exclude unavailable sources", () => {
  const shared = file("Stile", "shared"),
    local = file("Proposta");
  assert.deepEqual(
    matchingSources(
      [
        shared,
        file("Escluso", "chat", false),
        local,
        file("Spostato", "shared", true, false),
        { ...file("registrazione.wav"), unsupported_reason: "Audio non supportato" },
      ],
      "",
    ),
    [local, shared],
  );
  assert.deepEqual(matchingSources([local, shared], "STILE"), [shared]);
});
test("inserting a mention preserves text after the caret", () => {
  const text = "Confronta @pro dopo";
  const result = insertMention(text, mentionQuery(text, 14), file("Proposta"));
  assert.equal(result.text, "Confronta @«chat/Proposta» dopo");
  assert.equal(result.caret, "Confronta @«chat/Proposta» ".length);
});

test("sent mentions resolve shared files while preserving surrounding text and line breaks", () => {
  const source = { ...file("Template_slide.pptx", "shared"), id: "shared-file", mention: "@«Condivise/Template_slide.pptx»" };
  const text = `${source.mention}\nvorrei una slide seguendo ${source.mention}`;
  const parts = splitSourceMentions(text, [source]);
  assert.equal(parts.filter(part => part.source).length, 2);
  assert.equal(parts[0].source.id, source.id);
  assert.equal(parts[1].text, "\nvorrei una slide seguendo ");
  assert.equal(parts.map(part => part.text).join(""), text);
  // Excluding a file from AI context must not prevent opening its location.
  assert.equal(splitSourceMentions(source.mention, [{ ...source, enabled: false }])[0].source.id, source.id);
});

test("missing or ambiguous mentions never guess a file path", () => {
  const source = { ...file("Stile.pptx"), id: "one" };
  const text = `Usa ${source.mention} e @«Chat/Sconosciuto»`;
  for (const inventory of [[], [{ ...source, available: false }], [{ ...source, path: null }], [source, { ...source, id: "two", path: "C:/altro/Stile.pptx" }]]) {
    assert.deepEqual(splitSourceMentions(text, inventory), [{ text }]);
  }
});
