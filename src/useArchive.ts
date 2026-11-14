import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, desktop } from "./api";
import { noFilter, type ArchiveFilter } from "./ArchiveFilters";
import { archiveDateBounds } from "./archiveDates";
import type { Call, CallCursor, TagCount } from "./types";

const PAGE_SIZE = 10;
type Listing = { calls: Call[]; total: number; cursor: CallCursor | null };
const empty: Listing = { calls: [], total: 0, cursor: null };

function compare(a: CallCursor, b: CallCursor) {
  if (a.created_at !== b.created_at) return a.created_at > b.created_at ? -1 : 1;
  return a.id === b.id ? 0 : a.id > b.id ? -1 : 1;
}

export function useArchive(onError: (message: string) => void) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ArchiveFilter>(noFilter);
  const [tags, setTags] = useState<TagCount[]>([]);
  const filters = useMemo(() => {
    const { from, to } = archiveDateBounds(filter.from, filter.to)!;
    return { query, dateFrom: from, dateTo: to, tags: filter.tags };
  }, [query, filter]);
  const [view, setView] = useState<{ filters: typeof filters; listing: Listing; loading: boolean }>();
  const controls = useRef<{ loadMore: () => void; refresh: () => void }>(null);

  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    let listing = empty;
    let loading = false;
    let requests = Promise.resolve();
    const publish = () => { if (alive) setView({ filters, listing, loading }); };
    const enqueue = (task: () => Promise<void>) => {
      requests = requests.then(async () => {
        if (alive) await task();
      }).catch(error => { if (alive) onError(String(error)); });
    };
    const refresh = () => enqueue(async () => {
      try {
        const page = await api.list(filters, null, PAGE_SIZE);
        listing = { calls: page.items, total: page.total ?? 0, cursor: page.next_cursor };
      } finally {
        publish();
      }
    });
    const updateCall = (id: string) => enqueue(async () => {
      const page = await api.list(filters, null, 1, id);
      const item = page.items[0];
      const calls = listing.calls.filter(call => call.id !== id);
      // A cursor remains valid even when its row is deleted or stops matching.
      if (item && (!listing.cursor || compare(item, listing.cursor) <= 0)) {
        calls.push(item);
        calls.sort(compare);
      }
      listing = { ...listing, calls, total: page.total ?? listing.total };
      publish();
    });
    controls.current = {
      refresh,
      loadMore: () => {
        if (loading || !listing.cursor) return;
        loading = true;
        publish();
        enqueue(async () => {
          try {
            if (!listing.cursor) return;
            const page = await api.list(filters, listing.cursor, PAGE_SIZE);
            listing = { ...listing, calls: [...listing.calls, ...page.items], cursor: page.next_cursor };
          } finally {
            loading = false;
            publish();
          }
        });
      },
    };
    const listener = listen<string | null>("calls-changed", ({ payload }) => {
      if (payload) updateCall(payload);
      else refresh();
    });
    refresh();
    return () => {
      alive = false;
      controls.current = null;
      void listener.then(stop => stop());
    };
  }, [filters, onError]);

  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    let revision = 0;
    const refresh = async () => {
      const current = ++revision;
      try {
        const values = await api.tags();
        if (alive && current === revision) setTags(values);
      } catch (error) {
        if (alive && current === revision) onError(String(error));
      }
    };
    const listener = listen("archive-tags-changed", () => void refresh());
    void refresh();
    return () => { alive = false; void listener.then(stop => stop()); };
  }, [onError]);

  const current = view?.filters === filters ? view : undefined;
  return {
    ...(current?.listing ?? empty),
    loading: desktop && !current,
    loadingMore: !!current?.loading,
    query, setQuery, filter, setFilter, tags,
    refresh: useCallback(() => controls.current?.refresh(), []),
    loadMore: useCallback(() => controls.current?.loadMore(), []),
  };
}
