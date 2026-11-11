import { useCallback, useEffect, useState } from "react";
import { api, desktop } from "./api";
import type { Settings, SharedDocument, SharedTranscript } from "./types";

const empty: SharedTranscript[] = [];

export function useSharedTranscripts(sharing: Settings["sharing"]) {
  const folder = desktop && sharing?.read_shared ? sharing.folder : "";
  const [listing, setListing] = useState<{ folder: string; items: SharedTranscript[] }>();
  const [selection, setSelection] = useState<{ folder: string; id: string }>();
  const [document, setDocument] = useState<{ folder: string; id: string; result?: SharedDocument; error?: string }>();
  const items = folder && listing?.folder === folder ? listing.items : empty;
  const active = folder && selection?.folder === folder ? items.find(item => item.id === selection.id) : undefined;
  const id = active?.id;
  const title = active?.title;
  const publishedAt = active?.published_at;
  const close = useCallback(() => setSelection(undefined), []);

  useEffect(() => {
    setListing(undefined);
    setSelection(undefined);
    if (!folder) return;
    let alive = true;
    let running = false;
    const refresh = async () => {
      if (running) return;
      running = true;
      try {
        const items = await api.listSharedTranscripts();
        if (alive) {
          setListing({ folder, items });
          setSelection(previous => previous && items.some(item => item.id === previous.id) ? previous : undefined);
        }
      } catch {
        // A cloud client can temporarily lose access while syncing.
      } finally {
        running = false;
      }
    };
    void refresh();
    const timer = window.setInterval(() => { if (!window.document.hidden) void refresh(); }, 30000);
    window.addEventListener("focus", refresh);
    return () => {
      alive = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  }, [folder]);

  useEffect(() => {
    setDocument(undefined);
    if (!id) return;
    let alive = true;
    void api.getSharedTranscript(id).then(
      result => { if (alive) setDocument({ folder, id, result }); },
      error => { if (alive) setDocument({ folder, id, error: String(error) }); },
    );
    return () => { alive = false; };
  }, [folder, id, publishedAt, title]);

  return {
    items,
    active,
    document: document?.folder === folder && document?.id === id ? document : undefined,
    open: (id: string) => setSelection({ folder, id }),
    close,
  };
}
