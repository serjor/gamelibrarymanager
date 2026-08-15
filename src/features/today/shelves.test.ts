import { describe, expect, it } from "bun:test";
import type { LibraryRow } from "../../lib/api";
import { destacado, estanterias } from "./shelves";

function row(overrides: Partial<LibraryRow>): LibraryRow {
  const title = overrides.title ?? "Juego";
  return {
    game_id: crypto.randomUUID(),
    title,
    sort_title: title.toLowerCase(),
    cover_url: null,
    summary: null,
    release_year: null,
    genres: [],
    owned_stores: ["steam"],
    wishlist_stores: [],
    store_cover_url: null,
    store_url: null,
    playtime_minutes: 0,
    last_played_at: null,
    status: null,
    rating: null,
    notes: null,
    ...overrides,
  };
}

/** Un instante fijo: las reglas cortan por «hace seis meses». */
const AHORA = 1_760_000_000;
const DIA = 86_400;

const titulos = (rows: LibraryRow[]) => rows.map((r) => r.title);
const ids = (rows: LibraryRow[]) => estanterias(rows, AHORA).map((e) => e.id);

describe("estanterías de Hoy", () => {
  it("una biblioteca vacía no devuelve ninguna estantería", () => {
    // No es lo mismo que devolver estanterías vacías: la pantalla pinta lo que
    // haya, y una lista de encabezados sin nada debajo no informa de nada.
    expect(estanterias([], AHORA)).toEqual([]);
    expect(destacado([], AHORA)).toBeNull();
  });

  it("un deseado no se propone: no se puede jugar hoy", () => {
    const deseado = [row({ title: "Deseado", owned_stores: [], wishlist_stores: ["steam"] })];
    expect(estanterias(deseado, AHORA)).toEqual([]);
    expect(destacado(deseado, AHORA)).toBeNull();
  });

  it("solo devuelve las estanterías que tienen algo dentro", () => {
    // Nada empezado y nada en dos tiendas: esas dos no salen.
    const biblioteca = [
      row({ title: "Sin abrir" }),
      row({ title: "Aparcado", playtime_minutes: 600, last_played_at: AHORA - 400 * DIA }),
    ];
    expect(ids(biblioteca)).toEqual(["sin-tocar", "sin-estrenar"]);
  });

  it("«hace mucho que no lo tocas» empieza por el que lleva más tiempo", () => {
    const biblioteca = [
      row({ title: "Hace un año", playtime_minutes: 600, last_played_at: AHORA - 370 * DIA }),
      row({ title: "Hace tres años", playtime_minutes: 600, last_played_at: AHORA - 1100 * DIA }),
      // Justo por debajo del corte: no entra.
      row({ title: "Hace un mes", playtime_minutes: 600, last_played_at: AHORA - 30 * DIA }),
    ];
    const estante = estanterias(biblioteca, AHORA).find((e) => e.id === "sin-tocar");
    expect(titulos(estante?.juegos ?? [])).toEqual(["Hace tres años", "Hace un año"]);
  });

  it("lo terminado y lo abandonado no vuelve a proponerse", () => {
    // Es una decisión ya tomada, y «Hoy» no la reabre. Sí sigue contando para
    // «lo tienes dos veces», que no es una propuesta sino un dato de la copia.
    const biblioteca = [
      row({
        title: "Terminado",
        status: "finished",
        playtime_minutes: 600,
        last_played_at: AHORA - 400 * DIA,
      }),
      row({ title: "Abandonado", status: "abandoned", owned_stores: ["steam", "gog"] }),
    ];
    expect(ids(biblioteca)).toEqual(["dos-veces"]);
  });

  it("un juego solo de GOG no cuenta como «sin estrenar» si lo has jugado", () => {
    // GOG no publica la última partida, así que llega sin fecha. Lo que se
    // puede afirmar son las horas: con horas jugadas, estrenado está.
    const biblioteca = [
      row({ title: "De GOG jugado", owned_stores: ["gog"], playtime_minutes: 240 }),
      row({ title: "De GOG sin abrir", owned_stores: ["gog"] }),
    ];
    const estante = estanterias(biblioteca, AHORA).find((e) => e.id === "sin-estrenar");
    expect(titulos(estante?.juegos ?? [])).toEqual(["De GOG sin abrir"]);
  });
});

describe("la propuesta de Hoy", () => {
  it("lo que estabas jugando gana a cualquier cosa por empezar", () => {
    // Proponer algo nuevo mientras tienes uno a medias es lo que hace crecer
    // la pila, que es justo lo que esta pantalla intenta deshacer.
    const biblioteca = [
      row({ title: "Sin estrenar" }),
      row({ title: "A medias", status: "playing", playtime_minutes: 300 }),
    ];
    expect(destacado(biblioteca, AHORA)?.juego.title).toBe("A medias");
    expect(destacado(biblioteca, AHORA)?.motivo).toBe("Lo tienes a medias");
  });

  it("entre varios empezados, el de la partida más reciente", () => {
    const biblioteca = [
      row({ title: "Antiguo", status: "playing", last_played_at: AHORA - 100 * DIA }),
      row({ title: "Reciente", status: "playing", last_played_at: AHORA - 2 * DIA }),
      row({ title: "Sin fecha", status: "playing" }),
    ];
    expect(destacado(biblioteca, AHORA)?.juego.title).toBe("Reciente");
  });

  it("sin nada empezado, la elección cambia con el día pero no dentro del día", () => {
    // Una recomendación que cambia cada vez que se pinta la pantalla es una
    // tragaperras, no una recomendación.
    const biblioteca = [row({ title: "Uno" }), row({ title: "Dos" }), row({ title: "Tres" })];

    const hoy = destacado(biblioteca, AHORA)?.juego.title;
    expect(destacado(biblioteca, AHORA + 3600)?.juego.title).toBe(hoy!);

    const proximos = [1, 2, 3].map((d) => destacado(biblioteca, AHORA + d * DIA)?.juego.title);
    expect(new Set([hoy, ...proximos]).size).toBe(3);
  });
});
