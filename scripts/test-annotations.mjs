import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { test } from "node:test";
import ts from "typescript";

// Test only the drawing engine with synthetic coordinates; no browser or screen capture.
const source = await readFile(
  new URL("../src/annotations.ts", import.meta.url),
  "utf8",
);
const moduleUrl = (text) => {
  const compiled = ts.transpileModule(text, {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.ESNext,
    },
  }).outputText;
  return `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`;
};
const annotationsModule = moduleUrl(source);
const {
  constrainedEnd,
  paintAnnotations,
  annotationBounds,
  hitAnnotation,
  manipulationBounds,
  transformAnnotation,
} = await import(annotationsModule);
const clipboardSource = await readFile(new URL("../src/annotationClipboard.ts", import.meta.url), "utf8");
const { annotationPlainText, decodeAnnotation, hasAnnotationPrefix } = await import(
  moduleUrl(clipboardSource.replace('"./annotations"', JSON.stringify(annotationsModule)))
);

test("Clipboard line endings preserve copied shapes and freehand strokes", () => {
  const items = [
    { kind: "shape", shape: "square", start: [.2, .2], end: [.4, .4], filled: true },
    { kind: "brush", points: [[.2, .2], [.3, .4]] },
  ];
  for (const item of items) {
    const copied = { version: 1, width: 1000, height: 800, item: { ...item, width: .01, color: "#ff565f" } };
    const encoded = annotationPlainText(copied);
    for (const text of [encoded, encoded.replace("\n", "\r\n"), encoded.replace("parlo-", "presentio-").replace("\n", "\r\n")]) {
      assert.ok(hasAnnotationPrefix(text));
      assert.deepEqual(decodeAnnotation(text), copied);
    }
  }
  assert.ok(hasAnnotationPrefix("parlo-annotation:v1\r\n{"));
  assert.equal(decodeAnnotation("parlo-annotation:v1\r\n{"), null);
  assert.equal(hasAnnotationPrefix("Un testo normale"), false);
});
function surface() {
  const calls = [];
  const context = { measureText: (text) => ({ width: [...text].length * 10 }) };
  for (const name of [
    "setTransform",
    "clearRect",
    "beginPath",
    "moveTo",
    "lineTo",
    "arc",
    "ellipse",
    "rect",
    "closePath",
    "stroke",
    "fill",
    "fillText",
  ])
    context[name] = (...args) => calls.push({ name, args, font: context.font });
  return { canvas: { width: 0, height: 0, getContext: () => context }, calls };
}
test("Shift keeps squares in pixel space and snaps lines at 45 degrees", () => {
  const square = constrainedEnd(
    [0.1, 0.1],
    [0.3, 0.25],
    1000,
    800,
    "rectangle",
  );
  assert.ok(
    Math.abs((square[0] - 0.1) * 1000 - (square[1] - 0.1) * 800) < 1e-6,
  );
  const arrow = constrainedEnd([0.1, 0.1], [0.2, 0.27], 1000, 800, "arrow");
  assert.ok(Math.abs((arrow[0] - 0.1) * 1000 - (arrow[1] - 0.1) * 800) < 1e-6);
  const edge = constrainedEnd([0.9, 0.9], [1, 1], 1000, 800, "ellipse");
  assert.ok(edge.every((value) => value >= 0 && value <= 1));
});
test("Dragging a filled rectangle backwards preserves bounds at high DPI", () => {
  const { canvas, calls } = surface();
  paintAnnotations(
    canvas,
    [
      {
        kind: "shape",
        shape: "rectangle",
        start: [0.75, 0.75],
        end: [0.25, 0.25],
        color: "#ff0000",
        width: 0.02,
        filled: true,
      },
    ],
    200,
    100,
    2,
  );
  assert.equal(canvas.width, 400);
  assert.equal(canvas.height, 200);
  assert.deepEqual(
    calls.find((call) => call.name === "rect").args,
    [50, 25, 100, 50],
  );
  assert.ok(calls.some((call) => call.name === "fill"));
  assert.ok(calls.some((call) => call.name === "stroke"));
  assert.equal(calls.filter((call) => call.name === "clearRect").length, 1);
});
test("Screenshot layer contains freehand ink, arrowheads and wrapped multiline text", () => {
  const { canvas, calls } = surface();
  paintAnnotations(
    canvas,
    [
      { kind: "brush", points: [[0.1, 0.1]], color: "#ffffff", width: 0.02 },
      {
        kind: "shape",
        shape: "arrow",
        start: [0.1, 0.1],
        end: [0.9, 0.8],
        color: "#ff0000",
        width: 0.01,
        filled: false,
      },
      {
        kind: "text",
        x: 0.1,
        y: 0.2,
        width: 0.15,
        fontSize: 0.1,
        color: "#00ff00",
        text: "abcdef\nx",
      },
    ],
    200,
    100,
    1,
  );
  assert.ok(calls.some((call) => call.name === "arc"));
  assert.equal(calls.filter((call) => call.name === "lineTo").length, 3);
  const text = calls.filter((call) => call.name === "fillText");
  assert.deepEqual(
    text.map((call) => call.args),
    [
      ["abc", 20, 20],
      ["def", 20, 47],
      ["x", 20, 74],
    ],
  );
  assert.ok(text.every((call) => call.font.includes("20px")));
});

