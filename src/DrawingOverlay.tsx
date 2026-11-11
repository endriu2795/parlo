import { t } from "./i18n";
import { defaultShortcut, formatShortcut, primaryModifier } from "./shortcuts";
import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Camera,
  Undo2,
  Redo2,
  Trash2,
  X,
  Pencil,
  Type,
  Shapes,
  RectangleHorizontal,
  Square,
  Copy,
  ClipboardPaste,
  Circle,
  MoveUpRight,
  Minus,
  Diamond,
  Check,
  ChevronDown,
  PaintBucket,
  Hand,
  MousePointerClick,
  MessageSquareText,
} from "lucide-react";
import { api } from "./api";
import { annotationMime, annotationPlainText, decodeAnnotation, encodeAnnotation, hasAnnotationPrefix, legacyAnnotationMime, pastedAnnotation } from "./annotationClipboard";
import type { CopiedAnnotation } from "./annotationClipboard";
import {
  ColorControls,
  SizeControls,
  PropertyButton,
} from "./AnnotationControls";
import { DockButton } from "./EdgeDock";
import {
  constrainedEnd,
  paintAnnotations,
  shapeKinds,
  annotationBounds,
  hitAnnotation,
  manipulationBounds,
  transformAnnotation,
} from "./annotations";
import type {
  Annotation,
  AnnotationTool,
  Point,
  ShapeKind,
  TextNote,
  Bounds,
  ResizeHandle,
} from "./annotations";

const shapeLabels: Record<ShapeKind, string> = {
  rectangle: t("Rettangolo"),
  square: t("Quadrato"),
  ellipse: t("Ellisse"),
  arrow: t("Freccia"),
  line: t("Linea"),
  diamond: t("Rombo"),
};

const shapeIcons = {
  rectangle: RectangleHorizontal,
  square: Square,
  ellipse: Circle,
  arrow: MoveUpRight,
  line: Minus,
  diamond: Diamond,
};
type Draft = { note: TextNote; index?: number };
type Popup = "color" | "size" | "shape" | null;
type Manipulation = {
  pointer: number;
  index: number;
  start: Point;
  original: Annotation;
  bounds: Bounds;
  handle?: ResizeHandle;
  preview: Annotation;
  changed: boolean;
};

