import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";

process.env.TZ = "Europe/Rome";
const source = await readFile(
  new URL("../src/archiveDates.ts", import.meta.url),
  "utf8",
);
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
  },
}).outputText;
const { archiveDateBounds } = await import(
  `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`
);

test("date filter includes the whole selected local day", () => {
  assert.deepEqual(archiveDateBounds("2026-10-17", "2026-10-17"), {
    from: "2026-10-16T22:00:00.000Z",
    to: "2026-10-17T22:00:00.000Z",
  });
});
test("date filter respects 23-hour and 25-hour daylight-saving days", () => {
  for (const [date, hours] of [
    ["2026-03-29", 23],
    ["2026-10-25", 25],
  ]) {
    const bounds = archiveDateBounds(date, date);
    assert.equal(
      (Date.parse(bounds.to) - Date.parse(bounds.from)) / 3600000,
      hours,
    );
  }
});
test("date filter supports one-sided bounds and rejects reversed ranges", () => {
  assert.deepEqual(archiveDateBounds("", ""), { from: null, to: null });
  assert.equal(archiveDateBounds("2026-10-17", "").to, null);
  assert.equal(archiveDateBounds("", "2026-10-17").from, null);
  assert.equal(archiveDateBounds("2026-10-18", "2026-10-17"), null);
  assert.equal(archiveDateBounds("invalid", ""), null);
});
