export type Point = [number, number];
export type ShapeKind = "rectangle" | "square" | "ellipse" | "arrow" | "line" | "diamond";
export type AnnotationTool = "interact" | "select" | "brush" | "text" | "shape";
type Style = { color: string; width: number };
export type Ink = Style & { kind: "brush"; points: Point[] };
export type Shape = Style & {
  kind: "shape";
  shape: ShapeKind;
  start: Point;
  end: Point;
  filled: boolean;
};
export type TextNote = {
  kind: "text";
  color: string;
  x: number;
  y: number;
  width: number;
  fontSize: number;
  text: string;
};
export type Annotation = Ink | Shape | TextNote;
export const shapeKinds: ShapeKind[] = [
  "rectangle",
  "square",
  "ellipse",
  "arrow",
  "line",
  "diamond",
];
export function constrainedEnd(
  start: Point,
  end: Point,
  width: number,
  height: number,
  shape: ShapeKind,
): Point {
  const dx = (end[0] - start[0]) * width,
    dy = (end[1] - start[1]) * height;
  if (shape === "line" || shape === "arrow") {
    const angle =
        (Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI) / 4,
      length = Math.hypot(dx, dy);
    return [
      Math.max(0, Math.min(1, start[0] + (Math.cos(angle) * length) / width)),
      Math.max(0, Math.min(1, start[1] + (Math.sin(angle) * length) / height)),
    ];
  }
  const size = Math.min(
    Math.max(Math.abs(dx), Math.abs(dy)),
    (dx < 0 ? start[0] : 1 - start[0]) * width,
    (dy < 0 ? start[1] : 1 - start[1]) * height,
  );
  return [
    start[0] + (Math.sign(dx || 1) * size) / width,
    start[1] + (Math.sign(dy || 1) * size) / height,
  ];
}
export function textLines(
  context: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split("\n")) {
    let line = "";
    for (const character of paragraph) {
      if (line && context.measureText(line + character).width > maxWidth) {
        lines.push(line);
        line = "";
      }
      line += character;
    }
    lines.push(line);
  }
  return lines;
}
export type Bounds = { x: number; y: number; width: number; height: number };
export type ResizeHandle = "nw" | "ne" | "sw" | "se";

// Bounds use CSS pixels; annotation coordinates stay independent of screen DPI.
export function annotationBounds(
  item: Annotation,
  width: number,
  height: number,
  context: CanvasRenderingContext2D,
): Bounds {
  if (item.kind === "text") {
    const fontSize = item.fontSize * width;
    context.font = `600 ${fontSize}px "Segoe UI", Arial, sans-serif`;
    const textWidth = Math.max(fontSize, item.width * width);
    return {
      x: item.x * width,
      y: item.y * height,
      width: textWidth,
      height: textLines(context, item.text, textWidth).length * fontSize * 1.35,
    };
  }
  const points = item.kind === "shape" ? [item.start, item.end] : item.points;
  const xs = points.map((p) => p[0] * width),
    ys = points.map((p) => p[1] * height);
  const x = Math.min(...xs),
    y = Math.min(...ys);
  return {
    x,
    y,
    width: Math.max(1, Math.max(...xs) - x),
    height: Math.max(1, Math.max(...ys) - y),
  };
}

export function hitAnnotation(
  item: Annotation,
  point: Point,
  width: number,
  height: number,
  context: CanvasRenderingContext2D,
): boolean {
  const [x, y] = [point[0] * width, point[1] * height];
  const bounds = annotationBounds(item, width, height, context);
  const tolerance =
    item.kind === "text" ? 4 : Math.max(7, (item.width * width) / 2 + 4);
  if (
    x < bounds.x - tolerance ||
    x > bounds.x + bounds.width + tolerance ||
    y < bounds.y - tolerance ||
    y > bounds.y + bounds.height + tolerance
  )
    return false;
  if (
    item.kind === "text" ||
    (item.kind === "shape" && !["line", "arrow"].includes(item.shape))
  )
    return true;
  const points = item.kind === "brush" ? item.points : [item.start, item.end];
  return points.some((p, i) => {
    const previous = points[Math.max(0, i - 1)];
    const ax = previous[0] * width,
      ay = previous[1] * height;
    const dx = p[0] * width - ax,
      dy = p[1] * height - ay;
    const length = dx * dx + dy * dy;
    const t = length
      ? Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / length))
      : 0;
    return Math.hypot(x - ax - t * dx, y - ay - t * dy) <= tolerance;
  });
}