export default function DrawingOverlay() {
  const canvas = useRef<HTMLCanvasElement>(null),
    textInput = useRef<HTMLTextAreaElement>(null);
  const items = useRef<Annotation[]>([]),
    past = useRef<Annotation[][]>([]),
    future = useRef<Annotation[][]>([]);
  const gesture = useRef<{
    pointer: number;
    item: Exclude<Annotation, TextNote>;
  } | null>(null);
  const manipulation = useRef<Manipulation | null>(null);
  const selectedRef = useRef<number | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const clipboard = useRef<CopiedAnnotation | null>(null);
  const localCopyOnly = useRef(false);
  const pasteCount = useRef(0);
  const [canPaste, setCanPaste] = useState(false);
  const [selectionBounds, setSelectionBounds] = useState<Bounds | null>(null);
  const select = useCallback((index: number | null) => {
    selectedRef.current = index;
    setSelected(index);
  }, []);
  const draft = useRef<Draft | null>(null),
    capturing = useRef(false),
    frame = useRef<number | null>(null);
  const controls = useRef({
    color: "#ff565f",
    size: 5,
    font: 24,
    filled: false,
  });
  const videoEnabled = useRef(false), videoDirty = useRef(false), videoSending = useRef(false);
  const videoTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const publishVideo = useCallback(function queueVideo() {
    if (!videoEnabled.current) return;
    videoDirty.current = true;
    if (videoTimer.current || videoSending.current) return;
    videoTimer.current = setTimeout(() => {
      videoTimer.current = null; videoDirty.current = false;
      const layer = canvas.current;
      if (!layer || !videoEnabled.current) return;
      videoSending.current = true;
      void api.videoDrawing(layer.toDataURL("image/png"))
        .catch(e => setMessage(String(e)))
        .finally(() => { videoSending.current = false; if (videoDirty.current) queueVideo(); });
    }, 100);
  }, []);
  useEffect(() => () => { if (videoTimer.current) clearTimeout(videoTimer.current); }, []);
  const loaded = useRef(false),
    saveQueue = useRef<Promise<void>>(Promise.resolve());
  const toolRef = useRef<AnnotationTool>("brush"),
    shapeRef = useRef<ShapeKind>("rectangle"),
    popupRef = useRef<Popup>(null);
  const [tool, setTool] = useState<AnnotationTool>("brush"),
    [shape, setShape] = useState<ShapeKind>("rectangle");
  const [color, setColor] = useState(controls.current.color),
    [size, setSize] = useState(5),
    [font, setFont] = useState(24),
    [filled, setFilled] = useState(false);
  const [popup, setPopupState] = useState<Popup>(null),
    [dock, setDock] = useState(false);
  const [showHelp, setShowHelp] = useState(true);
  const [textDraft, setTextDraft] = useState<Draft | null>(null),
    [busy, setBusy] = useState(false),
    [message, setMessage] = useState("");
  const [history, setHistory] = useState({
      count: 0,
      undo: false,
      redo: false,
    }),
    [shortcut, setShortcut] = useState(defaultShortcut("S"));
  const [viewport, setViewport] = useState({
    width: innerWidth,
    height: innerHeight,
  });
  const setPopup = useCallback((next: Popup) => {
    popupRef.current = next;
    setPopupState(next);
  }, []);
  const updateHistory = useCallback(
    () =>
      setHistory({
        count: items.current.length,
        undo: past.current.length > 0,
        redo: future.current.length > 0,
      }),
    [],
  );
  const redraw = useCallback(() => {
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      if (!canvas.current) return;
      const visible = items.current
        .map((item, i) =>
          manipulation.current?.index === i
            ? manipulation.current.preview
            : item,
        )
        .filter((_, i) => i !== draft.current?.index);
      if (gesture.current) visible.push(gesture.current.item);
      paintAnnotations(canvas.current, visible, innerWidth, innerHeight);
      publishVideo();
      const index = selectedRef.current;
      const item =
        index === null
          ? null
          : manipulation.current?.index === index
            ? manipulation.current.preview
            : items.current[index];
      const context = canvas.current.getContext("2d");
      setSelectionBounds(
        item && context && toolRef.current === "select"
          ? annotationBounds(item, innerWidth, innerHeight, context)
          : null,
      );
    });
  }, [publishVideo]);
  const remember = useCallback(() => {
    past.current.push(items.current.slice());
    if (past.current.length > 100) past.current.shift();
    future.current = [];
  }, []);
  const persist = useCallback(() => {
    const { color, size } = controls.current;
    const request = saveQueue.current
      .catch(() => {})
      .then(() => api.setBrush(color, size));
    saveQueue.current = request;
    return request;
  }, []);
  const commitText = useCallback(() => {
    const current = draft.current;
    if (!current) return;
    const text = current.note.text;
    if (text.trim() || current.index !== undefined) {
      remember();
      const width =
        textInput.current?.getBoundingClientRect().width ??
        current.note.width * innerWidth;
      const note: TextNote = {
        ...current.note,
        text: text.slice(0, 4000),
        color: controls.current.color,
        fontSize: controls.current.font / innerWidth,
        width: width / innerWidth,
      };
      if (current.index !== undefined) {
        if (text.trim()) items.current[current.index] = note;
        else items.current.splice(current.index, 1);
      } else items.current.push(note);
    }
    draft.current = null;
    setTextDraft(null);
    updateHistory();
    redraw();
  }, [redraw, remember, updateHistory]);
  const finishGesture = useCallback(() => {
    if (canvas.current) canvas.current.style.cursor = "";
    const editing = manipulation.current;
    manipulation.current = null;
    if (editing) {
      if (editing.changed) {
        remember();
        items.current[editing.index] = editing.preview;
        updateHistory();
      }
      redraw();
      return;
    }
    const current = gesture.current;
    gesture.current = null;
    if (!current) return;
    if (
      current.item.kind === "shape" &&
      Math.hypot(
        (current.item.end[0] - current.item.start[0]) * innerWidth,
        (current.item.end[1] - current.item.start[1]) * innerHeight,
      ) < 3
    ) {
      redraw();
      return;
    }
    remember();
    items.current.push(current.item);
    updateHistory();
    redraw();
  }, [redraw, remember, updateHistory]);
  const changeTool = useCallback(
    (next: AnnotationTool) => {
      if (capturing.current) return;
      finishGesture();
      commitText();
      toolRef.current = next;
      setTool(next);
      select(null);
      redraw();
      setDock(false);
      setPopup(null);
    },
    [commitText, finishGesture, redraw, select, setPopup],
  );
  const changeShape = useCallback((next: ShapeKind) => {
    shapeRef.current = next;
    setShape(next);
  }, []);
  const copySelection = useCallback(() => {
    if (capturing.current || toolRef.current !== "select" || selectedRef.current === null) return null;
    finishGesture();
    const item = items.current[selectedRef.current];
    if (!item) return null;
    const copy: CopiedAnnotation = { version: 1, width: innerWidth, height: innerHeight, item: structuredClone(item) };
    clipboard.current = copy;
    pasteCount.current = 0;
    setCanPaste(true);
    setMessage(t("Elemento copiato"));
    return copy;
  }, [finishGesture]);
  const copyToClipboard = useCallback(() => {
    const value = copySelection();
    if (!value) return;
    localCopyOnly.current = true;
    if (navigator.clipboard) {
      void navigator.clipboard.writeText(annotationPlainText(value))
        .then(() => { localCopyOnly.current = false; })
        .catch(() => setMessage(t("Elemento copiato nell’editor")));
    }
  }, [copySelection]);
  const pasteSelection = useCallback((copy: CopiedAnnotation) => {
    if (capturing.current || toolRef.current !== "select") return;
    finishGesture();
    if (items.current.length >= 2000) { setMessage(t("Cancella alcune annotazioni per continuare.")); return; }
    const context = canvas.current?.getContext("2d");
    if (!context) return;
    const item = pastedAnnotation(copy, innerWidth, innerHeight, context, ++pasteCount.current);
    remember();
    items.current.push(item);
    select(items.current.length - 1);
    setPopup(null);
    updateHistory();
    redraw();
    setMessage(t("Elemento incollato"));
  }, [finishGesture, redraw, remember, select, setPopup, updateHistory]);
  const cancelText = useCallback(() => {
    draft.current = null;
    setTextDraft(null);
    redraw();
  }, [redraw]);
  const undo = useCallback(() => {
    if (capturing.current) return;
    if (draft.current) {
      cancelText();
      return;
    }
    gesture.current = null;
    manipulation.current = null;
    select(null);
    const previous = past.current.pop();
    if (!previous) {
      redraw();
      return;
    }
    future.current.push(items.current);
    items.current = previous;
    updateHistory();
    redraw();
  }, [cancelText, redraw, select, updateHistory]);
  const redo = useCallback(() => {
    if (capturing.current) return;
    const next = future.current.pop();
    if (!next) return;
    cancelText();
    gesture.current = null;
    manipulation.current = null;
    select(null);
    past.current.push(items.current);
    items.current = next;
    updateHistory();
    redraw();
  }, [cancelText, redraw, select, updateHistory]);
  useEffect(() => {
    const editingText = (target: EventTarget | null) => target instanceof Element && !!target.closest("input,textarea,select,[contenteditable=true]");
    const copy = (event: ClipboardEvent) => {
      if (editingText(event.target) || !event.clipboardData) return;
      const value = copySelection();
      if (!value) return;
      event.preventDefault();
      event.clipboardData.setData("text/plain", annotationPlainText(value));
      localCopyOnly.current = false;
      // A plain-text fallback also works on WebViews without custom MIME support.
      try { event.clipboardData.setData(annotationMime, encodeAnnotation(value)); } catch { /* Optional format. */ }
    };
    const paste = (event: ClipboardEvent) => {
      if (capturing.current || toolRef.current !== "select" || editingText(event.target) || !event.clipboardData) return;
      const plain = event.clipboardData.getData("text/plain");
      let custom = "";
      try { custom = event.clipboardData.getData(annotationMime) || event.clipboardData.getData(legacyAnnotationMime); } catch { /* Plain-text fallback below. */ }
      let value = decodeAnnotation(custom) ?? decodeAnnotation(plain);
      if (!value && (custom || hasAnnotationPrefix(plain))) {
        event.preventDefault(); setMessage(t("L’elemento negli appunti non è valido.")); return;
      }
      if (!value && clipboard.current && plain === annotationPlainText(clipboard.current)) value = clipboard.current;
      if (!value && plain.trim()) {
        value = { version: 1, width: innerWidth, height: innerHeight, item: {
          kind: "text", text: plain.slice(0, 4000), color: controls.current.color,
          x: .2, y: .25, width: Math.min(300, innerWidth * .6) / innerWidth, fontSize: controls.current.font / innerWidth,
        } };
      }
      if (!value) return;
      event.preventDefault();
      clipboard.current = value;
      setCanPaste(true);
      pasteSelection(value);
    };
    window.addEventListener("copy", copy);
    window.addEventListener("paste", paste);
    return () => { window.removeEventListener("copy", copy); window.removeEventListener("paste", paste); };
  }, [copySelection, pasteSelection]);
  const capture = useCallback(async () => {
    if (capturing.current) return;
    capturing.current = true;
    setBusy(true);
    setMessage("");
    try {
      finishGesture();
      commitText();
      setPopup(null);
      const layer = document.createElement("canvas");
      paintAnnotations(layer, items.current, innerWidth, innerHeight);
      const overlay_png = items.current.length
        ? layer.toDataURL("image/png")
        : undefined;
      await persist();
      // Let the compositor present an empty overlay before the native window
      // hides: Windows can otherwise capture its fading previous frame.
      document.documentElement.classList.add("capturing-screen");
      await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      await api.screenshot({ overlay_png });
      setMessage("");
    } catch (e) {
      setMessage(String(e));
    } finally {
      document.documentElement.classList.remove("capturing-screen");
      capturing.current = false;
      setBusy(false);
    }
  }, [commitText, finishGesture, persist, setPopup]);
  const close = useCallback(() => {
    if (capturing.current) return;
    finishGesture();
    commitText();
    setPopup(null);
    toolRef.current = "brush";
    setTool("brush");
    void api.drawingInteraction(false, []).catch(() => {});
    void persist()
      .catch((e) => setMessage(String(e)))
      .then(() => api.videoDrawing(null))
      .catch((e) => setMessage(String(e)))
      .then(() => getCurrentWindow().hide())
      .catch((e) => setMessage(String(e)));
  }, [commitText, finishGesture, persist, setPopup]);
  useEffect(() => {
    let alive = true;
    const presentationFrames = new Set<number>();
    let presentationToken: string | null = null;
    const present = (token: string | null) => {
      if (!alive || !token || token === presentationToken) return;
      presentationToken = token;
      setViewport({ width: innerWidth, height: innerHeight });
      redraw();
      // The native window is composed but still cloaked on Windows. Let the
      // canvas resize and one complete frame paint before revealing it.
      const afterFrame = (callback: () => void) => {
        const id = requestAnimationFrame(() => {
          presentationFrames.delete(id);
          if (alive) callback();
        });
        presentationFrames.add(id);
      };
      afterFrame(() => afterFrame(() => {
        void api.drawingFrame(token).catch(e => { if (alive) setMessage(String(e)); });
      }));
    };
    const settings = () =>
      void api
        .settings()
        .then((s) => {
          if (!alive) return;
          controls.current.color = s.brush_color || "#ff565f";
          controls.current.size = s.brush_size || 5;
          loaded.current = true;
          videoEnabled.current = s.keep_video === true;
          redraw();
          setColor(controls.current.color);
          setSize(controls.current.size);
          setShortcut(s.screenshot_shortcut || defaultShortcut("S"));
        })
        .catch((e) => {
          if (alive) setMessage(String(e));
        });
    settings();
    const reset = () => {
      items.current = [];
      past.current = [];
      future.current = [];
      gesture.current = null;
      manipulation.current = null;
      select(null);
      draft.current = null;
      setTextDraft(null);
      setDock(false);
      setPopup(null);
      setMessage("");
      toolRef.current = "brush";
      setTool("brush");
      updateHistory();
      redraw();
      settings();
    };
    const subscriptions = [
      listen("screenshot-request", () => void capture()),
      listen("screenshot-saved", () => setMessage(t("Screenshot salvato nella trascrizione"))),
      listen("annotation-exit", close),
      listen<string | null>("drawing-shown", e => { setShowHelp(true); redraw(); present(e.payload); }),
      listen("drawing-reset", reset),
      listen<AnnotationTool>("annotation-tool", (e) => changeTool(e.payload)),
    ];
    // Also cover a shortcut pressed while the lazy editor is still mounting.
    void Promise.all(subscriptions).then(() => api.drawingFrame()).then(present)
      .catch(e => { if (alive) setMessage(String(e)); });
    const resize = () => {
      setViewport({ width: innerWidth, height: innerHeight });
      redraw();
    };
    const blur = () => {
      if (loaded.current)
        void persist().catch((e) => {
          if (alive) setMessage(String(e));
        });
    };
    const key = (e: KeyboardEvent) => {
      if (capturing.current) return;
      if (e.key === "Escape") { e.preventDefault(); close(); return; }
      const field = (e.target as HTMLElement)?.closest(
        "input,textarea,select,[contenteditable=true]",
      );
      if (field) {
        if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && draft.current) {
          e.preventDefault(); commitText();
        }
        return;
      }
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && toolRef.current === "select") {
        if (e.key.toLowerCase() === "c" && selectedRef.current !== null) {
          e.preventDefault(); copyToClipboard(); return;
        }
        if (e.key.toLowerCase() === "v" && localCopyOnly.current && clipboard.current) {
          e.preventDefault(); pasteSelection(clipboard.current); return;
        }
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
        e.preventDefault();
        redo();
        return;
      }
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key.toLowerCase() === "i") {
        e.preventDefault();
        changeTool("interact");
      }
      if (
        (e.key === "Delete" || e.key === "Backspace") &&
        selectedRef.current !== null
      ) {
        e.preventDefault();
        finishGesture();
        remember();
        items.current.splice(selectedRef.current, 1);
        select(null);
        updateHistory();
        redraw();
        return;
      }
      if (e.key.toLowerCase() === "v") {
        e.preventDefault();
        changeTool("select");
      }
      if (e.key.toLowerCase() === "b") {
        e.preventDefault();
        changeTool("brush");
      }
      if (e.key.toLowerCase() === "t") {
        e.preventDefault();
        changeTool("text");
      }
      if (e.key.toLowerCase() === "g") {
        e.preventDefault();
        changeTool("shape");
        setPopup("shape");
      }
      if (
        toolRef.current === "shape" &&
        ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)
      ) {
        e.preventDefault();
        const step = ["ArrowLeft", "ArrowUp"].includes(e.key) ? -1 : 1;
        changeShape(
          shapeKinds[
            (shapeKinds.indexOf(shapeRef.current) + step + shapeKinds.length) %
              shapeKinds.length
          ],
        );
      }
      if (e.key === "Enter" && popupRef.current === "shape") {
        e.preventDefault();
        setPopup(null);
      }
    };
    window.addEventListener("keydown", key);
    window.addEventListener("resize", resize);
    window.addEventListener("blur", blur);
    redraw();
    return () => {
      alive = false;
      presentationFrames.forEach(cancelAnimationFrame);
      subscriptions.forEach((p) => void p.then((stop) => stop()));
      window.removeEventListener("keydown", key);
      window.removeEventListener("resize", resize);
      window.removeEventListener("blur", blur);
      if (frame.current !== null) {
        cancelAnimationFrame(frame.current);
        frame.current = null;
      }
    };
  }, [
    cancelText,
    capture,
    changeShape,
    changeTool,
    close,
    commitText,
    copyToClipboard,
    pasteSelection,
    persist,
    redraw,
    redo,
    remember,
    select,
    setPopup,
    undo,
    updateHistory,
  ]);
  useEffect(() => {
    if (!textDraft) return;
    const frame = requestAnimationFrame(() => textInput.current?.focus({preventScroll: true}));
    return () => cancelAnimationFrame(frame);
  }, [textDraft?.index, textDraft?.note.x, textDraft?.note.y]);
  useEffect(() => {
    const field = textInput.current;
    if (!field || !textDraft) return;
    field.style.height = "0px";
    field.style.height = `${Math.min(viewport.height * .5, Math.max(font * 1.35, field.scrollHeight))}px`;
  }, [textDraft?.note.text, font, viewport]);
  useEffect(() => {
    const elements = Array.from(document.querySelectorAll<HTMLElement>(
      ".annotation-toolbar, .annotation-popover, .annotation-edge-tab, .annotation-context",
    ));
    const update = () => {
      const toolbar = document.querySelector<HTMLElement>(".annotation-toolbar");
      if (toolbar) document.querySelector<HTMLElement>(".drawing-overlay")?.style.setProperty("--annotation-toolbar-bottom", `${toolbar.getBoundingClientRect().bottom + 10}px`);
      const regions = elements.map(element => {
        const r = element.getBoundingClientRect();
        return { x: r.x - 6, y: r.y - 6, width: r.width + 12, height: r.height + 12 };
      });
      void api.drawingInteraction(tool === "interact", regions).catch(e => setMessage(String(e)));
    };
    update();
    const observer = new ResizeObserver(update);
    elements.forEach(element => observer.observe(element));
    return () => observer.disconnect();
  }, [tool, dock, popup, viewport, showHelp]);
  useEffect(() => () => { void api.drawingInteraction(false, []).catch(() => {}); }, []);
  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(() => setMessage(""), 4000);
    return () => clearTimeout(timer);
  }, [message]);
  useEffect(() => {
    if (!loaded.current) return;
    const timer = setTimeout(
      () => void persist().catch((e) => setMessage(String(e))),
      220,
    );
    return () => clearTimeout(timer);
  }, [color, size, persist]);
  const updateColor = (value: string) => {
    controls.current.color = value;
    setColor(value);
  };
  const updateSize = (value: number) => {
    if (tool === "text") {
      controls.current.font = value;
      setFont(value);
    } else {
      controls.current.size = value;
      setSize(value);
    }
  };
  const point = (e: { clientX: number; clientY: number }): Point => [
    Math.max(0, Math.min(1, e.clientX / innerWidth)),
    Math.max(0, Math.min(1, e.clientY / innerHeight)),
  ];
  const beginText = (p: Point, existing?: number) => {
    if (existing !== undefined) {
      const note = items.current[existing] as TextNote;
      draft.current = { note: { ...note }, index: existing };
      controls.current.color = note.color;
      controls.current.font = Math.round(note.fontSize * innerWidth);
      setColor(note.color);
      setFont(controls.current.font);
    } else {
      const x =
        Math.min(
          Math.max(12, p[0] * innerWidth),
          Math.max(12, innerWidth - 180),
        ) / innerWidth;
      const y =
        Math.min(
          Math.max(12, p[1] * innerHeight),
          Math.max(12, innerHeight - 150),
        ) / innerHeight;
      draft.current = {
        note: {
          kind: "text",
          color: controls.current.color,
          x,
          y,
          width: Math.min(150, innerWidth - 40) / innerWidth,
          fontSize: controls.current.font / innerWidth,
          text: "",
        },
      };
    }
    setTextDraft(draft.current);
    setPopup(null);
    redraw();
  };
  const startManipulation = (
    e: React.PointerEvent<HTMLElement>,
    index: number,
    handle?: ResizeHandle,
  ) => {
    const context = canvas.current?.getContext("2d");
    if (!context || capturing.current || e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    const item = items.current[index];
    if (canvas.current) canvas.current.style.cursor = "grabbing";
    select(index);
    manipulation.current = {
      pointer: e.pointerId,
      index,
      start: point(e),
      original: item,
      bounds: annotationBounds(item, innerWidth, innerHeight, context),
      handle,
      preview: item,
      changed: false,
    };
    redraw();
  };
  const moveManipulation = (e: React.PointerEvent<HTMLElement>) => {
    const editing = manipulation.current;
    if (!editing || editing.pointer !== e.pointerId || capturing.current)
      return;
    const p = point(e);
    const dx = (p[0] - editing.start[0]) * innerWidth;
    const dy = (p[1] - editing.start[1]) * innerHeight;
    if (Math.hypot(dx, dy) < 2 && !editing.changed) return;
    const bounds = manipulationBounds(
      editing.bounds,
      dx,
      dy,
      innerWidth,
      innerHeight,
      editing.handle,
      editing.original.kind === "text" || (editing.original.kind === "shape" && editing.original.shape === "square"),
    );
    editing.preview = transformAnnotation(
      editing.original,
      editing.bounds,
      bounds,
      innerWidth,
      innerHeight,
    );
    editing.changed = true;
    redraw();
  };
  const cancelGesture = () => {
    gesture.current = null;
    manipulation.current = null;
    if (canvas.current) canvas.current.style.cursor = "";
    redraw();
  };
  const ShapeIcon = shapeIcons[shape];
  const togglePopup = (name: Popup) => {
    finishGesture();
    setPopup(popup === name ? null : name);
  };
  const hint =
    tool === "interact"
      ? t("Usa le altre finestre mantenendo visibili i disegni")
      : tool === "select"
      ? t("Trascina per spostare · {0}+C / {1}+V copia e incolla · doppio clic sul testo", primaryModifier, primaryModifier)
      : tool === "text"
        ? t("Clic per scrivere · {0}+Invio per confermare", primaryModifier)
        : tool === "shape"
          ? t("{0} · frecce per cambiare forma", shapeLabels[shape])
          : t("Trascina per disegnare");
  return (
    <div
      className={`drawing-overlay tool-${tool} ${dock ? "toolbar-docked" : ""}`}
    >
      <canvas
        ref={canvas}
        className="drawing-canvas"
        aria-label={t("Annotazioni sullo schermo")}
        onPointerDown={(e) => {
          if (capturing.current || e.button !== 0 || toolRef.current === "interact") return;
          setPopup(null);
          commitText();
          if (
            toolRef.current !== "select" &&
            toolRef.current !== "text" &&
            items.current.length >= 2000
          ) {
            setMessage(t("Cancella alcune annotazioni per continuare."));
            return;
          }
          const p = point(e);
          if (toolRef.current === "select") {
            const context = canvas.current?.getContext("2d");
            let found = -1;
            if (context)
              for (let i = items.current.length - 1; i >= 0; i--) {
                if (
                  hitAnnotation(
                    items.current[i],
                    p,
                    innerWidth,
                    innerHeight,
                    context,
                  )
                ) {
                  found = i;
                  break;
                }
              }
            if (found >= 0) startManipulation(e, found);
            else {
              select(null);
              redraw();
            }
            return;
          }
          if (toolRef.current === "text") {
            e.preventDefault();
            let found = -1;
            const context = canvas.current?.getContext("2d");
            for (let i = items.current.length - 1; i >= 0; i--) {
              const note = items.current[i];
              if (note.kind !== "text") continue;
              if (
                context &&
                hitAnnotation(note, p, innerWidth, innerHeight, context)
              ) {
                found = i;
                break;
              }
            }
            if (found < 0 && items.current.length >= 2000) {
              setMessage(t("Cancella alcune annotazioni per continuare."));
              return;
            }
            beginText(p, found >= 0 ? found : undefined);
            return;
          }
          e.currentTarget.setPointerCapture(e.pointerId);
          const style = {
            color: controls.current.color,
            width: controls.current.size / innerWidth,
          };
          gesture.current = {
            pointer: e.pointerId,
            item:
              toolRef.current === "brush"
                ? { kind: "brush", ...style, points: [p] }
                : {
                    kind: "shape",
                    ...style,
                    shape: shapeRef.current,
                    start: p,
                    end: p,
                    filled: controls.current.filled,
                  },
          };
          redraw();
        }}
        onPointerMove={(e) => {
          if (manipulation.current) {
            moveManipulation(e);
            return;
          }
          if (toolRef.current === "select") {
            const context = canvas.current?.getContext("2d");
            const p = point(e);
            e.currentTarget.style.cursor =
              context &&
              items.current.some((item) =>
                hitAnnotation(item, p, innerWidth, innerHeight, context),
              )
                ? "grab"
                : "default";
            return;
          }
          const g = gesture.current;
          if (!g || g.pointer !== e.pointerId || capturing.current) return;
          const p = point(e);
          if (g.item.kind === "brush") {
            if (g.item.points.length >= 10000) {
              finishGesture();
              return;
            }
            g.item.points.push(p);
          } else
            g.item.end = e.shiftKey || g.item.shape === "square"
              ? constrainedEnd(
                  g.item.start,
                  p,
                  innerWidth,
                  innerHeight,
                  g.item.shape,
                )
              : p;
          redraw();
        }}
        onPointerUp={finishGesture}
        onPointerCancel={cancelGesture}
        onLostPointerCapture={finishGesture}
        onDoubleClick={(e) => {
          if (toolRef.current !== "select" || capturing.current) return;
          const index = selectedRef.current;
          if (index !== null && items.current[index]?.kind === "text") {
            changeTool("text");
            beginText(point(e), index);
          }
        }}
      />
      {tool === "select" &&
        selected !== null &&
        selectionBounds &&
        !textDraft && (
          <div
            className="annotation-selection"
            style={{
              left: selectionBounds.x,
              top: selectionBounds.y,
              width: selectionBounds.width,
              height: selectionBounds.height,
            }}
          >
            {(["nw", "ne", "sw", "se"] as const).map((handle) => (
              <button
                key={handle}
                className={`annotation-resize-handle handle-${handle}`}
                aria-label={t("Ridimensiona da {0}", { nw: "alto a sinistra", ne: "alto a destra", sw: "basso a sinistra", se: "basso a destra" }[handle])}
                disabled={busy}
                onPointerDown={(e) => startManipulation(e, selected, handle)}
                onPointerMove={moveManipulation}
                onPointerUp={finishGesture}
                onPointerCancel={cancelGesture}
                onLostPointerCapture={finishGesture}
              />
            ))}
          </div>
        )}
      {textDraft && (
        <div
          className="annotation-text-editor"
          style={{
            left: textDraft.note.x * viewport.width - 10,
            top: textDraft.note.y * viewport.height - 10,
          }}
        >
          <textarea
            ref={textInput}
            autoFocus
            aria-label={t("Testo da aggiungere allo schermo")}
            placeholder={t("Scrivi qui…")}
            maxLength={4000}
            spellCheck={false}
            style={{
              color,
              fontSize: font,
              width: textDraft.note.width * viewport.width,
              height: Math.max(
                font * 1.35,
                font * 1.35 * textDraft.note.text.split("\n").length,
              ),
            }}
            value={textDraft.note.text}
            onChange={(e) => {
              const next = {
                ...draft.current!,
                note: { ...draft.current!.note, text: e.target.value },
              };
              draft.current = next;
              setTextDraft(next);
            }}
          />
          <div>
            <span>{primaryModifier} {t("+ Invio")}</span>
            <button
              title={t("Annulla testo")}
              aria-label={t("Annulla testo")}
              onClick={cancelText}
            >
              <X size={14} />
            </button>
            <button className="confirm-text" onClick={commitText}>
              <Check size={14} /> {t("Inserisci")}
            </button>
          </div>
        </div>
      )}
      {dock ? (
        <button
          className="annotation-edge-tab"
          title={t("Mostra editor annotazioni")}
          aria-label={t("Mostra editor annotazioni")}
          onClick={() => setDock(false)}
        >
          <ChevronDown size={19} />
        </button>
      ) : (
        <>
          <div
            className="annotation-toolbar"
            role="toolbar"
            aria-label={t("Editor annotazioni")}
          >
            <div
              className="annotation-tools"
              role="group"
              aria-label={t("Strumento")}
            >
              <button
                className={tool === "select" ? "active" : ""}
                title={t("Seleziona, sposta e ridimensiona (V)")}
                aria-label={t("Seleziona e sposta")}
                aria-pressed={tool === "select"}
                onClick={() => changeTool("select")}
                disabled={busy}
              >
                <Hand size={19} />
                <kbd>V</kbd>
              </button>
              <button
                className={tool === "brush" ? "active" : ""}
                title={t("Pennello (B)")}
                aria-label={t("Pennello")}
                aria-pressed={tool === "brush"}
                onClick={() => changeTool("brush")}
                disabled={busy}
              >
                <Pencil size={19} />
                <kbd>B</kbd>
              </button>
              <button
                className={tool === "text" ? "active" : ""}
                title={t("Testo (T)")}
                aria-label={t("Testo")}
                aria-pressed={tool === "text"}
                onClick={() => changeTool("text")}
                disabled={busy}
              >
                <Type size={20} />
                <kbd>T</kbd>
              </button>
              <div className="annotation-control">
                <button
                  className={tool === "shape" ? "active" : ""}
                  title={t("Forme (G) · usa le frecce per scegliere")}
                  aria-label={t("Forme")}
                  aria-expanded={popup === "shape"}
                  onClick={() => {
                    if (tool !== "shape") changeTool("shape");
                    togglePopup("shape");
                  }}
                  disabled={busy}
                >
                  <Shapes size={20} />
                  <kbd>G</kbd>
                  <ChevronDown size={12} />
                </button>
                {popup === "shape" && (
                  <div className="annotation-popover shapes-popover">
                    <div className="annotation-popover-label">
                      {t("Forme")}<span>{t("← → scegli · Invio conferma")}</span>
                    </div>
                    <div
                      className="annotation-shapes"
                      role="radiogroup"
                      aria-label={t("Forma")}
                    >
                      {shapeKinds.map((kind) => {
                        const Icon = shapeIcons[kind];
                        return (
                          <button
                            key={kind}
                            role="radio"
                            aria-checked={shape === kind}
                            className={shape === kind ? "selected" : ""}
                            onClick={() => {
                              changeShape(kind);
                              setPopup(null);
                            }}
                          >
                            <Icon size={23} />
                            <span>{shapeLabels[kind]}</span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                )}
              </div>
              <button
                className={`annotation-interact ${tool === "interact" ? "active" : ""}`}
                title={t("Interagisci (I): clic e scorrimento raggiungono le altre finestre; i disegni restano visibili. Esc chiude l’editor.")}
                aria-label={t("Interagisci con il desktop")}
                aria-pressed={tool === "interact"}
                onClick={() => changeTool("interact")}
                disabled={busy}
              >
                <MousePointerClick size={19} /><span>{t("Interagisci")}</span><kbd>I</kbd>
              </button>
            </div>
            <i className="annotation-divider" />
            <div className="annotation-control">
              <PropertyButton
                label={t("Scegli colore")}
                open={popup === "color"}
                onClick={() => togglePopup("color")}
              >
                <i
                  className="annotation-color-dot"
                  style={{ background: color }}
                />
                <span>{t("Colore")}</span>
              </PropertyButton>
              {popup === "color" && (
                <div className="annotation-popover">
                  <ColorControls value={color} onChange={updateColor} />
                </div>
              )}
            </div>
            <div className="annotation-control">
              <PropertyButton
                label={tool === "text" ? t("Dimensione testo") : t("Spessore tratto")}
                open={popup === "size"}
                onClick={() => togglePopup("size")}
              >
                {tool === "text" ? (
                  <Type size={16} />
                ) : (
                  <i
                    className="annotation-stroke-preview"
                    style={{
                      width: Math.min(size, 14),
                      height: Math.min(size, 14),
                    }}
                  />
                )}
                <span>
                  {tool === "text" ? font : size}
                  <small> px</small>
                </span>
              </PropertyButton>
              {popup === "size" && (
                <div className="annotation-popover">
                  <SizeControls
                    text={tool === "text"}
                    value={tool === "text" ? font : size}
                    onChange={updateSize}
                  />
                </div>
              )}
            </div>
            {tool === "shape" && !["line", "arrow"].includes(shape) && (
              <button
                className={`annotation-fill ${filled ? "active" : ""}`}
                title={
                  filled
                    ? t("Forma piena: passa al contorno")
                    : t("Solo contorno: attiva riempimento")
                }
                aria-label={t("Riempimento forma")}
                aria-pressed={filled}
                onClick={() => {
                  controls.current.filled = !filled;
                  setFilled(!filled);
                }}
              >
                <PaintBucket size={17} />
                <span>{filled ? t("Piena") : t("Contorno")}</span>
              </button>
            )}
            {tool === "select" && <div className="annotation-clipboard" role="group" aria-label={t("Copia e incolla elementi")}>
              <button type="button" className="annotation-icon-button" title={t("Copia elemento ({0}+C)", primaryModifier)} aria-label={t("Copia elemento selezionato")}
                disabled={busy || selected === null} onClick={copyToClipboard}><Copy size={17}/></button>
              <button type="button" className="annotation-icon-button" title={t("Incolla elemento copiato ({0}+V)", primaryModifier)} aria-label={t("Incolla elemento copiato")}
                disabled={busy || !canPaste} onClick={() => { if (clipboard.current) pasteSelection(clipboard.current); }}><ClipboardPaste size={17}/></button>
            </div>}
            <i className="annotation-divider" />
            <button
              className="annotation-icon-button"
              title={t("Annulla ({0}+Z)", primaryModifier)}
              aria-label={t("Annulla annotazione")}
              disabled={busy || !history.undo}
              onClick={undo}
            >
              <Undo2 size={18} />
            </button>
            <button
              className="annotation-icon-button"
              title={t("Ripeti ({0}+Shift+Z)", primaryModifier)}
              aria-label={t("Ripeti annotazione")}
              disabled={busy || !history.redo}
              onClick={redo}
            >
              <Redo2 size={18} />
            </button>
            <button
              className="annotation-icon-button"
              title={t("Cancella tutte le annotazioni")}
              aria-label={t("Cancella tutte le annotazioni")}
              disabled={busy || (!history.count && !textDraft)}
              onClick={() => {
                cancelText();
                gesture.current = null;
                manipulation.current = null;
                select(null);
                remember();
                items.current = [];
                updateHistory();
                redraw();
              }}
            >
              <Trash2 size={17} />
            </button>
            <button
              className="annotation-capture"
              disabled={busy}
              onClick={() => void capture()}
              title={formatShortcut(shortcut)}
            >
              <Camera size={17} />
              <span>{busy ? t("Salvataggio…") : "Screenshot"}</span>
            </button>
            <DockButton
              disabled={busy}
              onClick={() => {
                commitText();
                finishGesture();
                setDock(true);
                setPopup(null);
              }}
            />
            {!showHelp && <button
              className="annotation-icon-button annotation-hints-toggle"
              title={showHelp ? t("Nascondi suggerimenti") : t("Mostra suggerimenti")}
              aria-label={showHelp ? t("Nascondi suggerimenti") : t("Mostra suggerimenti")}
              aria-expanded={showHelp}
              aria-controls="annotation-help"
              onClick={() => { setShowHelp(!showHelp); setPopup(null); }}
            >
              <MessageSquareText size={15} /><span>{t("Suggerimenti")}</span>
            </button>}
            <button
              className="annotation-icon-button annotation-close"
              title={t("Nascondi annotazioni: i disegni rimangono disponibili")}
              aria-label={t("Nascondi annotazioni")}
              disabled={busy}
              onClick={close}
            >
              <X size={18} />
            </button>
          </div>
          {showHelp && <div id="annotation-help" className="annotation-context" aria-live="polite">
            <span className="annotation-tool-name">
              {tool === "interact" ? (
                <MousePointerClick size={12} />
              ) : tool === "select" ? (
                <Hand size={12} />
              ) : tool === "brush" ? (
                <Pencil size={12} />
              ) : tool === "text" ? (
                <Type size={12} />
              ) : (
                <ShapeIcon size={12} />
              )}{" "}
              {tool === "interact" ? "Desktop" : tool === "select"
                ? t("Selezione")
                : tool === "brush"
                  ? t("Pennello")
                  : tool === "text"
                    ? t("Testo")
                    : shapeLabels[shape]}
            </span>
            <span title={hint}>{hint} {t("· Esc chiude")}</span>
            <button type="button" className="annotation-help-close" title={t("Chiudi suggerimenti")} aria-label={t("Chiudi suggerimenti")} onClick={() => setShowHelp(false)}><X size={14} /></button>
          </div>}
        </>
      )}
      {message && (
        <div className="annotation-toast" role="status">
          <Check size={14} />
          {message}
        </div>
      )}
    </div>
  );
}
