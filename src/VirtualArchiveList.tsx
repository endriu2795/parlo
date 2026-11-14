import { useCallback, useRef, useState, type ReactNode, type UIEventHandler } from "react";
import { defaultRangeExtractor, useVirtualizer, type Range } from "@tanstack/react-virtual";

type Row = { id: string };

export default function VirtualArchiveList<T extends Row>({ label, rows, estimateSize, renderRow, onScroll, children }: {
  label: string;
  rows: readonly T[];
  estimateSize: number;
  renderRow: (row: T, index: number) => ReactNode;
  onScroll?: UIEventHandler<HTMLElement>;
  children?: ReactNode;
}) {
  const scrollRef = useRef<HTMLElement>(null);
  const [focusedId, setFocusedId] = useState<string>();
  const focusedIndex = focusedId === undefined ? -1 : rows.findIndex(row => row.id === focusedId);
  const getItemKey = useCallback((index: number) => rows[index].id, [rows]);
  const rangeExtractor = useCallback((range: Range) => {
    const indexes = defaultRangeExtractor(range);
    // Keep the active button mounted if the user scrolls away with the mouse.
    if (focusedIndex >= 0 && !indexes.includes(focusedIndex)) {
      indexes.push(focusedIndex);
      indexes.sort((a, b) => a - b);
    }
    return indexes;
  }, [focusedIndex]);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    getItemKey,
    estimateSize: () => estimateSize,
    overscan: 5,
    rangeExtractor,
  });

  return (
    <nav className="call-list" aria-label={label} ref={scrollRef} onScroll={onScroll}>
      {rows.length > 0 && (
        <div className="virtual-archive-rows" style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map(item => (
            <div className="virtual-archive-row" key={item.key} data-index={item.index} ref={virtualizer.measureElement}
              style={{ transform: `translateY(${item.start}px)` }}
              onFocus={() => setFocusedId(rows[item.index].id)}
              onBlur={event => {
                if (!event.currentTarget.contains(event.relatedTarget)) setFocusedId(undefined);
              }}>
              {renderRow(rows[item.index], item.index)}
            </div>
          ))}
        </div>
      )}
      {children}
    </nav>
  );
}