export function manipulationBounds(
  initial: Bounds,
  dx: number,
  dy: number,
  width: number,
  height: number,
  handle?: ResizeHandle,
  preserveAspect = false,
): Bounds {
  if (!handle)
    return {
      ...initial,
      x: Math.max(
        0,
        Math.min(Math.max(0, width - initial.width), initial.x + dx),
      ),
      y: Math.max(
        0,
        Math.min(Math.max(0, height - initial.height), initial.y + dy),
      ),
    };
  if (preserveAspect) {
    const west = handle.includes("w"),
      north = handle.includes("n");
    const requested = Math.max(
      (initial.width + (west ? -dx : dx)) / initial.width,
      (initial.height + (north ? -dy : dy)) / initial.height,
    );
    const availableWidth = west ? initial.x + initial.width : width - initial.x;
    const availableHeight = north
      ? initial.y + initial.height
      : height - initial.y;
    const maximum = Math.min(
      availableWidth / initial.width,
      availableHeight / initial.height,
    );
    const minimum = Math.min(
      1,
      16 / Math.min(initial.width, initial.height),
      maximum,
    );
    const scale = Math.max(minimum, Math.min(maximum, requested));
    const nextWidth = initial.width * scale,
      nextHeight = initial.height * scale;
    return {
      x: west ? initial.x + initial.width - nextWidth : initial.x,
      y: north ? initial.y + initial.height - nextHeight : initial.y,
      width: nextWidth,
      height: nextHeight,
    };
  }
  const right = initial.x + initial.width,
    bottom = initial.y + initial.height;
  const minWidth = Math.min(16, initial.width),
    minHeight = Math.min(16, initial.height);
  const x = handle.includes("w")
    ? Math.max(0, Math.min(right - minWidth, initial.x + dx))
    : initial.x;
  const y = handle.includes("n")
    ? Math.max(0, Math.min(bottom - minHeight, initial.y + dy))
    : initial.y;
  return {
    x,
    y,
    width: handle.includes("w")
      ? right - x
      : Math.max(minWidth, Math.min(width - x, initial.width + dx)),
    height: handle.includes("n")
      ? bottom - y
      : Math.max(minHeight, Math.min(height - y, initial.height + dy)),
  };
}

export function transformAnnotation(
  item: Annotation,
  from: Bounds,
  to: Bounds,
  width: number,
  height: number,
): Annotation {
  const transform = (p: Point): Point => [
    (to.x + ((p[0] * width - from.x) * to.width) / from.width) / width,
    (to.y + ((p[1] * height - from.y) * to.height) / from.height) / height,
  ];
  if (item.kind === "text")
    return {
      ...item,
      x: to.x / width,
      y: to.y / height,
      width: to.width / width,
      fontSize: (item.fontSize * to.height) / from.height,
    };
  if (item.kind === "shape")
    return { ...item, start: transform(item.start), end: transform(item.end) };
  return { ...item, points: item.points.map(transform) };
}

export function paintAnnotations(
  canvas: HTMLCanvasElement,
  items: Annotation[],
  width: number,
  height: number,
  ratio = window.devicePixelRatio || 1,
) {
  const w = Math.max(1, Math.round(width * ratio)),
    h = Math.max(1, Math.round(height * ratio));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Disegno non disponibile");
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, width, height);
  context.lineCap = "round";
  context.lineJoin = "round";
  for (const item of items) {
    context.fillStyle = item.color;
    context.strokeStyle = item.color;
    if (item.kind === "text") {
      const fontSize = item.fontSize * width;
      context.font = `600 ${fontSize}px "Segoe UI", Arial, sans-serif`;
      context.textBaseline = "top";
      textLines(
        context,
        item.text,
        Math.max(fontSize, item.width * width),
      ).forEach((line, i) =>
        context.fillText(
          line,
          item.x * width,
          item.y * height + i * fontSize * 1.35,
        ),
      );
      continue;
    }
    context.lineWidth = item.width * width;
    context.beginPath();
    if (item.kind === "brush") {
      item.points.forEach(([x, y], i) => {
        if (i) context.lineTo(x * width, y * height);
        else context.moveTo(x * width, y * height);
      });
      if (item.points.length === 1) {
        const [x, y] = item.points[0];
        context.arc(
          x * width,
          y * height,
          context.lineWidth / 2,
          0,
          Math.PI * 2,
        );
        context.fill();
      } else context.stroke();
      continue;
    }
    const ax = item.start[0] * width,
      ay = item.start[1] * height,
      bx = item.end[0] * width,
      by = item.end[1] * height;
    const left = Math.min(ax, bx),
      top = Math.min(ay, by),
      sw = Math.abs(bx - ax),
      sh = Math.abs(by - ay);
    if (item.shape === "rectangle" || item.shape === "square") context.rect(left, top, sw, sh);
    else if (item.shape === "ellipse")
      context.ellipse(
        left + sw / 2,
        top + sh / 2,
        sw / 2,
        sh / 2,
        0,
        0,
        Math.PI * 2,
      );
    else if (item.shape === "diamond") {
      context.moveTo(left + sw / 2, top);
      context.lineTo(left + sw, top + sh / 2);
      context.lineTo(left + sw / 2, top + sh);
      context.lineTo(left, top + sh / 2);
      context.closePath();
    } else {
      context.moveTo(ax, ay);
      context.lineTo(bx, by);
    }
    if (item.filled && !["line", "arrow"].includes(item.shape)) context.fill();
    context.stroke();
    if (item.shape === "arrow") {
      const length = Math.hypot(bx - ax, by - ay);
      if (length < 1) continue;
      const angle = Math.atan2(by - ay, bx - ax),
        head = Math.min(length * 0.45, Math.max(14, context.lineWidth * 3.5));
      context.beginPath();
      context.moveTo(
        bx - head * Math.cos(angle - Math.PI / 6),
        by - head * Math.sin(angle - Math.PI / 6),
      );
      context.lineTo(bx, by);
      context.lineTo(
        bx - head * Math.cos(angle + Math.PI / 6),
        by - head * Math.sin(angle + Math.PI / 6),
      );
      context.stroke();
    }
  }
}
