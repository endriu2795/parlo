import assert from "node:assert/strict";
import { test } from "node:test";
import { passageAtTime } from "../src/transcriptNavigation.ts";

const passages = [
  { start_ms: 1000, end_ms: 3000 },
  { start_ms: 5000, end_ms: 7000 },
  { start_ms: 7000, end_ms: 9000 },
];

test("finds the current passage and switches at a shared boundary", () => {
  assert.equal(passageAtTime(passages, 2000), 0);
  assert.equal(passageAtTime(passages, 7000), 2);
  assert.equal(passageAtTime(passages, 4000), null);
});

test("jump selects the closest passage before, after and between speech", () => {
  assert.equal(passageAtTime(passages, 0, true), 0);
  assert.equal(passageAtTime(passages, 4600, true), 1);
  assert.equal(passageAtTime(passages, 15000, true), 2);
  assert.equal(passageAtTime([], 500, true), null);
});

test("ignores invalid timestamps and supports overlapping and zero-duration passages", () => {
  const ranges = [{ start_ms: NaN, end_ms: 500 }, { start_ms: 100, end_ms: 500 }, { start_ms: 200, end_ms: 300 }, { start_ms: 900, end_ms: 900 }];
  assert.equal(passageAtTime(ranges, 250), 2);
  assert.equal(passageAtTime(ranges, 900), 3);
  assert.equal(passageAtTime(ranges, NaN, true), null);
  assert.equal(passageAtTime([{ start_ms: 500, end_ms: 100 }], 200, true), null);
});
