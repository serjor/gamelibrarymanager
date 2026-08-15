import { useEffect, useMemo, useRef, useState } from "react";

interface Options {
  itemCount: number;
  rowHeight: number;
  /**
   * The width of each column. Without it the grid has one column, which is what
   * a table needs: a list is a grid that has nothing to divide.
   */
  columnWidth?: number;
  /** The extra rows above and below, so that you do not see an empty space
   *  while the list scrolls. */
  overscan?: number;
}

interface VirtualGrid {
  containerRef: React.RefObject<HTMLDivElement | null>;
  columns: number;
  /** The total height, so that the scroll bar is the true scroll bar. */
  totalHeight: number;
  /** The position of the visible window in the total height. */
  offsetY: number;
  range: { start: number; end: number };
}

/**
 * The window of visible items over a grid with a fixed height.
 *
 * One hundred lines of dependency fewer: to show one thousand covers together is
 * one thousand nodes and one thousand image downloads, and that is where the
 * grid starts to jump. With a fixed height the calculation is arithmetic and not
 * measurement, thus no library is necessary.
 */
export function useVirtualGrid({
  itemCount,
  rowHeight,
  columnWidth,
  overscan = 2,
}: Options): VirtualGrid {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;

    const measure = () =>
      setViewport({ width: element.clientWidth, height: element.clientHeight });
    measure();

    const onScroll = () => setScrollTop(element.scrollTop);
    element.addEventListener("scroll", onScroll, { passive: true });

    const observer = new ResizeObserver(measure);
    observer.observe(element);

    return () => {
      element.removeEventListener("scroll", onScroll);
      observer.disconnect();
    };
  }, []);

  return useMemo(() => {
    const columns = columnWidth ? Math.max(1, Math.floor(viewport.width / columnWidth)) : 1;
    const rows = Math.ceil(itemCount / columns);
    const firstRow = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
    const visibleRows = Math.ceil(viewport.height / rowHeight) + overscan * 2;
    const lastRow = Math.min(rows, firstRow + visibleRows);

    return {
      containerRef,
      columns,
      totalHeight: rows * rowHeight,
      offsetY: firstRow * rowHeight,
      range: {
        start: firstRow * columns,
        end: Math.min(itemCount, lastRow * columns),
      },
    };
  }, [itemCount, rowHeight, columnWidth, overscan, scrollTop, viewport]);
}
