import type { LibraryRow, PriceRow } from "../../lib/api";

/**
 * Las reglas de la lista de deseados, puras y fuera de React, como el filtrado
 * y la ordenación de la biblioteca.
 *
 * Lo que convierte una lista de deseados en una decisión de compra es el orden:
 * lo más rebajado primero. Un deseado ordenado por título es una lista de
 * buenas intenciones.
 */

export interface Deseado {
  juego: LibraryRow;
  /** Falta mientras no se hayan pedido los precios, o si no lo vende nadie. */
  precio: PriceRow | null;
}

/**
 * Los deseados con su precio al lado, lo más rebajado primero.
 *
 * Lo que no tiene precio va al final, no arriba ni mezclado: es «no hay dato»,
 * igual que las horas que no publica ninguna tienda, y son dos preguntas
 * distintas. Entre iguales manda el título, para que el orden no baile.
 */
export function deseados(rows: LibraryRow[], precios: PriceRow[]): Deseado[] {
  const porJuego = new Map(precios.map((precio) => [precio.game_id, precio]));

  return rows
    .filter((row) => row.wishlist_stores.length > 0)
    .map((juego) => ({ juego, precio: porJuego.get(juego.game_id) ?? null }))
    .sort(porDescuento);
}

function porDescuento(a: Deseado, b: Deseado): number {
  if (a.precio === null && b.precio === null) return porTitulo(a, b);
  if (a.precio === null) return 1;
  if (b.precio === null) return -1;
  return b.precio.cut - a.precio.cut || porTitulo(a, b);
}

function porTitulo(a: Deseado, b: Deseado): number {
  return a.juego.sort_title.localeCompare(b.juego.sort_title, "es");
}

/**
 * Si el precio de ahora iguala o mejora el mínimo histórico.
 *
 * Es la única pregunta que de verdad se hace quien mira esta pantalla: un −60 %
 * no dice nada si el juego estuvo a −75 % hace dos meses. Se compara con `<=`
 * porque el mínimo de ITAD incluye la oferta en curso: si no, la mejor rebaja
 * de la historia de un juego no se marcaría nunca.
 */
export function enMinimoHistorico(precio: PriceRow): boolean {
  return precio.low_all_time !== null && precio.amount <= precio.low_all_time;
}

/**
 * Céntimos a texto, en la moneda que diga el propio precio.
 *
 * El idioma se fija a español, como el desempate por título de la biblioteca:
 * es el idioma de la aplicación, y dejarlo al azar del sistema haría que la
 * misma cifra se escribiera de dos formas en dos máquinas.
 */
export function dinero(cents: number, currency: string): string {
  return new Intl.NumberFormat("es-ES", { style: "currency", currency }).format(cents / 100);
}
