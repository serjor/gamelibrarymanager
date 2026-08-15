import { useEffect, useMemo, useRef, useState } from "react";

interface Options {
  itemCount: number;
  rowHeight: number;
  /**
   * Ancho de cada columna. Sin él la rejilla es de una sola columna, que es lo
   * que necesita una tabla: una lista es una rejilla que no tiene nada que
   * dividir.
   */
  columnWidth?: number;
  /** Filas de más arriba y abajo, para que no se vea el hueco al desplazar. */
  overscan?: number;
}

interface VirtualGrid {
  containerRef: React.RefObject<HTMLDivElement | null>;
  columns: number;
  /** Alto total, para que la barra de desplazamiento sea la de verdad. */
  totalHeight: number;
  /** Desplazamiento de la ventana visible dentro del alto total. */
  offsetY: number;
  range: { start: number; end: number };
}

/**
 * Ventana de elementos visibles sobre una rejilla de altura fija.
 *
 * Cien líneas de dependencia menos: pintar mil portadas de golpe son mil nodos
 * y mil descargas de imagen, y ahí es donde la rejilla empieza a dar tirones.
 * Con altura fija el cálculo es aritmética, no medición, así que no hace falta
 * ninguna librería.
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
