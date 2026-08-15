import type { LibraryRow } from "../../lib/api";

/**
 * Las reglas de «Hoy», puras y fuera de React, como el filtrado y la
 * ordenación de la biblioteca.
 *
 * `ahora` entra por parámetro y no se lee del reloj aquí dentro: una regla que
 * mira la hora por su cuenta no se puede probar sin viajar en el tiempo, y
 * media estantería depende de cuánto hace de la última partida.
 */

const DIA = 86_400;
const SEIS_MESES = 182 * DIA;

/** Lo que cabe en un estante. Pasado eso, la sugerencia ya es una lista. */
const POR_ESTANTE = 12;

export interface Estanteria {
  id: string;
  titulo: string;
  /** Por qué están ahí estos juegos. Una estantería sin motivo es una lista. */
  motivo: string;
  juegos: LibraryRow[];
}

export interface Destacado {
  juego: LibraryRow;
  motivo: string;
}

/** Sin copia no hay nada que proponer: un deseado no se puede jugar hoy. */
function enPropiedad(rows: LibraryRow[]): LibraryRow[] {
  return rows.filter((row) => row.owned_stores.length > 0);
}

/** Terminado o abandonado es una decisión tomada, y «Hoy» no la reabre. */
function pendiente(row: LibraryRow): boolean {
  return row.status !== "finished" && row.status !== "abandoned";
}

function porTitulo(a: LibraryRow, b: LibraryRow): number {
  return a.sort_title.localeCompare(b.sort_title, "es");
}

/**
 * Lo más reciente primero, y lo que no publica fecha al final.
 *
 * Solo Steam publica la última partida, así que un juego de GOG llega aquí sin
 * fecha aunque se haya jugado ayer. Se va al final, que es lo que se puede
 * afirmar; tratarlo como «nunca jugado» sería fingir un dato.
 */
function porUltimaPartida(a: LibraryRow, b: LibraryRow): number {
  if (a.last_played_at === null && b.last_played_at === null) return porTitulo(a, b);
  if (a.last_played_at === null) return 1;
  if (b.last_played_at === null) return -1;
  return b.last_played_at - a.last_played_at;
}

/**
 * Las estanterías que tienen algo dentro, en el orden en que se pintan.
 *
 * Una estantería vacía no se devuelve. Enseñar «hace mucho que no lo tocas» sin
 * un solo juego debajo no informa de nada y convierte la pantalla en una lista
 * de encabezados, que es justo lo contrario de una recomendación.
 */
export function estanterias(rows: LibraryRow[], ahora: number): Estanteria[] {
  const mios = enPropiedad(rows);

  const candidatas: Estanteria[] = [
    {
      id: "a-medias",
      titulo: "Lo dejaste a medias",
      motivo: "Marcados como «jugando»",
      juegos: mios.filter((row) => row.status === "playing").sort(porUltimaPartida),
    },
    {
      id: "sin-tocar",
      titulo: "Hace mucho que no lo tocas",
      motivo: "Más de seis meses desde la última partida que publica la tienda",
      juegos: mios
        .filter(
          (row) =>
            pendiente(row) && row.last_played_at !== null && ahora - row.last_played_at > SEIS_MESES,
        )
        // Al revés que el resto: primero el que lleva más tiempo criando polvo.
        .sort((a, b) => (a.last_played_at ?? 0) - (b.last_played_at ?? 0)),
    },
    {
      id: "sin-estrenar",
      titulo: "Sin estrenar",
      motivo: "En tu biblioteca y sin una sola partida",
      juegos: mios
        .filter((row) => pendiente(row) && row.playtime_minutes === 0 && row.last_played_at === null)
        .sort(porTitulo),
    },
    {
      id: "dos-veces",
      titulo: "Lo tienes dos veces",
      motivo: "La misma ficha con copia en más de una tienda",
      juegos: mios.filter((row) => row.owned_stores.length > 1).sort(porTitulo),
    },
  ];

  return candidatas
    .filter((estante) => estante.juegos.length > 0)
    .map((estante) => ({ ...estante, juegos: estante.juegos.slice(0, POR_ESTANTE) }));
}

/**
 * El juego que se propone hoy, con la razón por la que se propone.
 *
 * El orden de preferencia es el de lo que menos cuesta retomar: lo que estabas
 * jugando gana siempre, porque proponerte empezar otra cosa mientras tienes una
 * a medias es justo lo que hace crecer la pila.
 *
 * Cuando no hay nada empezado, la elección rota con el día. El corte sale de
 * `ahora`, así que dentro del mismo día siempre sale el mismo juego —cambiar de
 * recomendación cada vez que se pinta la pantalla la convierte en una
 * tragaperras— y al día siguiente sale otro.
 */
export function destacado(rows: LibraryRow[], ahora: number): Destacado | null {
  const mios = enPropiedad(rows);

  const jugando = mios.filter((row) => row.status === "playing").sort(porUltimaPartida);
  if (jugando[0]) {
    return { juego: jugando[0], motivo: "Lo tienes a medias" };
  }

  const sinEstrenar = mios
    .filter((row) => pendiente(row) && row.playtime_minutes === 0)
    .sort(porTitulo);
  if (sinEstrenar.length > 0) {
    return { juego: delDia(sinEstrenar, ahora), motivo: "Lo tienes sin estrenar" };
  }

  const resto = mios.filter(pendiente).sort(porTitulo);
  if (resto.length > 0) {
    return { juego: delDia(resto, ahora), motivo: "De lo que tienes pendiente" };
  }

  return null;
}

function delDia(juegos: LibraryRow[], ahora: number): LibraryRow {
  return juegos[Math.floor(ahora / DIA) % juegos.length]!;
}
