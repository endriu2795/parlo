import { useEffect, useRef, type KeyboardEvent } from "react";

const focusable =
  'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]';

/**
 * Modal behaviour shared by every dialog: initial focus, focus restore on close,
 * Tab trapped inside the dialog and Escape to close.
 */
export function useDialog<T extends HTMLElement>(onClose: () => void, initialFocus?: string) {
  const ref = useRef<T>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const target = (initialFocus && ref.current?.querySelector<HTMLElement>(initialFocus)) || ref.current?.querySelector<HTMLElement>(focusable);
    target?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, []);
  const onKeyDown = (event: KeyboardEvent<T>) => {
    if (event.defaultPrevented) return;
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;
    const controls = Array.from(ref.current?.querySelectorAll<HTMLElement>(focusable) || [])
      .filter(element => element.getClientRects().length);
    const first = controls[0], last = controls[controls.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  };
  return { ref, onKeyDown };
}

/** Closes a `<details>` menu on outside pointer, Escape, or when `resetKey` changes. */
export function useDetailsMenu(resetKey?: unknown) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) ref.current.open = false;
    };
    const escape = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape" && ref.current?.open) {
        ref.current.open = false;
        ref.current.querySelector("summary")?.focus();
      }
    };
    window.addEventListener("pointerdown", outside);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointerdown", outside);
      window.removeEventListener("keydown", escape);
    };
  }, []);
  useEffect(() => { if (ref.current) ref.current.open = false; }, [resetKey]);
  const close = () => { if (ref.current) ref.current.open = false; };
  return { ref, close };
}