test("Selection uses wrapped text bounds and distance to a line instead of its empty bounding box", () => {
  const context = surface().canvas.getContext();
  const note = {
    kind: "text",
    x: 0.1,
    y: 0.2,
    width: 0.03,
    fontSize: 0.02,
    color: "#fff",
    text: "abcdef\nx",
  };
  assert.deepEqual(annotationBounds(note, 1000, 800, context), {
    x: 100,
    y: 160,
    width: 30,
    height: 81,
  });
  assert.equal(hitAnnotation(note, [0.12, 0.29], 1000, 800, context), true);
  assert.equal(hitAnnotation(note, [0.2, 0.2], 1000, 800, context), false);
  const line = {
    kind: "shape",
    shape: "line",
    start: [0.1, 0.1],
    end: [0.5, 0.5],
    width: 0.002,
    color: "#fff",
    filled: false,
  };
  assert.equal(hitAnnotation(line, [0.3, 0.3], 1000, 800, context), true);
  assert.equal(hitAnnotation(line, [0.1, 0.5], 1000, 800, context), false);
});

test("Moving clamps to the screen and leaves the original annotation untouched for undo", () => {
  const context = surface().canvas.getContext();
  const item = {
    kind: "shape",
    shape: "rectangle",
    start: [0.75, 0.75],
    end: [0.25, 0.25],
    width: 0.01,
    color: "#fff",
    filled: false,
  };
  const before = structuredClone(item),
    bounds = annotationBounds(item, 1000, 800, context);
  const moved = transformAnnotation(
    item,
    bounds,
    manipulationBounds(bounds, 2000, -2000, 1000, 800),
    1000,
    800,
  );
  assert.deepEqual(moved.start, [1, 0.5]);
  assert.deepEqual(moved.end, [0.5, 0]);
  assert.deepEqual(item, before);
});

test("All resize corners keep the opposite corner fixed, including reversed shapes", () => {
  const context = surface().canvas.getContext();
  const item = {
    kind: "shape",
    shape: "ellipse",
    start: [0.4, 0.4],
    end: [0.1, 0.1],
    width: 0.01,
    color: "#fff",
    filled: false,
  };
  const bounds = annotationBounds(item, 1000, 1000, context);
  for (const handle of ["nw", "ne", "sw", "se"]) {
    const resized = manipulationBounds(
      bounds,
      handle.includes("w") ? -50 : 50,
      handle.includes("n") ? -50 : 50,
      1000,
      1000,
      handle,
    );
    const next = transformAnnotation(item, bounds, resized, 1000, 1000);
    assert.equal(resized.width, 350);
    assert.equal(resized.height, 350);
    assert.equal(
      handle.includes("w") ? resized.x + resized.width : resized.x,
      handle.includes("w") ? 400 : 100,
    );
    assert.equal(
      handle.includes("n") ? resized.y + resized.height : resized.y,
      handle.includes("n") ? 400 : 100,
    );
    assert.ok(next.start[0] > next.end[0] && next.start[1] > next.end[1]);
  }
  const minimum = manipulationBounds(bounds, 1000, 1000, 1000, 1000, "nw");
  assert.ok(minimum.width >= 16 && minimum.height >= 16);
});

test("Resizing text scales its font and box together and preserves wrapping", () => {
  const context = surface().canvas.getContext();
  const item = {
    kind: "text",
    x: 0.1,
    y: 0.1,
    width: 0.3,
    fontSize: 0.02,
    color: "#fff",
    text: "one line\nand another",
  };
  const bounds = annotationBounds(item, 1000, 800, context);
  const resized = manipulationBounds(
    bounds,
    300,
    bounds.height,
    1000,
    800,
    "se",
    true,
  );
  const next = transformAnnotation(item, bounds, resized, 1000, 800);
  assert.equal(next.width, 0.6);
  assert.equal(next.fontSize, 0.04);
  assert.equal(next.text, item.text);
  assert.deepEqual([next.x, next.y], [item.x, item.y]);
});
