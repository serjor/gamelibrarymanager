import type { PlayStatus } from "./api";

/**
 * Las etiquetas del estado, en un solo sitio.
 *
 * Las escribían por su cuenta la ficha y ahora también la tabla y la barra de
 * lote. Tres copias de la misma lista es la forma más fácil de que un día
 * «Pendiente» se llame de dos maneras según dónde lo mires.
 */
export const ETIQUETA_ESTADO: Record<PlayStatus, string> = {
  backlog: "Pendiente",
  playing: "Jugando",
  finished: "Terminado",
  abandoned: "Abandonado",
};

/** En el orden en que se recorre un juego, que es como se ordena por estado. */
export const ESTADOS: PlayStatus[] = ["backlog", "playing", "finished", "abandoned"];
